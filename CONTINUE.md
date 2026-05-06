# CONTINUE — cold-resume checkpoint

_Written: 2026-05-06. Owner-readable, self-contained. Pick this up with zero prior context._

---

## TL;DR (executive summary)

**Publish-side reworked end to end.** This session landed a coherent slice across `vibe-core` / `vibe-publish` / `vibe-cli`:

1. Default `vibe init` now scaffolds **two `[[registry]]` blocks** — `vibespecs` (GitHub, primary, drives the API publish path) + `vibespecs-gitverse` (GitVerse, secondary, queried on `UnknownPackage` fall-through). GitVerse default uses `naming = "name"` because the org provisions repos under bare names (`vibevm-direct-push-smoke`) rather than the kind-prefixed form GitHub uses.
2. **GitVerse publish stub.** `vibe registry publish --registry vibespecs-gitverse` short-circuits with a clear "not implemented" envelope (`ok: false, stub: true, host: gitverse.ru`). The GitVerse public REST API does not yet expose org-scoped repo creation; the stub is honest about that without burning a token.
3. **Per-host publish-token env vars.** New precedence: `VIBEVM_PUBLISH_TOKEN_<HOST>` (host-specific) → `VIBEVM_PUBLISH_TOKEN` (legacy host-agnostic) → `~/.vibevm/<host-prefix>.publish.token` → `~/.vibevm/git.publish.token`. CI can hold tokens for several hosts in the same env without one clobbering the others. `vibe show config` lists all three publish-token vars with `redacted` provenance gating intact.
4. **`vibe registry publish --repo-url <git-url>`** — new no-API direct-push path. Pushes the freshly-built commit + tag straight to a supplied URL using the local user's git credentials (SSH agent / credential.helper / netrc). No token loaded, no host-API call. Implemented as `DirectGitCreator` declaring `direct_repo_url()`; `Publisher::publish` short-circuits the org-extraction + repo_exists + create_repo dance when that hook returns `Some`. `--repo-url` and `--registry` are mutually exclusive at the clap layer.
5. **Live e2e suite + manual-test fixtures.** Three `#[ignore]`-d tests in `crates/vibe-cli/tests/cli_live_e2e.rs` reach `github.com` + `gitverse.ru` and prove cross-registry resolution end to end. Two test packages published live: `https://github.com/vibespecs/flow-vibevm-github-smoke` (API path) + `git@gitverse.ru:vibespecs/vibevm-direct-push-smoke.git` (direct push). Walked successfully on this machine — all three tests green in 21.8s combined.

Workspace state at HEAD (`f6f4f0c`):

- **418 hermetic tests** + **3 ignored live tests** across the workspace, all green.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `tools/self-check.sh` green.
- Working tree clean (only `.claude/settings.local.json` untracked).

The most recent commit chain (newest first):

```
f6f4f0c test(cli): live e2e for cross-registry resolution + smoke fixtures
44a8c1c feat(core,publish,cli): two default registries + per-host tokens + no-API direct push
5429c17 docs(wal): session-end checkpoint 2026-05-05 — refresh top + Next
25915f5 docs(continue): cold-resume checkpoint at 2026-05-05 session-end
0566dbf docs(wal): record M1.7 slice 3 landing — lazy-pull runtime closed
3c9e710 feat(vibe-mcp): cache-precise read_subskill + materialise_subskill tool
390fc3a feat(vibe-core,vibe-install): per-subskill files index + lazy-pull becomes truly lazy
37d0ceb docs(wal): record M1.7 slice 2 landing — vibe mcp install + status
```

Push to `gitverse.ru:anarchic/vibevm` is current.

---

## Where we are right now

- **Branch:** `main`. Working tree clean.
- **Latest commit:** `f6f4f0c` (live e2e + fixtures).
- **Live test packages:**
  - GitHub: `https://github.com/vibespecs/flow-vibevm-github-smoke` @ `v0.0.1` (created via API path).
  - GitVerse: `git@gitverse.ru:vibespecs/vibevm-direct-push-smoke.git` @ `v0.0.1` (created via `--repo-url` direct push, SSH).
- **Other registry contents (untouched this session):** `https://github.com/vibespecs` still carries the original three v0.1.0 demo flows (`flow:wal`, `flow:sync-from-code`, `flow:atomic-commits`). GitVerse `vibespecs` org carries only the smoke fixture.
- **Integration fixtures (untouched):** `fixtures/registry/{flow/integration-alpha, flow/integration-beta, stack/integration-rust}` exercise every PROP-003 r2 surface in combination. Not yet published to vibespecs; ready when wanted.

---

## What landed in this session (chronological, newest first)

### Live e2e + manual-test fixtures (`f6f4f0c`)

- `crates/vibe-cli/tests/cli_live_e2e.rs` — three `#[ignore]`-d tests:
  - `install_github_smoke_alone` — `flow:vibevm-github-smoke` resolves via `vibespecs` (GitHub).
  - `install_gitverse_smoke_alone` — `flow:vibevm-direct-push-smoke` falls through GitHub's `UnknownPackage` and lands via `vibespecs-gitverse`.
  - `cross_registry_resolution_routes_each_package_to_correct_host` — both in the same `vibe install` invocation; lockfile records the right `registry` per package; distinct `content_hash`-es.
- `fixtures/manual-test-packages/flow-vibevm-github-smoke/` — throwaway test fixture for the GitHub API publish path.
- `fixtures/manual-test-packages/flow-vibevm-direct-push-smoke/` — throwaway test fixture for the no-API `--repo-url` direct push path.
- Both fixtures: trivial no-op flows (one PROTOCOL.md + one boot snippet); names scream "test"; pinned at `v0.0.1` forever so they stay deletable.
- Run live tests with: `cargo test --test cli_live_e2e -- --ignored`.

### Publish-side rework (`44a8c1c`)

**Dual-registry default.**
- `crates/vibe-core/src/manifest/project.rs` — new constants `DEFAULT_REGISTRY_GITVERSE_NAME = "vibespecs-gitverse"`, `DEFAULT_REGISTRY_GITVERSE_URL = "https://gitverse.ru/vibespecs"`. Existing `DEFAULT_REGISTRY_*` (GitHub) untouched.
- `crates/vibe-cli/src/commands/init.rs::resolve_registry_sections` — returns `Vec<RegistrySection>`; default = both registries (GitHub primary, kind-name; GitVerse secondary, name); `--registry-url` overrides to single; `--no-registry` empty.
- Root `vibe.toml` updated to mirror the new default shape (so self-`vibe check` validates against the same layout fresh projects use).

**GitVerse publish stub.**
- `crates/vibe-cli/src/commands/registry.rs::run_publish` — host-detection short-circuit for GitVerse-shaped registries. Emits `PublishStubReport { ok: false, command: "registry:publish", host, org_url, registry, stub: true, reason }`. No token loaded, no HTTP call.
- Resolve-time reads against GitVerse continue to work via `MultiRegistryResolver` (the stub only affects `vibe registry publish`).

**Per-host publish-token env vars.**
- `crates/vibe-publish/src/token.rs` — `TokenSource::EnvVar(String)` (was `&'static str`); new `host_env_var(host) -> Option<String>` builds `VIBEVM_PUBLISH_TOKEN_<HOST>` (e.g. `_GITHUB`, `_GITVERSE`). New precedence: host-env → legacy-env → host-file → legacy-file.
- `crates/vibe-cli/src/commands/show.rs` — `CONFIG_ENV_VARS` lists all three publish-token vars (`_GITHUB`, `_GITVERSE`, legacy bare). All `sensitive: true` → `redacted` provenance.

**`--repo-url` direct push (no API).**
- `crates/vibe-publish/src/direct_git.rs` — new `DirectGitCreator` adapter. `repo_exists` → `Ok(true)`; `create_repo` → error (unreachable on direct path); `push_url` → configured URL verbatim; `direct_repo_url` returns `Some(&url)`.
- `crates/vibe-publish/src/lib.rs` — new `RepoCreator::direct_repo_url() -> Option<&str>` hook (default `None`); `Publisher::publish` short-circuits on `Some`, skipping `extract_org_segment` + `repo_exists` + `create_repo`, going straight into `git_publish::push_release` with the supplied URL.
- `crates/vibe-cli/src/cli.rs` — new `RegistryPublishArgs::repo_url: Option<String>` with `conflicts_with = "registry"`.
- `crates/vibe-cli/src/commands/registry.rs::run_publish_direct` — dispatch path; emits `DirectPublishReport { ok: true, command: "registry:publish", mode: "direct-git", host, repo_url, repo_name, tag, dry_run }`. No token loading on this path.

**Tests.**
- 4 new unit tests in `vibe-publish::token::tests` (host_env_var rendering / sanitisation / blank input).
- 7 new unit tests in `vibe-publish::direct_git::tests` (host extraction / scope no-op / push_url verbatim / etc.).
- 3 new e2e tests in `cli_e2e.rs` (`publish_against_gitverse_registry_emits_stub_envelope`, `publish_direct_repo_url_pushes_to_local_bare_repo` against `--bare` `file:///` repo, `publish_repo_url_and_registry_are_mutually_exclusive`).
- Existing `init_writes_default_registry` updated to assert both registries land + GitVerse uses `naming = "name"`.

---

## Repository map

```
vibevm/                                     (this repo — gitverse.ru:anarchic/vibevm)
├── CLAUDE.md / AGENTS.md / GEMINI.md       ← byte-identical, the four rules + memory discipline
├── CONTINUE.md                             ← THIS FILE
├── DEV-GUIDE.md / RUNTIME-GUIDE.md         ← contributor / end-user setup
├── MEMORY.md                               ← pointer to spec/boot/90-user.md
├── ROADMAP.md / TASKS.md
├── VIBEVM-SPEC.md                          ← owner-frozen v1.0
├── tools/self-check.sh                     ← cargo test + clippy + vibe check, one entry point
├── tools/jtd-codegen/                      ← JTD codegen toolchain (binary not committed)
├── vibe.toml / vibe.lock                   ← bootstrap manifest (now carries dual registry default)
│
├── crates/                                 (Rust workspace — 13 crates, 3 placeholders)
│   ├── vibe-core/      ← manifest types, lockfile (schema v3), PURL, i18n, subskill, features,
│   │                     conditional, DEFAULT_REGISTRY_* + DEFAULT_REGISTRY_GITVERSE_*
│   ├── vibe-cli/       ← `vibe` binary; `--repo-url` direct push, GitVerse publish stub,
│   │                     dual-registry init defaults
│   ├── vibe-registry/  ← LocalRegistry + GitPackageRegistry + MultiRegistryResolver
│   ├── vibe-resolver/  ← NaiveDepSolver + features expansion + activation evaluator + conditional
│   ├── vibe-install/   ← plan/apply/register install, subskill discovery + materialisation
│   ├── vibe-publish/   ← GitHub / GitVerse / DirectGit RepoCreator adapters, host-aware token loader
│   ├── vibe-check/     ← spec linter (10 checks, including activation_conflict)
│   ├── vibe-mcp/       ← Model Context Protocol server (JSON-RPC over stdio, query_package /
│   │                     read_subskill / materialise_subskill tools)
│   ├── vibe-wire/      ← JTD-generated wire types (init_report fully migrated; rest hand-rolled)
│   ├── vibe-llm/       ← M0 placeholder (M1.5)
│   ├── vibe-graph/     ← M0 placeholder
│   └── xtask/          ← `cargo xtask codegen` / `check-codegen`
│
├── fixtures/
│   ├── registry/                            ← LocalRegistry + PROP-003 r2 omnibus (untouched)
│   │   ├── flow/{wal, sync-from-code, atomic-commits}/v0.1.0/
│   │   ├── flow/{integration-alpha, integration-beta}/v0.1.0/
│   │   └── stack/integration-rust/v0.1.0/
│   └── manual-test-packages/                ← NEW: throwaway fixtures for live publish tests
│       ├── flow-vibevm-github-smoke/        ← published live to GitHub
│       └── flow-vibevm-direct-push-smoke/   ← published live to GitVerse
│
├── docs/                                   ← user-facing reference per command
├── manual-tests/                           ← runnable smoke protocols
├── schemas/                                ← JTD wire-contract schemas
├── spec/
│   ├── boot/00-core.md … 90-user.md        ← session-boot foundation
│   ├── WAL.md                              ← canonical living state
│   ├── common/PROP-000.md
│   ├── modules/vibe-registry/PROP-001-...md / PROP-002-...md
│   ├── modules/vibe-resolver/PROP-003-dep-evolution.md   (r2)
│   └── research/PROP-004-tessl-comparative-research.md
│
├── refs/                                   (.gitignored — cargo / dnf / dnf5 study sources)
└── packages/                               (reserved for vibevm-using-vibevm dogfooding; empty)
```

---

## Quick-start commands

```bash
# Workspace health.
bash tools/self-check.sh

# Run live e2e tests against real internet (~22s combined).
cargo test --test cli_live_e2e -- --ignored

# Inspect host-aware token resolution in `vibe show config`.
cargo run -p vibe-cli -- show config

# Direct-push a fixture to a known git URL (no API, no token, local creds).
cargo run -p vibe-cli -- registry publish \
    fixtures/manual-test-packages/flow-vibevm-direct-push-smoke \
    --repo-url git@gitverse.ru:vibespecs/vibevm-direct-push-smoke.git \
    --path .

# Drive the MCP server manually.
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | \
  cargo run -p vibe-cli -- mcp serve --path /tmp/demo
```

---

## Non-obvious findings discovered this session

1. **GitVerse HTTPS public read works for repos that exist; returns 401-with-credentials-prompt for repos that don't.** This is host behaviour, not vibevm's. The first run of the live e2e failed not because GitVerse needed auth but because the resolver was asking for a wrong-named URL (`flow-vibevm-direct-push-smoke.git` under default `kind-name`, while the actual repo is `vibevm-direct-push-smoke.git`). The fix was the `naming = "name"` default for `vibespecs-gitverse`, not switching auth paths.
2. **GitVerse `vibespecs` org convention is bare names (no `kind-` prefix).** Recorded as the default `naming` for the secondary registry. If the convention changes in the future, the default needs to change with it.
3. **`forbid(unsafe_code)` blocks env-mutation tests** — `std::env::set_var` is `unsafe` from Rust 1.85+. `vibe-publish` uses `forbid(unsafe_code)` so token-loading tests with live env mutations are not possible there. The host_env_var unit tests cover the renderer; live env-precedence is exercised through the live e2e suite (sets `VIBEVM_PUBLISH_TOKEN_GITHUB` / `VIBEVM_PUBLISH_TOKEN` to bogus values and asserts direct-push doesn't read them).
4. **GitHub publishing's HTTPS-token push works fine even when the registry URL is HTTPS public read.** The push URL is constructed by `GitHubCreator::push_url` (token-credentialed HTTPS) regardless of the configured registry URL form. Switching the default registry URL to SSH would not have affected push behaviour.
5. **The user's machine preference is SSH for push paths** (recorded in `~/.claude/projects/.../memory/feedback_publish_url_form_preference.md`). Product code supports both equally; on this dev box, default to SSH form when proposing `--repo-url` invocations.

---

## What's still open

By size and priority:

1. **M1.8 — `vibe review` static quality scoring.** New `vibe-eval` crate with three-axis scoring (validation / implementation / activation). Static portion only; LLM-judge mode lands in M2.7. Smallest immediate win — ~1 weekend.

2. **M2.10 — `vibe search` registry inspector.** Walks every configured `[[registry]]` URL, lists packages whose `vibe-package.toml` description matches a query. Naive at first; indexing later. Useful at 20+ packages, essential at 100+. Recent research surfaced three viable paths (cargo sparse-index style / DNF repodata-style / Nix flake-registry-style); leaning toward the cargo sparse-style for the first pass — one optional JSON file per package in the org, populated by `vibe registry publish`.

3. **M1.5 — LLM provider abstraction + `vibe build`.** The Big One. ROADMAP §M1.5.1–§M1.5.5. 3-6 weekends. Requires explicit owner sign-off per CLAUDE.md Rule 4.

4. **libsolv FFI / `SatDepSolver`** (PROP-003 §2.1, Phase A). Standalone slice. ~2-3 weekends.

5. **M2.9 — scenario generation from real commits.** Depends on M1.5.1 + M1.8.

6. **`vibe update` feature-awareness.** Known gap: `vibe install foo --features X` followed by `vibe update foo` loses X. ~1 weekend.

7. **vibe-mcp follow-ups.** Gemini / Codex / Copilot agent writers, `list_capabilities` discovery tool, user-level config (`~/.config/claude/...`).

8. **GitHub publish path → SSH option.** Currently HTTPS-token only. Could add SSH fallback for operators who prefer key-based push. Tied to broader publish-flow polish.

9. **`vibe registry publish --repo-url` for GitVerse → unstub.** When/if the GitVerse public API ever exposes org-scoped repo creation, the stub branch in `run_publish` flips back to the regular adapter dispatch. Note tracked in the stub message itself.

10. **Documentation files for new commands.** `docs/commands/{publish-direct.md,publish-stub.md}` plus `docs/commands/show.md` refresh for the new env-var entries. Mostly mechanical translation of `--help` text into reference shape.

11. **Conditional-dep cleanup on uninstall** — orphan auto-remove when a trigger goes away. Park.

12. **`vibe outdated --upstream`** PURL probe. Per-ecosystem HTTP clients.

---

## Standing rules / pointers

- **Read on session boot, in this order:** `CLAUDE.md`, every file in `spec/boot/` in filename order, `spec/WAL.md`, then any PROP under `spec/common/` or `spec/modules/` for the task at hand, then start work.
- **Four non-negotiable rules** (CLAUDE.md / [PROP-000 §12](spec/common/PROP-000.md#commits)): (1) human-only attribution, (2) Conventional Commits, (3) group commits by meaning, (4) autonomy on routine changes only.
- **Memory discipline:** project facts in repo, machine-local in user-memory.
- **Setup-docs obligation** ([PROP-000 §19](spec/common/PROP-000.md#setup-docs)): toolchain / prereqs / env / paths changes → `DEV-GUIDE.md` or `RUNTIME-GUIDE.md` in same commit.
- **Vocabulary lock:** `flow` / `feat` / `stack` / `tool`; never `lifecycle` / `phase` / `goal` / `plugin`.
- **Token secrecy** ([PROP-000 §20](spec/common/PROP-000.md#token-secrecy)): never display, never persist outside `~/.vibevm/`, never commit. Per-host env vars (`VIBEVM_PUBLISH_TOKEN_<HOST>`) follow the same discipline.
- **User-owned files** (`vibe install` / `uninstall` never modifies): `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any 00-09 or 90-99 boot file.
- **Default registries:** GitHub primary (`https://github.com/vibespecs`, `kind-name`); GitVerse secondary (`https://gitverse.ru/vibespecs`, `name`). Pull is HTTPS public read (no auth); push prefers SSH on this dev machine.
- **Live e2e tests are `#[ignore]`-d.** CI stays hermetic; live walks are an explicit `cargo test --test cli_live_e2e -- --ignored` opt-in.

---

## If something has changed since this checkpoint

This file is frozen at 2026-05-06. Before acting on it:

- Re-read `spec/WAL.md` (it gets updated more often, and at session-end at the same time).
- `git log origin/main..HEAD --oneline` to see local-only work (should be empty after a clean session-end).
- `git status` to see the working tree.
- `bash tools/self-check.sh` to confirm the workspace is shippable.

If the WAL and this file disagree, **trust the WAL**.
