//! Git operations for the publish flow.
//!
//! Initialise a temp working tree, copy the package contents, set up
//! `origin`, commit, tag, push the tag. Wraps `git` shell-out the same
//! way `vibe-registry`'s `ShellGit` does for consume-side ops, but
//! kept inline here because the publish-side commands aren't on
//! [`vibe_registry::GitBackend`] — that trait is intentionally narrow.
//!
//! Error classification matches PROP-002 §2.10 — push-denied, tag-already-
//! exists, and host-unreachable each produce a distinct
//! [`crate::PublishError`] variant.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::PublishError;

/// Initialise a temp git repo, copy the source dir contents into it,
/// commit on `main`, add `origin`, push (creates the branch upstream),
/// then create the tag and push it.
///
/// Path expectations: `source_dir` contains the package payload at its
/// root. We copy verbatim — no `.git` filtering needed since a freshly
/// authored package is unlikely to carry one, and the bare-init in our
/// staging dir would shadow it anyway.
pub fn push_release(
    source_dir: &Path,
    clone_url: &str,
    tag: &str,
    package_name: &str,
    version: &semver::Version,
) -> Result<(), PublishError> {
    let staging = TempDir::new().map_err(|e| PublishError::Io {
        path: std::env::temp_dir(),
        message: format!("creating publish staging dir: {e}"),
    })?;
    let staging_path = staging.path();

    // Copy package contents into staging.
    copy_dir(source_dir, staging_path)?;

    // git init main + identity (use repo-local config so we don't
    // mutate the user's global git config).
    run_git_in(staging_path, &["init", "--initial-branch=main"])?;
    run_git_in(
        staging_path,
        &["config", "user.email", "publish@vibevm.local"],
    )?;
    run_git_in(staging_path, &["config", "user.name", "vibevm publisher"])?;

    // Stage + commit.
    run_git_in(staging_path, &["add", "-A"])?;
    let commit_msg = format!("Release {package_name}@{version}");
    run_git_in(staging_path, &["commit", "-m", &commit_msg])?;

    // Tag the commit. `-a` annotated so the registry's `ls-remote
    // --tags` peeled-form dedup is exercised on the consumer side.
    let tag_msg = format!("{package_name}@{version}");
    run_git_in(
        staging_path,
        &["tag", "-a", tag, "-m", &tag_msg],
    )?;

    // Wire up origin and push the branch first, then the tag. Two
    // separate pushes because `--mirror` would imply we own every
    // ref on the remote — we don't, and a freshly-created repo has
    // none anyway.
    run_git_in(staging_path, &["remote", "add", "origin", clone_url])?;

    push_with_classification(staging_path, &["push", "-u", "origin", "main"], clone_url)?;
    push_with_classification(staging_path, &["push", "origin", tag], clone_url)?;

    Ok(())
}

/// Recursively copy `src` → `dst`. Skips any `.git/` subtree (defensive;
/// unusual to find one in a publish source dir).
fn copy_dir(src: &Path, dst: &Path) -> Result<(), PublishError> {
    std::fs::create_dir_all(dst).map_err(|e| PublishError::Io {
        path: dst.to_path_buf(),
        message: format!("create_dir_all: {e}"),
    })?;
    for entry in walk(src)? {
        let path = entry;
        let rel = path
            .strip_prefix(src)
            .expect("walk yields paths under src")
            .to_path_buf();
        if rel
            .components()
            .any(|c| c.as_os_str() == std::ffi::OsStr::new(".git"))
        {
            continue;
        }
        let target = dst.join(&rel);
        if path.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| PublishError::Io {
                path: target.clone(),
                message: format!("create_dir_all: {e}"),
            })?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| PublishError::Io {
                    path: parent.to_path_buf(),
                    message: format!("create_dir_all: {e}"),
                })?;
            }
            std::fs::copy(&path, &target).map_err(|e| PublishError::Io {
                path: target.clone(),
                message: format!("copy: {e}"),
            })?;
        }
    }
    Ok(())
}

/// Manual recursive walk; avoids pulling `walkdir` into this crate's
/// runtime deps for one helper.
fn walk(root: &Path) -> Result<Vec<std::path::PathBuf>, PublishError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            let entries = std::fs::read_dir(&p).map_err(|e| PublishError::Io {
                path: p.clone(),
                message: format!("read_dir: {e}"),
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| PublishError::Io {
                    path: p.clone(),
                    message: format!("read_dir entry: {e}"),
                })?;
                let path = entry.path();
                stack.push(path.clone());
                if path.is_file() {
                    out.push(path);
                }
            }
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(out)
}

fn run_git_in(cwd: &Path, args: &[&str]) -> Result<Output, PublishError> {
    let output = git_command(cwd, args).output().map_err(|e| PublishError::Git(format!(
        "spawning git {}: {e}",
        args.join(" ")
    )))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PublishError::Git(format!(
            "git {} failed: {stderr}",
            args.join(" ")
        )));
    }
    Ok(output)
}

/// Like [`run_git_in`] but maps `git push` failures onto the structured
/// PROP-002 error variants the operator sees.
fn push_with_classification(
    cwd: &Path,
    args: &[&str],
    clone_url: &str,
) -> Result<(), PublishError> {
    let output = git_command(cwd, args).output().map_err(|e| PublishError::Git(format!(
        "spawning git {}: {e}",
        args.join(" ")
    )))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("permission denied")
        || stderr.contains("publickey")
        || stderr.contains("authentication failed")
        || stderr.contains("403")
    {
        return Err(PublishError::PushDenied {
            repo: clone_url.to_string(),
        });
    }
    if stderr.contains("could not resolve host")
        || stderr.contains("network is unreachable")
        || stderr.contains("could not read from remote repository")
    {
        return Err(PublishError::HostUnreachable {
            host: clone_url.to_string(),
        });
    }
    if stderr.contains("already exists") && (stderr.contains("tag") || stderr.contains("ref")) {
        // Pull the tag out of args for a useful message.
        let tag = args
            .iter()
            .rev()
            .find(|a| a.starts_with('v') || a.contains('.'))
            .copied()
            .unwrap_or("<unknown>")
            .to_string();
        return Err(PublishError::TagCollision {
            repo: clone_url.to_string(),
            tag,
        });
    }
    Err(PublishError::Git(format!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn git_command(cwd: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd.current_dir(cwd);
    cmd.env("LC_ALL", "C").env("LANG", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn copy_dir_skips_dot_git_subtrees() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::create_dir_all(src.path().join(".git/objects")).unwrap();
        fs::write(src.path().join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        fs::write(src.path().join("README.md"), "hi").unwrap();
        fs::write(
            src.path().join("vibe-package.toml"),
            "[package]\nname = \"x\"\nkind = \"flow\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        copy_dir(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("README.md").exists());
        assert!(dst.path().join("vibe-package.toml").exists());
        assert!(!dst.path().join(".git").exists());
        assert!(!dst.path().join(".git/HEAD").exists());
    }

    #[test]
    fn push_release_against_local_bare_origin() {
        if !git_available() {
            eprintln!("skipping: git not on PATH");
            return;
        }

        // Build a bare origin we can push to.
        let outer = tempdir().unwrap();
        let bare = outer.path().join("origin.git");
        let init_status = Command::new("git")
            .args(["init", "--bare", bare.to_str().unwrap()])
            .env("LC_ALL", "C")
            .status()
            .unwrap();
        assert!(init_status.success());

        // Build a fake source dir with a manifest + spec file.
        let src = tempdir().unwrap();
        fs::write(
            src.path().join("vibe-package.toml"),
            "[package]\nname = \"wal\"\nkind = \"flow\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::create_dir_all(src.path().join("spec")).unwrap();
        fs::write(src.path().join("spec/PROTOCOL.md"), "...").unwrap();

        let url = bare.to_string_lossy().into_owned();
        let v = semver::Version::parse("0.1.0").unwrap();
        push_release(src.path(), &url, "v0.1.0", "wal", &v).expect("push ok");

        // Inspect the bare repo: tag and main branch should both be there.
        let tags = Command::new("git")
            .args(["-C", bare.to_str().unwrap(), "tag", "--list"])
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        let tag_list = String::from_utf8_lossy(&tags.stdout);
        assert!(
            tag_list.contains("v0.1.0"),
            "expected v0.1.0 in tags, got: {tag_list}"
        );

        let branches = Command::new("git")
            .args(["-C", bare.to_str().unwrap(), "branch", "--list"])
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&branches.stdout).contains("main"),
            "expected main branch in bare origin"
        );
    }

    // Tag-collision classification is exercised via the substring
    // matcher in `push_with_classification`. End-to-end testing of the
    // collision case is awkward because publishing two distinct package
    // trees to the same bare origin fails on the `main` branch
    // non-fast-forward before reaching the tag push. The collision
    // path is best validated against a real registry; that's part of
    // the live-migration smoke-test in the next commit.
}
