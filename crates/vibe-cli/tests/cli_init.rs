//! Integration tests for `vibe init`.
//!
//! Spec: `VIBEVM-SPEC.md` §11.1 and the M0 acceptance checklist in §16.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn vibe() -> Command {
    Command::cargo_bin("vibe").expect("vibe binary built")
}

#[test]
fn init_creates_expected_layout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    vibe()
        .arg("init")
        .arg("--path")
        .arg(path)
        .assert()
        .success();

    for rel in [
        "CLAUDE.md",
        "AGENTS.md",
        "GEMINI.md",
        "spec/boot/00-core.md",
        "spec/boot/90-user.md",
        "spec/WAL.md",
        "vibe.toml",
        "vibe.lock",
        ".vibe/.gitignore",
        ".gitignore",
    ] {
        assert!(
            path.join(rel).exists(),
            "expected `{rel}` to exist after init"
        );
    }

    // CLAUDE.md / AGENTS.md / GEMINI.md have the exact same one-line body.
    let claude = fs::read_to_string(path.join("CLAUDE.md")).unwrap();
    let agents = fs::read_to_string(path.join("AGENTS.md")).unwrap();
    let gemini = fs::read_to_string(path.join("GEMINI.md")).unwrap();
    assert_eq!(claude, agents);
    assert_eq!(agents, gemini);
    assert!(claude.trim_end().ends_with("await the user's instructions."));

    // vibe.toml should parse as a valid ProjectManifest.
    let manifest_text = fs::read_to_string(path.join("vibe.toml")).unwrap();
    let parsed: vibe_core::manifest::ProjectManifest = toml::from_str(&manifest_text).unwrap();
    assert_eq!(parsed.project.version, "0.0.1");
    assert!(parsed.project.name.ends_with(
        path.file_name().unwrap().to_str().unwrap()
    ) || parsed.project.name == path.file_name().unwrap().to_str().unwrap());

    // Empty lockfile parses back and carries the expected metadata.
    let lock_text = fs::read_to_string(path.join("vibe.lock")).unwrap();
    let lock: vibe_core::manifest::Lockfile = toml::from_str(&lock_text).unwrap();
    assert!(lock.packages.is_empty());
    assert!(lock.meta.generated_by.starts_with("vibe "));
}

#[test]
fn init_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    // First run.
    vibe()
        .arg("init")
        .arg("--path")
        .arg(path)
        .assert()
        .success();

    // Mark boot/00-core.md with a user edit, then re-init.
    let user_marker = "# EDITED BY USER\n";
    let core_path = path.join("spec/boot/00-core.md");
    fs::write(&core_path, user_marker).unwrap();

    vibe()
        .arg("init")
        .arg("--path")
        .arg(path)
        .assert()
        .success()
        .stdout(predicate::str::contains("kept"));

    // Second run must NOT overwrite the user's edit.
    let after = fs::read_to_string(&core_path).unwrap();
    assert_eq!(after, user_marker, "00-core.md must be preserved");
}

#[test]
fn init_stack_flag_sets_active_stack() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    vibe()
        .arg("init")
        .arg("--path")
        .arg(path)
        .arg("--stack")
        .arg("rust-cli")
        .assert()
        .success();

    let manifest_text = fs::read_to_string(path.join("vibe.toml")).unwrap();
    let parsed: vibe_core::manifest::ProjectManifest = toml::from_str(&manifest_text).unwrap();
    assert_eq!(
        parsed.active.as_ref().and_then(|a| a.stack.as_deref()),
        Some("rust-cli")
    );
}

#[test]
fn init_custom_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    vibe()
        .arg("init")
        .arg("--path")
        .arg(path)
        .arg("--name")
        .arg("my-special-project")
        .assert()
        .success();

    let manifest_text = fs::read_to_string(path.join("vibe.toml")).unwrap();
    let parsed: vibe_core::manifest::ProjectManifest = toml::from_str(&manifest_text).unwrap();
    assert_eq!(parsed.project.name, "my-special-project");
}

#[test]
fn init_json_output_parses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let out = vibe()
        .arg("--json")
        .arg("init")
        .arg("--path")
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout must be valid JSON");
    assert_eq!(v["ok"], true);
    assert_eq!(v["command"], "init");
    assert_eq!(v["created"], 10);
    assert_eq!(v["kept"], 0);
}

#[test]
fn init_quiet_emits_single_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();

    let out = vibe()
        .arg("--quiet")
        .arg("init")
        .arg("--path")
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).unwrap();
    let trimmed = stdout.trim();
    assert!(!trimmed.contains('\n'), "quiet output must be single line: {trimmed:?}");
    assert!(trimmed.contains("vibe init:"));
}

#[test]
fn init_version() {
    vibe().arg("version").assert().success();
    vibe().arg("--version").assert().success();
}
