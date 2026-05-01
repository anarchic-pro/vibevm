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
    PublishConfig, Publisher, creator_for_url, extract_host_segment, extract_org_segment,
    load_token_for_host,
};
use vibe_registry::{MultiRegistryResolver, RefreshedVia};

use crate::cli::{
    RegistryArgs, RegistryListArgs, RegistryPublishArgs, RegistrySubcommand, RegistrySyncArgs,
};
use crate::output;

pub fn run(ctx: &output::Context, args: RegistryArgs) -> Result<()> {
    match args.command {
        RegistrySubcommand::Sync(sub) => run_sync(ctx, sub),
        RegistrySubcommand::Publish(sub) => run_publish(ctx, sub),
        RegistrySubcommand::List(sub) => run_list(ctx, sub),
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

#[derive(Debug, Serialize)]
struct ListReport {
    ok: bool,
    command: &'static str,
    registries: Vec<ListReportRegistry>,
    mirrors: Vec<ListReportMirror>,
    overrides: Vec<ListReportOverride>,
}

#[derive(Debug, Serialize)]
struct ListReportRegistry {
    name: String,
    url: String,
    #[serde(rename = "ref")]
    refname: String,
    naming: String,
    host: String,
    org: String,
    /// Adapter that `vibe registry publish` would dispatch to for this
    /// registry's host. `null` if the host has no adapter today.
    adapter: Option<String>,
    /// Mirrors that fall through to this registry, in priority order.
    mirrors: Vec<ListReportMirror>,
}

#[derive(Debug, Serialize)]
struct ListReportMirror {
    of: String,
    url: String,
    priority: i32,
}

#[derive(Debug, Serialize)]
struct ListReportOverride {
    pkgref: String,
    source_url: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    refname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// Map a host segment to the `RepoCreator` adapter `creator_for_url`
/// would pick. `None` means there is no adapter and `vibe registry
/// publish` would fail with `UnsupportedHost`. Pure read of the
/// dispatch rule in `vibe-publish::creator_for_url`; kept in sync by
/// hand because the rule is short and keeping it in code-as-data
/// would defer the user-facing label here from the rule there for no
/// real win.
fn adapter_for_host(host: &str) -> Option<&'static str> {
    let h = host.to_ascii_lowercase();
    if h == "github.com" || h.ends_with(".github.com") {
        return Some("github");
    }
    if h == "gitverse.ru" || h.ends_with(".gitverse.ru") {
        return Some("gitverse");
    }
    None
}

fn run_list(ctx: &output::Context, args: RegistryListArgs) -> Result<()> {
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

    let mut registries: Vec<ListReportRegistry> = Vec::with_capacity(manifest.registries.len());
    for reg in &manifest.registries {
        let host = extract_host_segment(&reg.url).unwrap_or_else(|_| String::from("?"));
        let org = extract_org_segment(&reg.url).unwrap_or_else(|_| String::from("?"));
        let adapter = adapter_for_host(&host).map(String::from);
        let naming_label = match reg.naming {
            vibe_core::manifest::NamingConvention::KindName => "kind-name",
            vibe_core::manifest::NamingConvention::Name => "name",
            vibe_core::manifest::NamingConvention::KindSlashName => "kind/name",
        }
        .to_string();
        // Mirrors targeted at this registry by name, plus the wildcard
        // `*` mirrors that apply to any registry. `mirrors_for` already
        // returns them sorted by priority ascending.
        let mirrors: Vec<ListReportMirror> = manifest
            .mirrors_for(&reg.name)
            .into_iter()
            .map(|m| ListReportMirror {
                of: m.of.clone(),
                url: m.url.clone(),
                priority: m.priority,
            })
            .collect();
        registries.push(ListReportRegistry {
            name: reg.name.clone(),
            url: reg.url.clone(),
            refname: reg.r#ref.clone(),
            naming: naming_label,
            host,
            org,
            adapter,
            mirrors,
        });
    }

    let all_mirrors: Vec<ListReportMirror> = manifest
        .mirrors
        .iter()
        .map(|m| ListReportMirror {
            of: m.of.clone(),
            url: m.url.clone(),
            priority: m.priority,
        })
        .collect();
    let overrides: Vec<ListReportOverride> = manifest
        .overrides
        .iter()
        .map(|o| ListReportOverride {
            pkgref: o.pkgref.clone(),
            source_url: o.source_url.clone(),
            refname: o.r#ref.clone(),
            reason: o.reason.clone(),
        })
        .collect();

    if ctx.is_json() {
        ctx.emit_json(&ListReport {
            ok: true,
            command: "registry:list",
            registries,
            mirrors: all_mirrors,
            overrides,
        })?;
        return Ok(());
    }

    if registries.is_empty() {
        ctx.summary(
            "No `[[registry]]` entries in `vibe.toml`. Use `--registry <path>` on \
             `vibe install` for a local-directory source, or add a `[[registry]]` block.",
        );
        return Ok(());
    }

    ctx.heading(&format!(
        "Registries ({}; primary listed first)",
        registries.len()
    ));
    for (i, reg) in registries.iter().enumerate() {
        let primary_marker = if i == 0 { " (primary)" } else { "" };
        let adapter_label = reg
            .adapter
            .as_deref()
            .map(|a| format!("adapter: {a}"))
            .unwrap_or_else(|| "adapter: none (publish unsupported)".to_string());
        println!(
            "  {}. {}{}\n     url:     {}\n     org:     {}\n     host:    {} ({})\n     naming:  {}\n     ref:     {}",
            i + 1,
            reg.name,
            primary_marker,
            reg.url,
            reg.org,
            reg.host,
            adapter_label,
            reg.naming,
            reg.refname,
        );
        if reg.mirrors.is_empty() {
            println!("     mirrors: (none)");
        } else {
            println!("     mirrors:");
            for m in &reg.mirrors {
                println!(
                    "       - of=`{}` priority={} url={}",
                    m.of, m.priority, m.url
                );
            }
        }
    }

    if !overrides.is_empty() {
        println!();
        ctx.heading(&format!("Overrides ({})", overrides.len()));
        for o in &overrides {
            let ref_part = o
                .refname
                .as_deref()
                .map(|r| format!("@{r}"))
                .unwrap_or_default();
            let reason_part = o
                .reason
                .as_deref()
                .map(|r| format!(" — {r}"))
                .unwrap_or_default();
            println!(
                "  {} → {}{}{}",
                o.pkgref, o.source_url, ref_part, reason_part
            );
        }
    }

    let mirror_total = registries.iter().map(|r| r.mirrors.len()).sum::<usize>();
    ctx.summary(&format!(
        "\nvibe registry list: {} registries, {} mirror{}, {} override{}.",
        registries.len(),
        mirror_total,
        if mirror_total == 1 { "" } else { "s" },
        overrides.len(),
        if overrides.len() == 1 { "" } else { "s" },
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

    // Pick the host adapter from the registry URL's host segment per
    // PROP-002 §2.10. `creator_for_url` returns a boxed `RepoCreator`
    // already scoped to the configured org; that's the boundary that
    // enforces "never operate outside the configured organization"
    // per PROP-000 §20. Each adapter additionally validates the org
    // at every method call as defence in depth.
    let host = extract_host_segment(&registry_section.url)
        .map_err(|e| anyhow!("registry URL `{}`: {e}", registry_section.url))?;
    let org_segment = extract_org_segment(&registry_section.url)
        .map_err(|e| anyhow!("registry URL `{}`: {e}", registry_section.url))?;

    ctx.heading(&format!(
        "Publishing {} → registry `{}` (`{}`){}",
        source_dir.display(),
        registry_section.name,
        registry_section.url,
        if args.dry_run { " [dry-run]" } else { "" },
    ));

    let token = load_token_for_host(&host).context("loading publish token")?;
    // The CLI surfaces the *source* of the token (env var, file path),
    // never the value. Token::Display redacts to `***` defensively in
    // case any future code path reaches for it.
    ctx.step(&format!(
        "Loaded publish token from {} (value redacted)",
        match token.source() {
            vibe_publish::TokenSource::Explicit => "explicit argument".to_string(),
            vibe_publish::TokenSource::EnvVar(name) => format!("$ {name}"),
            vibe_publish::TokenSource::File(p) => p.display().to_string(),
        }
    ));
    let creator = creator_for_url(&registry_section.url, org_segment, token)
        .map_err(|e| anyhow!("{e}"))?;

    let config = PublishConfig {
        source_dir: source_dir.clone(),
        org_url: registry_section.url.clone(),
        naming: registry_section.naming,
        tag_prefix: "v".to_string(),
        dry_run: args.dry_run,
    };

    let outcome = Publisher::new(creator.as_ref())
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

    let action_verb = if outcome.dry_run {
        if outcome.created_repo {
            "Would create"
        } else {
            "Would reuse existing"
        }
    } else if outcome.created_repo {
        "Created"
    } else {
        "Reusing existing"
    };
    ctx.step(&format!(
        "{} repository `{}` on `{}`",
        action_verb, outcome.repo_name, outcome.host
    ));
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

#[cfg(test)]
mod tests {
    use super::adapter_for_host;

    #[test]
    fn adapter_for_host_picks_github() {
        assert_eq!(adapter_for_host("github.com"), Some("github"));
        assert_eq!(adapter_for_host("api.github.com"), Some("github"));
        assert_eq!(adapter_for_host("GITHUB.com"), Some("github"));
    }

    #[test]
    fn adapter_for_host_picks_gitverse() {
        assert_eq!(adapter_for_host("gitverse.ru"), Some("gitverse"));
        assert_eq!(adapter_for_host("api.gitverse.ru"), Some("gitverse"));
    }

    #[test]
    fn adapter_for_host_returns_none_for_unknown_host() {
        assert_eq!(adapter_for_host("example.invalid"), None);
        assert_eq!(adapter_for_host(""), None);
    }
}
