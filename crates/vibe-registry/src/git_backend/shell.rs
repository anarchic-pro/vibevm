//! [`GitBackend`] implementation that shells out to the system `git`.
//!
//! Design decisions pinned in
//! [`spec/modules/vibe-registry/PROP-001-git-backend.md`][prop]:
//!
//! - Every spawn is wrapped with `LC_ALL=C` / `LANG=C` so stderr parsing
//!   is locale-invariant.
//! - On Windows, `CREATE_NO_WINDOW` is set on every spawn so a
//!   hostless parent process never flashes a console window.
//! - Error classification is substring-based against the stable
//!   C-locale stderr.
//!
//! [prop]: ../../../../../spec/modules/vibe-registry/PROP-001-git-backend.md

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use super::{GitBackend, GitError};

/// `git` subprocess backend.
///
/// Safe to share across threads: every operation constructs a fresh
/// [`Command`] and blocks on its output. The only shared state is the
/// immutable path to the `git` binary.
#[derive(Debug)]
pub struct ShellGit {
    binary: PathBuf,
    // Cached preflight result — populated on first `preflight()` call
    // for this instance. Kept per-instance (not global) so tests with
    // bogus binaries do not poison the cache for instances pointing at
    // a real `git`.
    preflight_cache: OnceLock<bool>,
}

impl Default for ShellGit {
    fn default() -> Self {
        ShellGit::new()
    }
}

impl ShellGit {
    /// Construct a [`ShellGit`] bound to `git` on `PATH`.
    ///
    /// Does **not** preflight — the cost is deferred to the first real
    /// operation, which fails fast with [`GitError::NotInstalled`] if
    /// git cannot be spawned. Call [`ShellGit::preflight`] once per
    /// `vibe` invocation to turn that into an up-front error.
    pub fn new() -> Self {
        // Allow `VIBE_GIT_BINARY` to override PATH lookup. See PROP-001 §6
        // for the rationale on env-var rather than CLI-flag.
        let binary = std::env::var_os("VIBE_GIT_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("git"));
        ShellGit {
            binary,
            preflight_cache: OnceLock::new(),
        }
    }

    /// Verify that the configured git binary responds to `--version`.
    ///
    /// Caches the result on this instance. Subsequent calls are free.
    pub fn preflight(&self) -> Result<(), GitError> {
        let ok = *self.preflight_cache.get_or_init(|| {
            let mut cmd = Command::new(&self.binary);
            cmd.arg("--version");
            apply_common_env(&mut cmd);
            cmd.output().map(|o| o.status.success()).unwrap_or(false)
        });
        if ok { Ok(()) } else { Err(GitError::NotInstalled) }
    }

    fn run(&self, args: &[&str], cwd: Option<&Path>) -> Result<Output, GitError> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(args);
        if let Some(d) = cwd {
            cmd.current_dir(d);
        }
        apply_common_env(&mut cmd);

        tracing::debug!(target: "vibe_registry::git", argv = ?render_argv(&self.binary, args), cwd = ?cwd, "running git");

        let output = cmd.output().map_err(|e| GitError::Io {
            cmd: render_argv(&self.binary, args),
            source: e,
        })?;
        if output.status.success() {
            return Ok(output);
        }
        Err(classify_failure(args, &output))
    }
}

impl GitBackend for ShellGit {
    fn clone(&self, url: &str, refname: &str, dest: &Path) -> Result<(), GitError> {
        self.preflight()?;
        let dest_s = dest.to_string_lossy();
        let args = [
            "clone",
            "--branch",
            refname,
            "--",
            url,
            dest_s.as_ref(),
        ];
        self.run(&args, None).map(|_| ())
    }

    fn update(&self, dest: &Path, refname: &str) -> Result<(), GitError> {
        self.preflight()?;
        // Fetch from origin, then hard-reset the working tree to the
        // fetched tip. `--hard` is intentional: the registry cache is a
        // read-only mirror and we want it to match upstream exactly,
        // never a surprise merge commit.
        self.run(&["fetch", "--prune", "origin"], Some(dest))?;
        let ref_arg = format!("origin/{refname}");
        self.run(&["reset", "--hard", &ref_arg], Some(dest))
            .map(|_| ())
    }
}

fn apply_common_env(cmd: &mut Command) {
    cmd.env("LC_ALL", "C").env("LANG", "C");

    // Never ask the user for interactive auth — the registry is either
    // public (no auth needed) or uses a configured ssh/credential
    // helper. A prompt would hang CI and non-TTY invocations.
    cmd.env("GIT_TERMINAL_PROMPT", "0");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — if `vibe` is ever spawned from a hostless
        // parent (GUI, service, IDE plugin), child git must not flash
        // a console window.
        cmd.creation_flags(0x0800_0000);
    }
}

fn classify_failure(args: &[&str], output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    let lc = combined.to_lowercase();

    // Extract `--` followed by URL for clone. For fetch, URL is from
    // origin which we don't know here; fall back to an empty string.
    let url = args
        .iter()
        .skip_while(|a| **a != "--")
        .nth(1)
        .map(|s| s.to_string())
        .unwrap_or_default();

    let refname = args
        .iter()
        .skip_while(|a| **a != "--branch")
        .nth(1)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if lc.contains("repository not found") || lc.contains("does not appear to be a git repository")
    {
        return GitError::RepoNotFound { url };
    }
    if lc.contains("permission denied (publickey)") || lc.contains("authentication failed") {
        return GitError::AuthFailed { url };
    }
    if lc.contains("could not resolve host")
        || lc.contains("could not read from remote repository")
        || lc.contains("network is unreachable")
    {
        return GitError::NetworkUnreachable { url };
    }
    if lc.contains("remote branch")
        && lc.contains("not found")
        || lc.contains("couldn't find remote ref")
    {
        return GitError::RefNotFound { url, refname };
    }

    GitError::CommandFailed {
        cmd: render_argv_for_display(args),
        status: output.status.code().unwrap_or(-1),
        stderr: combined.trim_end().to_string(),
    }
}

fn render_argv(binary: &Path, args: &[&str]) -> String {
    let mut out = OsString::from(binary);
    for a in args {
        out.push(" ");
        out.push(a);
    }
    out.to_string_lossy().into_owned()
}

fn render_argv_for_display(args: &[&str]) -> String {
    let mut out = String::from("git");
    for a in args {
        out.push(' ');
        out.push_str(a);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// Build a bare git repo under `root/origin.git` seeded with one
    /// commit on `main`, and return its absolute path. Requires `git`
    /// on `PATH`; tests that need it skip themselves via
    /// `skip_without_git!()` below.
    fn make_bare_origin(root: &Path) -> PathBuf {
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        // Work repo: init, set identity, create a file, commit on main.
        run_or_panic(&src, &["init", "--initial-branch=main"]);
        run_or_panic(&src, &["config", "user.email", "t@example.com"]);
        run_or_panic(&src, &["config", "user.name", "Test"]);
        fs::write(src.join("README.md"), "hello\n").unwrap();
        run_or_panic(&src, &["add", "README.md"]);
        run_or_panic(&src, &["commit", "-m", "init"]);

        let bare = root.join("origin.git");
        run_or_panic(root, &[
            "clone", "--bare", src.to_str().unwrap(), bare.to_str().unwrap(),
        ]);
        // Make sure HEAD in the bare repo points at main.
        run_or_panic(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]);

        bare
    }

    fn run_or_panic(cwd: &Path, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.args(args);
        cmd.current_dir(cwd);
        apply_common_env(&mut cmd);
        let out = cmd.output().expect("failed to spawn git for test setup");
        if !out.status.success() {
            panic!(
                "test setup `git {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    macro_rules! skip_without_git {
        () => {
            if !git_available() {
                eprintln!("skipping test: git not on PATH");
                return;
            }
        };
    }

    #[test]
    fn preflight_succeeds_when_git_installed() {
        skip_without_git!();
        let g = ShellGit::new();
        g.preflight().expect("preflight should succeed");
    }

    #[test]
    fn preflight_reports_not_installed_for_bogus_binary() {
        let g = ShellGit {
            binary: PathBuf::from("definitely-not-git-xyz"),
            preflight_cache: OnceLock::new(),
        };
        let err = g.preflight().unwrap_err();
        assert!(
            matches!(err, GitError::NotInstalled),
            "expected NotInstalled, got: {err:?}"
        );
    }

    #[test]
    fn clone_then_update_against_bare_origin() {
        skip_without_git!();
        let tmp = tempdir().unwrap();
        let bare = make_bare_origin(tmp.path());
        let dest = tmp.path().join("clone");

        let g = ShellGit::new();
        g.clone(&bare.to_string_lossy(), "main", &dest)
            .expect("initial clone should succeed");
        assert!(dest.join("README.md").exists());

        // Push a new commit into origin, then update from the clone.
        let src2 = tmp.path().join("src2");
        run_or_panic(tmp.path(), &[
            "clone", bare.to_str().unwrap(), src2.to_str().unwrap(),
        ]);
        run_or_panic(&src2, &["config", "user.email", "t@example.com"]);
        run_or_panic(&src2, &["config", "user.name", "Test"]);
        fs::write(src2.join("new.md"), "new\n").unwrap();
        run_or_panic(&src2, &["add", "new.md"]);
        run_or_panic(&src2, &["commit", "-m", "add new"]);
        run_or_panic(&src2, &["push", "origin", "main"]);

        g.update(&dest, "main").expect("update should succeed");
        assert!(dest.join("new.md").exists());
    }

    #[test]
    fn clone_reports_repo_not_found_for_missing_url() {
        skip_without_git!();
        let tmp = tempdir().unwrap();
        let bogus = tmp.path().join("does/not/exist.git");
        let dest = tmp.path().join("clone");

        let g = ShellGit::new();
        let err = g.clone(&bogus.to_string_lossy(), "main", &dest).unwrap_err();
        // The exact message varies by git version; classification should
        // land on either RepoNotFound or a generic CommandFailed with
        // the raw stderr — both are acceptable for this test.
        match err {
            GitError::RepoNotFound { .. } | GitError::CommandFailed { .. } => {}
            other => panic!("unexpected classification: {other:?}"),
        }
    }

    #[test]
    fn clone_reports_ref_not_found_for_missing_branch() {
        skip_without_git!();
        let tmp = tempdir().unwrap();
        let bare = make_bare_origin(tmp.path());
        let dest = tmp.path().join("clone");

        let g = ShellGit::new();
        let err = g.clone(&bare.to_string_lossy(), "no-such-branch", &dest).unwrap_err();
        match err {
            GitError::RefNotFound { .. } | GitError::CommandFailed { .. } => {}
            other => panic!("unexpected classification: {other:?}"),
        }
    }
}
