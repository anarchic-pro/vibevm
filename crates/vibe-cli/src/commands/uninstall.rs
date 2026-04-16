//! `vibe uninstall <kind>:<name>` — remove an installed package.
//!
//! Spec: `VIBEVM-SPEC.md` §9.1, §11.1.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use serde::Serialize;
use vibe_core::PackageRef;
use vibe_core::manifest::Lockfile;
use vibe_install::{InstallError, apply_uninstall, plan_uninstall, unregister_installed};

use crate::cli::UninstallArgs;
use crate::output;

pub fn run(ctx: &output::Context, args: UninstallArgs) -> Result<()> {
    let project_root = resolve_project_root(&args.path)?;
    let mut lockfile = load_lockfile(&project_root)?;

    let pkgref = PackageRef::parse(&args.package)
        .with_context(|| format!("parsing `{}`", args.package))?;

    let plan = plan_uninstall(&project_root, &lockfile, &pkgref)?;

    if !ctx.is_json() && !ctx.is_quiet() {
        ctx.heading(&format!(
            "\nPlan for uninstall {}:{}@{}",
            plan.kind, plan.name, plan.version
        ));
        for rel in &plan.removed_paths {
            println!("  remove  {}", rel.to_string_lossy().replace('\\', "/"));
        }
        println!();
    }

    let approved = if args.assume_yes || ctx.is_json() {
        true
    } else if !console::user_attended() {
        bail!(
            "no TTY available for confirmation; re-run with `--assume-yes` to uninstall non-interactively"
        );
    } else {
        Confirm::new()
            .with_prompt(format!(
                "Remove {} file{} for {}:{}?",
                plan.removed_paths.len(),
                if plan.removed_paths.len() == 1 { "" } else { "s" },
                plan.kind,
                plan.name,
            ))
            .default(false)
            .interact()
            .context("reading user confirmation")?
    };
    if !approved {
        return Err(InstallError::UserDeclined.into());
    }

    let removed = apply_uninstall(&plan)?;
    let _entry = unregister_installed(
        &mut lockfile,
        &pkgref,
        crate::commands::init::current_timestamp_utc(),
    )?;
    lockfile.write(project_root.join(Lockfile::FILENAME))?;

    emit_report(ctx, &plan.kind.to_string(), &plan.name, &plan.version.to_string(), &removed)
}

#[derive(Debug, Serialize)]
struct UninstallReport {
    ok: bool,
    command: &'static str,
    package: String,
    version: String,
    removed_count: usize,
    paths: Vec<String>,
}

fn emit_report(
    ctx: &output::Context,
    kind: &str,
    name: &str,
    version: &str,
    removed: &[std::path::PathBuf],
) -> Result<()> {
    let paths: Vec<String> = removed
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();

    if ctx.is_json() {
        let report = UninstallReport {
            ok: true,
            command: "uninstall",
            package: format!("{kind}:{name}"),
            version: version.to_string(),
            removed_count: paths.len(),
            paths,
        };
        ctx.emit_json(&report)?;
        return Ok(());
    }
    if ctx.is_quiet() {
        ctx.summary(&format!(
            "vibe uninstall: {kind}:{name}@{version}, {} file{} removed",
            paths.len(),
            if paths.len() == 1 { "" } else { "s" }
        ));
        return Ok(());
    }
    for p in &paths {
        ctx.removed(p);
    }
    ctx.summary(&format!(
        "\nUninstalled {kind}:{name}@{version} ({} file{}).",
        paths.len(),
        if paths.len() == 1 { "" } else { "s" }
    ));
    Ok(())
}

fn resolve_project_root(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("canonicalizing `{}`", path.display()))?;
    let stripped = super::init::strip_unc_public(canonical);
    if !stripped.join("vibe.toml").exists() {
        bail!(
            "no `vibe.toml` in `{}`; run `vibe init` first",
            stripped.display()
        );
    }
    Ok(stripped)
}

fn load_lockfile(root: &Path) -> Result<Lockfile> {
    let path = root.join(Lockfile::FILENAME);
    Ok(Lockfile::read(&path)?)
}
