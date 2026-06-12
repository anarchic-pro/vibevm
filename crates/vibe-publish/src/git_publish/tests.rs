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
        src.path().join("vibe.toml"),
        "[package]\ngroup = \"org.vibevm\"\nname = \"x\"\nkind = \"flow\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    copy_dir(src.path(), dst.path()).unwrap();

    assert!(dst.path().join("README.md").exists());
    assert!(dst.path().join("vibe.toml").exists());
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
        src.path().join("vibe.toml"),
        "[package]\ngroup = \"org.vibevm\"\nname = \"wal\"\nkind = \"flow\"\nversion = \"0.1.0\"\n",
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

#[test]
fn redact_credentials_hides_user_info() {
    let url = "https://x-access-token:abcd1234@github.com/vibespecs/flow-wal.git";
    let scrubbed = redact_credentials(url);
    assert_eq!(
        scrubbed, "https://***@github.com/vibespecs/flow-wal.git",
        "credentials must be replaced with `***`"
    );
    assert!(!scrubbed.contains("abcd1234"));
}

#[test]
fn redact_credentials_passthrough_when_no_credentials() {
    let url = "https://github.com/vibespecs/flow-wal.git";
    assert_eq!(redact_credentials(url), url);
}

#[test]
fn redact_credentials_handles_ssh_no_scheme() {
    // SSH shorthand `git@host:path` does not match the userinfo
    // pattern (no `://`); pass-through is correct here because
    // this form has no embedded password to hide.
    let url = "git@github.com:vibespecs/flow-wal.git";
    assert_eq!(redact_credentials(url), url);
}

#[test]
fn redact_credentials_handles_ssh_scheme() {
    let url = "ssh://git@github.com/vibespecs/flow-wal.git";
    // `ssh://git@github.com/...` has user `git` but no password —
    // the helper still scrubs it to be safe (consistent with the
    // PROP-000 §20 "never any credential-like token in output"
    // posture). Operators that genuinely needed to see "git" can
    // read the registry URL from `vibe.toml`.
    assert_eq!(
        redact_credentials(url),
        "ssh://***@github.com/vibespecs/flow-wal.git"
    );
}

#[test]
fn redact_credentials_within_message() {
    let msg =
        "git remote add origin https://x-access-token:secret@github.com/foo/bar.git failed: oops";
    let scrubbed = redact_credentials(msg);
    assert!(!scrubbed.contains("secret"));
    assert!(scrubbed.contains("https://***@github.com/foo/bar.git"));
    assert!(scrubbed.contains("failed: oops"));
}

#[test]
fn commit_and_push_lands_local_edit_on_bare_origin() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }

    // Build a bare origin, seed it via push_initial so it has a
    // `main` HEAD, then clone, edit a file, and exercise
    // commit_and_push. Mirrors the real workflow of
    // `vibe registry redirect-update`.
    let outer = tempdir().unwrap();
    let bare = outer.path().join("origin.git");
    let init_status = Command::new("git")
        .args(["init", "--bare", bare.to_str().unwrap()])
        .env("LC_ALL", "C")
        .status()
        .unwrap();
    assert!(init_status.success());

    let seed = tempdir().unwrap();
    fs::write(
        seed.path().join("vibe-redirect.toml"),
        "[redirect]\ntarget_url = \"https://example.invalid/v1\"\n",
    )
    .unwrap();
    let url = bare.to_string_lossy().into_owned();
    push_initial(seed.path(), &url, "stub: initial").expect("seed ok");

    let work = shallow_clone(&url).expect("clone ok");
    fs::write(
        work.path().join("vibe-redirect.toml"),
        "[redirect]\ntarget_url = \"https://example.invalid/v2\"\n",
    )
    .unwrap();

    commit_and_push(work.path(), &url, "stub: retarget to v2").expect("commit_and_push ok");

    // The bare origin must now carry two commits on main.
    let log = Command::new("git")
        .args(["-C", bare.to_str().unwrap(), "log", "--oneline", "main"])
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    let log_out = String::from_utf8_lossy(&log.stdout);
    let lines: Vec<&str> = log_out.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "expected exactly two commits on main, got: {log_out}"
    );
    assert!(
        lines[0].contains("retarget"),
        "newest commit subject lost: {log_out}"
    );
}

#[test]
fn commit_and_push_refuses_when_working_tree_clean() {
    if !git_available() {
        eprintln!("skipping: git not on PATH");
        return;
    }

    let outer = tempdir().unwrap();
    let bare = outer.path().join("origin.git");
    let init_status = Command::new("git")
        .args(["init", "--bare", bare.to_str().unwrap()])
        .env("LC_ALL", "C")
        .status()
        .unwrap();
    assert!(init_status.success());

    let seed = tempdir().unwrap();
    fs::write(seed.path().join("file.txt"), "hi").unwrap();
    let url = bare.to_string_lossy().into_owned();
    push_initial(seed.path(), &url, "initial").expect("seed ok");

    let work = shallow_clone(&url).expect("clone ok");
    // No edits — working tree is clean against HEAD.
    let err = commit_and_push(work.path(), &url, "should fail").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("nothing to commit"),
        "expected nothing-to-commit error, got: {msg}"
    );
}

#[test]
fn redact_credentials_multiple_urls_in_message() {
    let msg = "trying https://user:pw1@host.example/a then https://user:pw2@other.example/b done";
    let scrubbed = redact_credentials(msg);
    assert!(!scrubbed.contains("pw1"));
    assert!(!scrubbed.contains("pw2"));
    assert!(scrubbed.contains("https://***@host.example/a"));
    assert!(scrubbed.contains("https://***@other.example/b"));
}
