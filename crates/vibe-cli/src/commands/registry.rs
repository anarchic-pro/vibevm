//! `vibe registry …` — registry cache management.
//!
//! Spec: `VIBEVM-SPEC.md` §8.3 (cache layout, refresh).
//! Decentralized per-package model: PROP-002.
//!
//! `vibe registry sync` walks the lockfile and refreshes the on-disk
//! clone of every installed package. For `[[registry]]`-served entries
//! that means `git fetch` + hard-reset on the per-package clone under
//! `<cache>/<canonical-url-hash>/packages/<kind>-<name>/clone/`. For
//! `[[override]]`-served entries that means the same against the
//! `__overrides__/<kind>-<name>/clone/` subtree. Local-directory
//! registries (`--registry <path>`) and legacy v1 entries are reported
//! as skipped — there is no per-package clone to refresh for them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use vibe_core::manifest::{Lockfile, ProjectManifest};
use vibe_publish::{
    GitVerseCreator, PublishConfig, Publisher, load_token,
};
use vibe_registry::{MultiRegistryResolver, RefreshedVia};

use crate::cli::{
    RegistryArgs, RegistryPublishArgs, RegistrySubcommand, RegistrySyncArgs,
};
use crate::output;

pub fn run(ctx: &output::Context, args: RegistryArgs) -> Result<()> {
    match args.command {
        RegistrySubcommand::Sync(sub) => run_sync(ctx, sub),
        RegistrySubcommand::Publish(sub) => run_publish(ctx, sub),
    }
}

#[derive(Debug, Serialize)]
struct SyncReport {
    ok: bool,
    command: &'static str,
    refreshed: Vec<RefreshedReportEntry>,
    skipped: Vec<SkippedReportEntry>,
}

#[derive(Debug, Serialize)]
struct RefreshedReportEntry {
    kind: String,
    name: String,
    via: String, // "registry:<name>" or "override"
    #[serde(rename = "ref")]
    refname: String,
}

#[derive(Debug, Serialize)]
struct SkippedReportEntry {
    kind: String,
    name: String,
    reason: String,
}

fn run_sync(ctx: &output::Context, args: RegistrySyncArgs) -> Result<()> {
    let project_root = resolve_project_root(&args.path)?;
    let manifest_path = project_root.join(ProjectManifest::FILENAME);
    if !manifest_path.exists() {
        bail!(
            "no `vibe.toml` in `{}`; run `vibe init` first",
            project_root.display()
        );
    }
    let manifest = ProjectManifest::read(&manifest_path)
        .with_context(|| format!("reading `{}`", manifest_path.display()))?;

    let lockfile_path = project_root.join(Lockfile::FILENAME);
    if !lockfile_path.exists() {
        ctx.summary(
            "vibe registry sync: no `vibe.lock` yet — nothing installed, nothing to refresh.",
        );
        if ctx.is_json() {
            ctx.emit_json(&SyncReport {
                ok: true,
                command: "registry:sync",
                refreshed: Vec::new(),
                skipped: Vec::new(),
            })?;
        }
        return Ok(());
    }
    let lockfile = Lockfile::read(&lockfile_path)
        .with_context(|| format!("reading `{}`", lockfile_path.display()))?;

    if lockfile.packages.is_empty() {
        ctx.summary("vibe registry sync: lockfile is empty — nothing to refresh.");
        if ctx.is_json() {
            ctx.emit_json(&SyncReport {
                ok: true,
                command: "registry:sync",
                refreshed: Vec::new(),
                skipped: Vec::new(),
            })?;
        }
        return Ok(());
    }

    if manifest.registries.is_empty() {
        // Empty `[[registry]]` is legal (e.g., projects that only use
        // `--registry <path>` or `[[override]]`-only setups), but
        // `registry sync` has nothing to do without `[[registry]]`
        // entries to dispatch through. Override-only refresh would
        // need its own flag; for now, surface the situation.
        ctx.summary(
            "vibe registry sync: no `[[registry]]` entries in `vibe.toml` — nothing to refresh.",
        );
        if ctx.is_json() {
            ctx.emit_json(&SyncReport {
                ok: true,
                command: "registry:sync",
                refreshed: Vec::new(),
                skipped: Vec::new(),
            })?;
        }
        return Ok(());
    }

    let mrr = MultiRegistryResolver::open(
        &manifest.registries,
        &manifest.mirrors,
        &manifest.overrides,
    )
    .context("opening multi-registry resolver")?;

    ctx.heading(&format!(
        "Syncing {} package clone{} referenced by lockfile",
        lockfile.packages.len(),
        if lockfile.packages.len() == 1 { "" } else { "s" }
    ));

    let report = mrr
        .refresh_lockfile_clones(&lockfile)
        .context("refreshing per-package clones")?;

    let json_refreshed: Vec<RefreshedReportEntry> = report
        .refreshed
        .iter()
        .map(|e| RefreshedReportEntry {
            kind: e.kind.as_str().to_string(),
            name: e.name.clone(),
            via: match &e.via {
                RefreshedVia::Registry(n) => format!("registry:{n}"),
                RefreshedVia::Override => "override".to_string(),
            },
            refname: e.refname.clone(),
        })
        .collect();
    let json_skipped: Vec<SkippedReportEntry> = report
        .skipped
        .iter()
        .map(|e| SkippedReportEntry {
            kind: e.kind.as_str().to_string(),
            name: e.name.clone(),
            reason: e.reason.clone(),
        })
        .collect();

    if ctx.is_json() {
        ctx.emit_json(&SyncReport {
            ok: true,
            command: "registry:sync",
            refreshed: json_refreshed,
            skipped: json_skipped,
        })?;
        return Ok(());
    }

    if !report.refreshed.is_empty() {
        for e in &report.refreshed {
            let via_text = match &e.via {
                RefreshedVia::Registry(name) => format!("registry `{name}`"),
                RefreshedVia::Override => "override".to_string(),
            };
            ctx.step(&format!(
                "{}:{} @ {} via {}",
                e.kind, e.name, e.refname, via_text
            ));
        }
    }
    if !report.skipped.is_empty() {
        for e in &report.skipped {
            ctx.skipped(&format!("{}:{}", e.kind, e.name), &e.reason);
        }
    }

    ctx.summary(&format!(
        "\nvibe registry sync: {} refreshed, {} skipped.",
        report.refreshed.len(),
        report.skipped.len()
    ));
    Ok(())
}

fn resolve_project_root(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|e| anyhow!("canonicalizing `{}`: {e}", path.display()))?;
    Ok(super::init::strip_unc_public(canonical))
}

#[derive(Debug, Serialize)]
struct PublishReport {
    ok: bool,
    command: &'static str,
    host: String,
    org_url: String,
    repo_name: String,
    repo_url: String,
    tag: String,
    created_repo: bool,
    dry_run: bool,
}

fn run_publish(ctx: &output::Context, args: RegistryPublishArgs) -> Result<()> {
    let project_root = resolve_project_root(&args.path)?;
    let manifest_path = project_root.join(ProjectManifest::FILENAME);
    if !manifest_path.exists() {
        bail!(
            "no `vibe.toml` in `{}`; run `vibe init` first",
            project_root.display()
        );
    }
    let manifest = ProjectManifest::read(&manifest_path)
        .with_context(|| format!("reading `{}`", manifest_path.display()))?;

    if manifest.registries.is_empty() {
        bail!(
            "no `[[registry]]` entries in `{}`. `vibe registry publish` needs a target registry.",
            manifest_path.display()
        );
    }

    let registry_section = match &args.registry {
        Some(name) => manifest
            .registry_by_name(name)
            .ok_or_else(|| anyhow!("no `[[registry]]` named `{name}` in `{}`", manifest_path.display()))?,
        None => manifest
            .primary_registry()
            .ok_or_else(|| anyhow!("no `[[registry]]` configured"))?,
    };

    // Canonicalise the source dir.
    let source_dir = args
        .source
        .canonicalize()
        .with_context(|| format!("source path `{}`", args.source.display()))?;
    let source_dir = super::init::strip_unc_public(source_dir);

    // Phase A only ships a GitVerse adapter. When more hosts land
    // (GitHub, Gitea, Forgejo per PROP-002 §2.10), pick the adapter
    // by host. For now, blow up clearly if the org URL doesn't look
    // like a GitVerse-shaped one — better to refuse than silently
    // mis-target.
    let host = "gitverse.ru";

    ctx.heading(&format!(
        "Publishing {} → registry `{}` (`{}`){}",
        source_dir.display(),
        registry_section.name,
        registry_section.url,
        if args.dry_run { " [dry-run]" } else { "" },
    ));

    let token = load_token(host).context("loading publish token")?;
    if !args.dry_run {
        ctx.step(&format!(
            "Loaded publish token from {} (value redacted)",
            match token.source() {
                vibe_publish::TokenSource::Explicit => "explicit argument".to_string(),
                vibe_publish::TokenSource::EnvVar(name) => format!("$ {name}"),
                vibe_publish::TokenSource::File(p) => p.display().to_string(),
            }
        ));
    }
    let creator = GitVerseCreator::new(token).context("constructing GitVerse adapter")?;

    let config = PublishConfig {
        source_dir: source_dir.clone(),
        org_url: registry_section.url.clone(),
        naming: registry_section.naming,
        tag_prefix: "v".to_string(),
        dry_run: args.dry_run,
    };

    let outcome = Publisher::new(&creator)
        .publish(&config)
        .map_err(|e| anyhow!("{e}"))?;

    if ctx.is_json() {
        ctx.emit_json(&PublishReport {
            ok: true,
            command: "registry:publish",
            host: outcome.host.clone(),
            org_url: registry_section.url.clone(),
            repo_name: outcome.repo_name.clone(),
            repo_url: outcome.repo_url.clone(),
            tag: outcome.tag.clone(),
            created_repo: outcome.created_repo,
            dry_run: outcome.dry_run,
        })?;
        return Ok(());
    }

    if outcome.created_repo {
        ctx.step(&format!(
            "Created repository `{}` on `{}`",
            outcome.repo_name, outcome.host
        ));
    } else {
        ctx.step(&format!(
            "Reusing existing repository `{}` on `{}`",
            outcome.repo_name, outcome.host
        ));
    }
    if outcome.dry_run {
        ctx.summary(&format!(
            "\nvibe registry publish [dry-run]: would push to `{}` and tag `{}`. \
             Re-run without `--dry-run` to apply.",
            outcome.repo_url, outcome.tag
        ));
    } else {
        ctx.summary(&format!(
            "\nvibe registry publish: pushed `{}:{}` @ {} → `{}` (tag `{}`).",
            outcome.kind, outcome.name, outcome.version, outcome.repo_url, outcome.tag
        ));
    }
    Ok(())
}
