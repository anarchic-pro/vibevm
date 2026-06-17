//! `vibe man` — the VibeVM Version Manager (VVM): build, install, switch,
//! and remove vibevm's own versions on this machine (PROP-019). A
//! standalone-mode capability — pure algorithm, no LLM (PROP-019 §2.1).
//!
//! This slice implements the read-only introspection verbs (`ls`,
//! `current`, `which`) over the [`store::VersionStore`] inventory and the
//! `VIBEVM_HOME`-named active version (PROP-019 §2.5, §2.11).

specmark::scope!("spec://vibevm/common/PROP-019#surface");

mod model;
mod store;

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::cli::{ManArgs, ManSubcommand};
use crate::output;

use store::VersionStore;

/// Env var naming the VVM root (defaults to `~/opt`); read at the
/// composition root (PROP-019 §2.4).
pub const VIBEVM_ROOT_ENV: &str = "VIBEVM_ROOT";
/// Env var naming the active version's prefix — the single source of truth
/// for "which version is active" (PROP-019 §2.5).
pub const VIBEVM_HOME_ENV: &str = "VIBEVM_HOME";

/// Ambient environment VVM needs, resolved at the composition root
/// (`main.rs`) and threaded in — the domain never reads the process env
/// itself (PROP-019 §2.1).
#[derive(Debug, Clone, Default)]
pub struct ManEnv {
    /// Resolved `$VIBEVM_ROOT`, or the `~/opt` default. `None` only when
    /// neither an override nor a home directory is available.
    pub root: Option<PathBuf>,
    /// `$VIBEVM_HOME` — the active version's prefix (PROP-019 §2.5).
    pub active_home: Option<PathBuf>,
}

impl ManEnv {
    fn store(&self) -> Result<VersionStore> {
        let root = self.root.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "cannot determine the VVM root: set $VIBEVM_ROOT, or ensure a home directory exists"
            )
        })?;
        Ok(VersionStore::new(root))
    }
}

pub fn run(ctx: &output::Context, args: ManArgs, env: ManEnv) -> Result<()> {
    match args.command {
        ManSubcommand::Ls => run_ls(ctx, &env),
        ManSubcommand::Current => run_current(ctx, &env),
        ManSubcommand::Which => run_which(ctx, &env),
    }
}

/// Abbreviate a commit hash for display, byte-safe (commits are ASCII hex).
fn short_commit(c: &str) -> &str {
    &c[..c.len().min(10)]
}

fn run_ls(ctx: &output::Context, env: &ManEnv) -> Result<()> {
    let store = env.store()?;
    let state = store.load_state()?;
    let active_id = store
        .active(env.active_home.as_deref())?
        .map(|r| r.version_id());

    if ctx.is_json() {
        let installs: Vec<serde_json::Value> = state
            .installs
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.version_id().to_string(),
                    "commit": r.commit,
                    "toolchain": r.toolchain,
                    "profile": r.profile,
                    "installed_at": r.installed_at,
                    "active": Some(r.version_id()) == active_id,
                })
            })
            .collect();
        return ctx.emit_json(&serde_json::json!({
            "ok": true,
            "command": "man:ls",
            "active": active_id.as_ref().map(|i| i.to_string()),
            "count": installs.len(),
            "installs": installs,
        }));
    }

    if state.installs.is_empty() {
        ctx.summary("(no versions installed — run `vibe man install latest`)");
        return Ok(());
    }
    for r in &state.installs {
        let marker = if Some(r.version_id()) == active_id {
            "*"
        } else {
            " "
        };
        ctx.step(&format!(
            "{marker} {}  {}  {}  {}",
            r.version_id(),
            short_commit(&r.commit),
            r.profile,
            r.installed_at
        ));
    }
    ctx.summary(&format!("{} version(s) installed.", state.installs.len()));
    Ok(())
}

fn run_current(ctx: &output::Context, env: &ManEnv) -> Result<()> {
    let store = env.store()?;
    let active = store.active(env.active_home.as_deref())?;
    if ctx.is_json() {
        return ctx.emit_json(&serde_json::json!({
            "ok": true,
            "command": "man:current",
            "active": active.as_ref().map(|r| r.version_id().to_string()),
        }));
    }
    match active {
        Some(r) => ctx.summary(&r.version_id().to_string()),
        None => ctx.summary("(no active version)"),
    }
    Ok(())
}

fn run_which(ctx: &output::Context, env: &ManEnv) -> Result<()> {
    let store = env.store()?;
    let Some(record) = store.active(env.active_home.as_deref())? else {
        bail!("no active version (run `vibe man use <selector>`)");
    };
    let path = store.binary_path(&record.version_id());
    if ctx.is_json() {
        return ctx.emit_json(&serde_json::json!({
            "ok": true,
            "command": "man:which",
            "path": path.display().to_string(),
        }));
    }
    ctx.summary(&path.display().to_string());
    Ok(())
}
