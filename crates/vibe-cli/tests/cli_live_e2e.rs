//! Live end-to-end tests against the public internet.
//!
//! These tests reach `github.com` and `gitverse.ru` — the canonical
//! `vibespecs` package registry on each host. Marked `#[ignore]` so
//! `cargo test --workspace` stays hermetic; run them explicitly with:
//!
//! ```
//! cargo test --test cli_live_e2e -- --ignored
//! ```
//!
//! What they prove
//! ===============
//!
//! 1. `cross_registry_resolution_routes_each_package_to_correct_host`
//!    — given the default two-registry layout (GitHub primary +
//!    GitVerse secondary), `vibe install` resolves a GitHub-only
//!    package against GitHub and a GitVerse-only package against
//!    GitVerse, in a single invocation. The lockfile records the
//!    correct `registry` per package, proving the fall-through walk
//!    on `UnknownPackage` works against real hosts.
//! 2. `install_github_smoke_alone` / `install_gitverse_smoke_alone`
//!    — split-half coverage so that a failure in one host doesn't
//!    obscure the other in the cross-registry combined case.
//!
//! Test fixtures published live
//! ============================
//!
//! - GitHub: `https://github.com/vibespecs/flow-vibevm-github-smoke`
//!   (created via `vibe registry publish` API path; see
//!   `fixtures/manual-test-packages/flow-vibevm-github-smoke/`).
//! - GitVerse: `https://gitverse.ru/vibespecs/flow-vibevm-direct-push-smoke`
//!   (created via `vibe registry publish --repo-url …` direct-push;
//!   see `fixtures/manual-test-packages/flow-vibevm-direct-push-smoke/`).
//!
//! Both carry `v0.0.1` and a single eager file plus a boot snippet —
//! enough to exercise the resolver, fetcher, integrity check, and
//! materialisation paths without burning a real package name.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

fn vibe() -> Command {
    Command::cargo_bin("vibe").expect("vibe binary built")
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
#[ignore = "live: hits github.com — run with `cargo test --test cli_live_e2e -- --ignored`"]
fn install_github_smoke_alone() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    vibe()
        .arg("install")
        .arg("flow:vibevm-github-smoke")
        .arg("--path")
        .arg(project.path())
        .arg("--assume-yes")
        .assert()
        .success();

    // Lockfile must record the GitHub registry as the source.
    let lock_text =
        fs::read_to_string(project.path().join("vibe.lock")).expect("lockfile present");
    let lock: vibe_core::manifest::Lockfile =
        toml::from_str(&lock_text).expect("lockfile parses");
    let pkg = lock
        .packages
        .iter()
        .find(|p| p.name == "vibevm-github-smoke")
        .expect("flow:vibevm-github-smoke must land in the lockfile");
    assert_eq!(
        pkg.registry.as_deref(),
        Some("vibespecs"),
        "GitHub package must attribute to `vibespecs` registry; lockfile entry: {pkg:?}"
    );
    assert!(
        pkg.source_url.contains("github.com"),
        "source_url must point at github.com; got `{}`",
        pkg.source_url
    );
    assert_eq!(pkg.version.to_string(), "0.0.1");

    // The package's eager file lands at the conventional path.
    assert!(
        project
            .path()
            .join("spec/flows/vibevm-github-smoke/PROTOCOL.md")
            .is_file(),
        "PROTOCOL.md must be materialised"
    );
}

#[test]
#[ignore = "live: hits gitverse.ru — run with `cargo test --test cli_live_e2e -- --ignored`"]
fn install_gitverse_smoke_alone() {
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    vibe()
        .arg("install")
        .arg("flow:vibevm-direct-push-smoke")
        .arg("--path")
        .arg(project.path())
        .arg("--assume-yes")
        .assert()
        .success();

    // Lockfile must record the GitVerse registry as the source after
    // the GitHub `[[registry]]` returned `UnknownPackage` (the package
    // does not exist on GitHub by design).
    let lock_text =
        fs::read_to_string(project.path().join("vibe.lock")).expect("lockfile present");
    let lock: vibe_core::manifest::Lockfile =
        toml::from_str(&lock_text).expect("lockfile parses");
    let pkg = lock
        .packages
        .iter()
        .find(|p| p.name == "vibevm-direct-push-smoke")
        .expect("flow:vibevm-direct-push-smoke must land in the lockfile");
    assert_eq!(
        pkg.registry.as_deref(),
        Some("vibespecs-gitverse"),
        "GitVerse-only package must attribute to `vibespecs-gitverse`; lockfile entry: {pkg:?}"
    );
    assert!(
        pkg.source_url.contains("gitverse.ru"),
        "source_url must point at gitverse.ru; got `{}`",
        pkg.source_url
    );
    assert_eq!(pkg.version.to_string(), "0.0.1");

    assert!(
        project
            .path()
            .join("spec/flows/vibevm-direct-push-smoke/PROTOCOL.md")
            .is_file(),
        "PROTOCOL.md must be materialised"
    );
}

#[test]
#[ignore = "live: hits github.com + gitverse.ru — run with `cargo test --test cli_live_e2e -- --ignored`"]
fn cross_registry_resolution_routes_each_package_to_correct_host() {
    // The headline test: prove that with both default registries
    // configured, two packages requested in the same `vibe install`
    // invocation route to the correct host based on which registry
    // carries them. Each is a name-only request (`flow:<name>`) — no
    // operator hint about which registry to use. The resolver walks
    // GitHub first (primary), falls through on `UnknownPackage`, and
    // lands on GitVerse for the package that only exists there.
    let project = tempfile::tempdir().unwrap();
    init_project(project.path());

    vibe()
        .arg("install")
        .arg("flow:vibevm-github-smoke")
        .arg("flow:vibevm-direct-push-smoke")
        .arg("--path")
        .arg(project.path())
        .arg("--assume-yes")
        .assert()
        .success();

    let lock_text =
        fs::read_to_string(project.path().join("vibe.lock")).expect("lockfile present");
    let lock: vibe_core::manifest::Lockfile =
        toml::from_str(&lock_text).expect("lockfile parses");

    let github_pkg = lock
        .packages
        .iter()
        .find(|p| p.name == "vibevm-github-smoke")
        .expect("github fixture installed");
    assert_eq!(
        github_pkg.registry.as_deref(),
        Some("vibespecs"),
        "github fixture must attribute to `vibespecs`; got: {github_pkg:?}"
    );
    assert!(
        github_pkg.source_url.contains("github.com"),
        "github fixture source_url must be on github.com; got `{}`",
        github_pkg.source_url
    );

    let gitverse_pkg = lock
        .packages
        .iter()
        .find(|p| p.name == "vibevm-direct-push-smoke")
        .expect("gitverse fixture installed");
    assert_eq!(
        gitverse_pkg.registry.as_deref(),
        Some("vibespecs-gitverse"),
        "gitverse fixture must attribute to `vibespecs-gitverse`; got: {gitverse_pkg:?}"
    );
    assert!(
        gitverse_pkg.source_url.contains("gitverse.ru"),
        "gitverse fixture source_url must be on gitverse.ru; got `{}`",
        gitverse_pkg.source_url
    );

    // Sanity: integrity hashes are present + distinct between the two.
    assert!(github_pkg.content_hash.starts_with("sha256:"));
    assert!(gitverse_pkg.content_hash.starts_with("sha256:"));
    assert_ne!(
        github_pkg.content_hash, gitverse_pkg.content_hash,
        "different packages must produce different content hashes"
    );
}
