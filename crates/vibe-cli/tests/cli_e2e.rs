//! End-to-end tests for the full M0 walk: init → install → list → uninstall.
//!
//! The registry used here is the hand-written `packages/` tree that ships in
//! the vibevm repo itself (the canonical `flow:wal` fixture per
//! `VIBEVM-SPEC.md` §13).

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn vibe() -> Command {
    Command::cargo_bin("vibe").expect("vibe binary built")
}

/// The `packages/` directory at the repo root is the fixture registry.
fn fixture_registry() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    workspace.join("packages")
}

fn init_project(dir: &Path) {
    vibe()
        .arg("init")
        .arg("--path")
        .arg(dir)
        .assert()
        .success();
}

#[test]
fn full_install_cycle() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    // Install flow:wal from the local fixture registry.
    vibe()
        .arg("install")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--registry")
        .arg(fixture_registry())
        .arg("--assume-yes")
        .assert()
        .success();

    // Expect all declared files to exist in the project.
    for rel in [
        "spec/flows/wal/WAL-PROTOCOL.md",
        "spec/flows/wal/session-end-hook.md",
        "spec/flows/wal/morning-routine.md",
        "spec/boot/10-flow-wal.md",
    ] {
        assert!(
            project.path().join(rel).is_file(),
            "expected {rel:?} to exist after install"
        );
    }

    // User-owned file survived untouched.
    let core_before = fs::read_to_string(project.path().join("spec/boot/00-core.md")).unwrap();

    // Lockfile must now carry the entry.
    let lock_text = fs::read_to_string(project.path().join("vibe.lock")).unwrap();
    let lock: vibe_core::manifest::Lockfile = toml::from_str(&lock_text).unwrap();
    assert_eq!(lock.packages.len(), 1);
    assert_eq!(lock.packages[0].name, "wal");
    assert_eq!(lock.packages[0].version.to_string(), "0.1.0");
    assert_eq!(lock.packages[0].boot_snippet.as_deref(), Some("10-flow-wal.md"));
    assert!(lock.packages[0].content_hash.starts_with("sha256:"));

    // Cache directory populated.
    assert!(project
        .path()
        .join(".vibe/cache/flow/wal/v0.1.0/vibe-package.toml")
        .is_file());

    // `vibe list` reflects the install.
    vibe()
        .arg("list")
        .arg("--path")
        .arg(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("flow"))
        .stdout(predicate::str::contains("wal"))
        .stdout(predicate::str::contains("0.1.0"));

    // `vibe uninstall` removes the declared files.
    vibe()
        .arg("uninstall")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--assume-yes")
        .assert()
        .success();

    for rel in [
        "spec/flows/wal/WAL-PROTOCOL.md",
        "spec/flows/wal/session-end-hook.md",
        "spec/flows/wal/morning-routine.md",
        "spec/boot/10-flow-wal.md",
    ] {
        assert!(
            !project.path().join(rel).exists(),
            "{rel:?} should be gone after uninstall"
        );
    }

    // User-owned file still intact.
    let core_after = fs::read_to_string(project.path().join("spec/boot/00-core.md")).unwrap();
    assert_eq!(core_before, core_after);

    // Lockfile entry removed.
    let lock_text = fs::read_to_string(project.path().join("vibe.lock")).unwrap();
    let lock: vibe_core::manifest::Lockfile = toml::from_str(&lock_text).unwrap();
    assert!(lock.packages.is_empty());

    // `list` after uninstall shows no packages.
    vibe()
        .arg("list")
        .arg("--path")
        .arg(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no packages"));
}

#[test]
fn install_rejects_second_install_of_same_package() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    vibe()
        .arg("install")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--registry")
        .arg(fixture_registry())
        .arg("--assume-yes")
        .assert()
        .success();

    // Second install should fail with a clear "already installed" error.
    vibe()
        .arg("install")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--registry")
        .arg(fixture_registry())
        .arg("--assume-yes")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already installed"));
}

#[test]
fn install_reports_json() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    let out = vibe()
        .arg("--json")
        .arg("install")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--registry")
        .arg(fixture_registry())
        .arg("--assume-yes")
        .output()
        .unwrap();
    assert!(out.status.success());

    // The plan and the report are both emitted as JSON documents,
    // concatenated on stdout. Use StreamDeserializer to read every document
    // in order and inspect the last one (the install report).
    let stdout = String::from_utf8(out.stdout).unwrap();
    let de = serde_json::Deserializer::from_str(&stdout);
    let docs: Vec<serde_json::Value> = de
        .into_iter::<serde_json::Value>()
        .collect::<Result<_, _>>()
        .expect("stdout is a stream of JSON documents");
    assert!(docs.len() >= 2, "expected at least a plan and a report");
    let last = docs.last().unwrap();
    assert_eq!(last["ok"], true);
    assert_eq!(last["command"], "install");
    assert_eq!(last["installed"].as_array().unwrap().len(), 1);
}

#[test]
fn uninstall_errors_when_package_not_installed() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    vibe()
        .arg("uninstall")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--assume-yes")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not installed"));
}

#[test]
fn install_boot_snippet_conflict_exits_with_code_three() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    // Plant a conflicting boot snippet with the same NN-prefix as flow:wal's
    // `10-flow-wal.md`.
    fs::create_dir_all(project.path().join("spec/boot")).unwrap();
    fs::write(
        project.path().join("spec/boot/10-flow-squatter.md"),
        "squatter\n",
    )
    .unwrap();

    let assertion = vibe()
        .arg("install")
        .arg("flow:wal")
        .arg("--path")
        .arg(project.path())
        .arg("--registry")
        .arg(fixture_registry())
        .arg("--assume-yes")
        .assert()
        .failure();
    let output = assertion.get_output();
    assert_eq!(output.status.code(), Some(3));
}
