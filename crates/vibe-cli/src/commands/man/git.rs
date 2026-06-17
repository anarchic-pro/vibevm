//! Thin wrappers over the system `git`, used by the build pipeline
//! (PROP-019 §2.7). vibevm shells out to the user's installed git rather
//! than linking a git library — matching the project's existing tooling and
//! honouring the user's own credentials and host-key config (§2.13).

specmark::scope!("spec://vibevm/common/PROP-019#build");

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Run `git <args>` in `dir`, returning trimmed stdout. A non-zero exit is
/// an error carrying git's stderr.
pub fn run(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .with_context(|| format!("spawning `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Resolve a revision to its full commit hash.
pub fn rev_parse(dir: &Path, rev: &str) -> Result<String> {
    run(dir, &["rev-parse", rev])
}

/// The current branch name, or `None` when HEAD is detached.
pub fn current_branch(dir: &Path) -> Option<String> {
    let name = run(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
    if name == "HEAD" || name.is_empty() {
        None
    } else {
        Some(name)
    }
}
