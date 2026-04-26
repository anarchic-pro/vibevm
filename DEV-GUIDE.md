# Developer Guide

Contributor-facing setup: what to install on a fresh machine to clone the repo, build the CLI, run the tests, run manual smokes, and publish packages (if authorized).

For end-user setup (how to *use* the shipped `vibe` CLI), see [`RUNTIME-GUIDE.md`](RUNTIME-GUIDE.md).

**Update policy.** Every change touching toolchain, prerequisites, env vars, or bootstrap steps MUST update this file in the same commit. Never ship a dev-env change and a doc update separately. Policy pinned in [PROP-000](spec/common/PROP-000.md) — the obligation is load-bearing.

---

## 1. Supported platforms

- **Primary dev:** Windows 11 + Git Bash (the machine of record for this project).
- **Also supported:** macOS 12+, Linux (any recent glibc distro).

## 2. Prerequisites

### 2.1 Rust toolchain

Pinned in [`rust-toolchain.toml`](rust-toolchain.toml). Install rustup from <https://rustup.rs>, clone the repo, and the first `cargo` invocation picks up the pinned toolchain automatically.

### 2.2 git

System `git` must be in `PATH`. `vibe-registry` shells out to `git` for all registry operations — see [PROP-001 §2.1](spec/modules/vibe-registry/PROP-001-git-backend.md#backend).

- Windows: [Git for Windows](https://git-scm.com/download/win). Bundled OpenSSH works with GitVerse out of the box once your key is in `ssh-agent`.
- macOS: `brew install git` or Xcode command-line tools.
- Linux: your distro's `git` package.

Verify with `git --version`.

### 2.3 SSH key for GitVerse (optional, required for publish)

Needed if you intend to push to `git@gitverse.ru:…`. Load the key into `ssh-agent`, verify with `ssh -T git@gitverse.ru` — it should confirm auth and exit without a shell.

### 2.4 Publish token (optional, required for `vibe registry publish`)

GitVerse public-API token at:

- POSIX: `~/.vibevm/git.publish.token`
- Windows: `%USERPROFILE%\.vibevm\git.publish.token`

Or export `VIBEVM_PUBLISH_TOKEN` (env wins over the file). Needed only for the publish subcommand — ordinary install/update never touches it.

### 2.5 Schema codegen (JTD)

JTD (JSON Type Definition, RFC 8927) is the source of truth for every wire contract in the project ([PROP-000 §16](spec/common/PROP-000.md#jtd)). `jtd-codegen` generates Rust types (and, eventually, other-language client types) from the `*.jtd.json` schemas at the repo root under [`schemas/`](schemas/) into [`crates/vibe-wire/src/generated/`](crates/vibe-wire/src/generated/).

**Install** the generator binary into the project-local `tools/jtd-codegen/` per the procedure in [`tools/jtd-codegen/README.md`](tools/jtd-codegen/README.md). The binary itself is gitignored; only the README travels with the repo.

**Regenerate** types after editing schemas:

```sh
cargo xtask codegen
```

**Drift check** (CI runs this):

```sh
cargo xtask check-codegen
```

The xtask reports an actionable error if `jtd-codegen` is not on PATH or in `tools/jtd-codegen/`.

## 3. Build / test / lint

From repo root:

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

81 tests green on `main` as of the last checkpoint; clippy clean with `-D warnings`.

## 4. Manual smoke-tests

Live integration scripts live under [`manual-tests/`](manual-tests/). One file per scenario, self-contained walkthrough with clean-slate setup and teardown. Read [`manual-tests/README.md`](manual-tests/README.md) for the authoring conventions. Run the relevant script before tagging any milestone and after any change to an integration surface (git backend, CLI args, lockfile schema).

## 5. Publishing packages (maintainers only — planned)

`vibe registry publish <path>` is the maintainer tool for creating a package repo on GitVerse and pushing a tagged release. Procedure, auth requirements, and error-handling surface will be pinned here when the command lands.

## 6. Troubleshooting

(Populated as real issues arise. Empty today.)
