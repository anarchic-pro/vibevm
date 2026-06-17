//! `vibe man` — the VibeVM Version Manager (VVM): build, install, switch,
//! and remove vibevm's own versions on this machine (PROP-019). A
//! standalone-mode capability — pure algorithm, no LLM (PROP-019 §2.1).
//!
//! This slice implements the read-only introspection verbs (`ls`,
//! `current`, `which`) over the [`store::VersionStore`] inventory and the
//! `VIBEVM_HOME`-named active version (PROP-019 §2.5, §2.11).

specmark::scope!("spec://vibevm/common/PROP-019#surface");

mod git;
mod install;
mod model;
mod store;

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::cli::{ManArgs, ManInstallArgs, ManSubcommand};
use crate::output;

use store::VersionStore;

/// Env var naming the install base (defaults to the user's home dir); the
/// VVM root is `$VIBEVM_INSTALL_ROOT/opt`. Read at the composition root and
/// overridden in tests to isolate installs under a temp dir (PROP-019 §2.4).
pub const VIBEVM_INSTALL_ROOT_ENV: &str = "VIBEVM_INSTALL_ROOT";
/// Env var naming the active version's prefix — the single source of truth
/// for "which version is active" (PROP-019 §2.5).
pub const VIBEVM_HOME_ENV: &str = "VIBEVM_HOME";

/// Ambient environment VVM needs, resolved at the composition root
/// (`main.rs`) and threaded in — the domain never reads the process env
/// itself (PROP-019 §2.1).
#[derive(Debug, Clone, Default)]
pub struct ManEnv {
    /// The resolved VVM root — `$VIBEVM_INSTALL_ROOT/opt`, defaulting to
    /// `~/opt`. `None` only when neither an override nor a home directory is
    /// available.
    pub root: Option<PathBuf>,
    /// `$VIBEVM_HOME` — the active version's prefix (PROP-019 §2.5).
    pub active_home: Option<PathBuf>,
    /// The current working directory — for in-tree source detection on
    /// `man install` (PROP-019 §2.7).
    pub cwd: Option<PathBuf>,
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
        ManSubcommand::Install(a) => run_install_cmd(ctx, &env, a),
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

fn run_install_cmd(ctx: &output::Context, env: &ManEnv, args: ManInstallArgs) -> Result<()> {
    let store = env.store()?;
    let profile = resolve_profile(&args)?;
    let selector = model::Selector::parse(&args.selector, forced_kind(&args))?;
    if selector != model::Selector::Latest {
        bail!(
            "selecting a specific ref (`{}`) needs the clone path, which lands in a later \
             slice; in-tree `vibe man install` builds the current checkout (selector `latest`)",
            args.selector
        );
    }
    let cwd = env
        .cwd
        .clone()
        .ok_or_else(|| anyhow::anyhow!("cannot determine the current directory"))?;
    let Some(root) = install::find_source_root(&cwd) else {
        bail!(
            "not inside a vibevm source tree.\nThe clone-based install lands in a later slice; \
             for now, from a checkout:\n  git clone <mirror> && cd vibevm && \
             cargo run -p vibe-cli -- man install"
        );
    };
    let resolved = install::label_in_tree(&root)?;
    let now = chrono::Utc::now().to_rfc3339();
    let req = install::InstallRequest {
        resolved: &resolved,
        profile,
        force: args.force,
        now: &now,
    };
    install::perform_install(ctx, &store, &root, &req, &install::CargoBuilder)
}

fn resolve_profile(args: &ManInstallArgs) -> Result<model::Profile> {
    if args.release {
        return Ok(model::Profile::Release);
    }
    match &args.profile {
        Some(p) => model::Profile::parse(p),
        None => Ok(model::DEFAULT_PROFILE),
    }
}

fn forced_kind(args: &ManInstallArgs) -> Option<model::Kind> {
    if args.tag {
        Some(model::Kind::Tag)
    } else if args.branch {
        Some(model::Kind::Branch)
    } else if args.commit {
        Some(model::Kind::Commit)
    } else {
        None
    }
}
