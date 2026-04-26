//! `cargo xtask` — project-tooling entry point.
//!
//! Subcommands:
//!
//! - `codegen` — regenerate every Rust file under
//!   `crates/vibe-wire/src/generated/` from the JTD schemas under
//!   `schemas/`. Calls the locally-vendored `jtd-codegen` binary
//!   (see `tools/jtd-codegen/README.md`); errors actionably when
//!   the binary is missing.
//! - `check-codegen` — `codegen`, then run `git diff --exit-code` over
//!   the generated dir. Used by CI to assert no schema drift.
//!
//! Entry shape follows the standard `xtask` pattern. Keep this
//! crate dep-light: clap + anyhow + std. Anything heavier belongs in
//! a regular workspace crate.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "xtask",
    about = "vibevm project tooling — codegen, drift checks, build helpers"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Regenerate Rust types under `crates/vibe-wire/src/generated/`
    /// from JTD schemas under `schemas/`.
    Codegen,

    /// Run `codegen`, then assert via `git diff --exit-code` that the
    /// generated tree matches what's checked in. CI runs this to catch
    /// schema drift.
    CheckCodegen,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Codegen => run_codegen(),
        Cmd::CheckCodegen => run_check_codegen(),
    }
}

fn repo_root() -> Result<PathBuf> {
    // `cargo xtask` runs the binary from the workspace root by
    // default, but be defensive: walk up from CARGO_MANIFEST_DIR
    // (which is `<root>/xtask`) to find the workspace root.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set; is xtask running under cargo?")?;
    let manifest_dir = PathBuf::from(manifest_dir);
    let parent = manifest_dir
        .parent()
        .context("xtask manifest dir has no parent")?;
    Ok(parent.to_path_buf())
}

/// Locate the jtd-codegen binary. Prefer the project-local copy under
/// `tools/jtd-codegen/`; fall back to PATH if the local copy is
/// absent so contributors who chose a system-wide install still work.
fn find_jtd_codegen(root: &Path) -> Result<PathBuf> {
    let exe = if cfg!(windows) {
        "jtd-codegen.exe"
    } else {
        "jtd-codegen"
    };
    let local = root.join("tools").join("jtd-codegen").join(exe);
    if local.exists() {
        return Ok(local);
    }
    // Fall back to PATH lookup.
    let probe = Command::new(exe).arg("--version").output();
    match probe {
        Ok(out) if out.status.success() => Ok(PathBuf::from(exe)),
        _ => bail!(
            "jtd-codegen not found. Looked at:\n  \
             1. {} (project-local, preferred)\n  \
             2. `{exe}` on PATH (fallback)\n\n\
             Install per `tools/jtd-codegen/README.md`. PROP-000 §16 \
             pins the JTD + codegen toolchain as project-local; the PATH \
             fallback is a courtesy for contributors who already have \
             it installed system-wide.",
            local.display()
        ),
    }
}

fn run_codegen() -> Result<()> {
    let root = repo_root()?;
    let schemas_dir = root.join("schemas");
    let out_dir = root.join("crates/vibe-wire/src/generated");

    if !schemas_dir.exists() {
        bail!("`schemas/` directory not found at {}", schemas_dir.display());
    }
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating output dir {}", out_dir.display()))?;

    let binary = find_jtd_codegen(&root)?;

    // Find every `*.jtd.json` under `schemas/`.
    let schemas: Vec<PathBuf> = std::fs::read_dir(&schemas_dir)
        .with_context(|| format!("reading {}", schemas_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".jtd.json"))
                    .unwrap_or(false)
        })
        .collect();

    if schemas.is_empty() {
        eprintln!(
            "no `*.jtd.json` schemas under `{}` — nothing to do.",
            schemas_dir.display()
        );
        return Ok(());
    }

    eprintln!(
        "xtask codegen: {} schema{} → {}",
        schemas.len(),
        if schemas.len() == 1 { "" } else { "s" },
        out_dir.display()
    );

    for schema in &schemas {
        eprintln!("  - {}", schema.display());
        let status = Command::new(&binary)
            .arg("--rust-out")
            .arg(&out_dir)
            .arg(schema)
            .status()
            .with_context(|| format!("spawning {}", binary.display()))?;
        if !status.success() {
            bail!(
                "jtd-codegen failed for `{}` (exit code {:?})",
                schema.display(),
                status.code()
            );
        }
    }

    eprintln!("xtask codegen: ok.");
    Ok(())
}

fn run_check_codegen() -> Result<()> {
    run_codegen()?;
    let root = repo_root()?;
    let out_dir = root.join("crates/vibe-wire/src/generated");
    let status = Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .arg(&out_dir)
        .current_dir(&root)
        .status()
        .context("spawning git diff")?;
    if !status.success() {
        bail!(
            "generated code under `{}` is out of date relative to `schemas/`. \
             Run `cargo xtask codegen` and commit the result.",
            out_dir.display()
        );
    }
    eprintln!("xtask check-codegen: clean.");
    Ok(())
}
