# CONTINUE — cold-resume checkpoint

_Written: 2026-05-21 session-end. Owner-readable, self-contained. Pick this up with zero prior context._

---

## TL;DR

**This session implemented M1.17 — Workspace (multi-package projects), the [PROP-007](spec/modules/vibe-workspace/PROP-007-workspace.md) milestone — across six phases, on the branch `m1.17-workspace`.** The complete workspace data model and tooling shipped: one unified `vibe.toml` per node, the `vibe-workspace` discovery engine, path-source dependencies + `vibe.lock` schema v4, `[workspace.versions]` placeholders, and the `vibe workspace publish` command. Ten commits (`b794e7a..3cb2a03`) plus the session-end docs commits; every phase landed clippy-clean with its tests green — **703 hermetic tests** across the workspace, `vibe check` 0/0/0.

**One piece of M1.17 is deliberately deferred:** wiring `vibe install` / `vibe build` to discover the workspace and run unified multi-member resolution. It is gated on a design decision PROP-007 leaves open — see **Deferred** below.

**The branch is local** — not pushed to origin, not merged to `main`. It awaits the owner's review.

The detailed implementation record is **[PROP-007 §9](spec/modules/vibe-workspace/PROP-007-workspace.md#impl)**. The canonical living state is **[`spec/WAL.md`](spec/WAL.md)** — if it disagrees with this file, trust the WAL.

---

## Where work stands

- **Branch:** `m1.17-workspace` — 10 commits ahead of `main` (`8de20c2`), plus this session-end's docs commits. **Local only — `origin` does not have this branch.**
- **`main`** is untouched, still at `8de20c2`.
- **Working tree:** clean (only `.claude/settings.local.json` untracked — pre-existing, not part of this work).
- **Active blocker:** none for the shipped work. The one remaining M1.17 piece is blocked on an owner decision — see **Deferred** #1.
- **Gates:** `cargo clippy --workspace --all-targets -- -D warnings` clean; 703 hermetic tests pass, 0 failures; `vibe check --path . --quiet` reports 0/0/0.

## What was built — the six phases

Brief; the full account is [PROP-007 §9](spec/modules/vibe-workspace/PROP-007-workspace.md#impl).

1. **Phase 1 — unified manifest** (`b794e7a`, `9a190ff`). One `Manifest` type / one `vibe.toml` per node replaces `ProjectManifest` + `PackageManifest`; the role is the set of sections present (`[project]` ⊕ `[package]`, `[workspace]`). **All manifest legacy deleted** — the `vibe-package.toml` filename, `[dependencies]`, array-form `packages`, the singleton `[registry]`. ~190 downstream call-sites + 8 fixtures migrated.
2. **Phase 2 — workspace model** (`ece30a6`). New `vibe-workspace` crate: `Workspace::discover` bubbles to the absolute root, recursive nesting, glob members, cycle detection. A standalone project is a degenerate workspace, so discovery is universal.
3. **Phase 3 — path-source + lockfile v4** (`ff21de3`, `e9a15d2`). `{ path = "../sibling" }` deps; resolver priority `override > path > git > registry`; `vibe.lock` schema v4 (`source_kind = "path"`), legacy v1/v2/v3 readers removed.
4. **Phase 4 — `[workspace.versions]`** (`98795e8`). Named version placeholders; `{ version.var = "core" }`; recursive matryoshka resolution in the workspace loader.
5. **Phase 5 — `vibe workspace publish`** (`b673d2b`). Selective publish (`[package].publish`), topological walk, `[origin]` marker + "contribute upstream" signalling, non-atomic.
6. **Phase 6 — documentation** (`047f92d`, `10406a1`, `3cb2a03`, + session-end). `VIBEVM-SPEC.md` §4.2 / §7.6, PROP-007 §9, ROADMAP / CHANGELOG, the `docs/` + `manual-tests/` sweep, `docs/commands/workspace-publish.md`, WAL, this file.

## What to do first in the next session

The branch is review-ready. Two natural pickups:

1. **Owner review of `m1.17-workspace`.** It is the entire M1.17 model + tooling. After review, the owner decides whether to push to origin and merge to `main`.
2. **The remaining M1.17 piece — workspace-aware `vibe install`** — once the owner answers the materialisation-target question (Deferred #1). Recipe: read PROP-007 §2.4 / §3, §6 question 3, and §9.3. `vibe install`'s entry point is `crates/vibe-cli/src/commands/install.rs::run`; `resolve_project_root` there would become a `vibe_workspace::Workspace::discover` call; the resolve would gather every member's `[requires]` (registry + git + `path_packages` → `vibe_registry::ResolvedPathDep` fed to `MultiRegistryResolver::with_path_packages`); one `vibe.lock` written at the absolute root.

---

## Deferred — with rationale

Three pieces of PROP-007 were deliberately not built this milestone. None is an oversight; each is recorded here, in [PROP-007 §9.3](spec/modules/vibe-workspace/PROP-007-workspace.md#impl-deferred), and as an open question in [PROP-007 §6](spec/modules/vibe-workspace/PROP-007-workspace.md#open).

1. **Workspace-aware `vibe install` / `vibe build`.** The one remaining piece of PROP-007's intent — discovering the workspace and running unified multi-member resolution. **Why deferred:** the concrete behaviour turns on a *per-member materialisation target* — when a dependency is resolved for member M, into which member's `spec/` does its content land? PROP-007 §2.4 / §3 sketch command-bubbling and unified resolution but do not specify this; it is a genuine design fork that wants an explicit owner decision, not an improvised one. The path-source resolver capability it builds on **is** implemented and tested (Phase 3). **Standalone single-package projects — every project today — are unaffected: `vibe install` works exactly as before.**
2. **`version = { workspace = true }`** — a member inheriting its own `[package].version` from the workspace. **Why deferred:** PROP-007 §2.6 names the feature but defines no source table for the inherited version — cargo reads `[workspace.package].version`, a table PROP-007 never specifies. Shipping it means extending the spec; that is a decision to take explicitly. The `[workspace.versions]` named placeholders (shipped) already cover the owner's stated "write the version once" use case.
3. **Publish-signalling polish** (PROP-007 §2.8) — `--archive` (the GitHub `archived = true` lockdown and its unarchive→push→archive re-publish cycle), `has_issues = false` at repo creation, the `published_repos = "read-only" | "open"` toggle, and multi-registry fan-out. **Why deferred:** the `[origin]` marker + README banner + PR template + description (all shipped) already make a published copy unmistakably a generated read-only copy; these remaining layers are incremental host-API hardening, and `--archive`'s re-publish cycle is a feature in its own right.

## Requires owner attention

1. **`spec/boot/00-core.md` line 38** still reads `package manifest = vibe-package.toml`. That is factually stale after Phase 1 — there is no `vibe-package.toml` any more. But `00-core.md` is a **user-owned boot file** vibevm tooling must not edit (it is on the must-not-touch list in that very file). **The owner should change that line to `vibe.toml`.**
2. **Branch `m1.17-workspace` is local** — not pushed to origin, not merged to `main`. The work was kept on a branch per the owner's instruction. Pushing / merging is the owner's call after review.
3. **The materialisation-target decision** ([PROP-007 §6 question 3](spec/modules/vibe-workspace/PROP-007-workspace.md#open)) is the gate on the remaining workspace-aware `vibe install` work. It needs an owner answer before that work can proceed well.
4. **(Carried from 2026-05-12.)** Delete `https://gitverse.ru/vibespecs/vibevm-direct-push-smoke` via the GitVerse web UI — GitVerse has no API DELETE endpoint. Not blocking.

---

## Non-obvious findings

1. **Windows Defender blocks `cargo test -p vibe-install` (`os error 740`).** On this machine, the freshly-compiled unsigned `vibe_install-<hash>.exe` test runner is blocked ("requires elevation"). This is **not a code bug** — `cargo build -p vibe-install --tests` type-checks cleanly, and `vibe-install` was verified that way this milestone (it carries the `is_path_source → SourceKind::Path` lockfile mapping). A cold session on this machine must not mistake this for a regression. The owner is resolving the AV side himself.
2. **The no-legacy hard break is by design.** vibevm is pre-release; M1.17 deleted *all* manifest and lockfile legacy rather than carry compatibility shims. There is **no migration path and none is intended**: a `vibe-package.toml`, a legacy `[dependencies]` / array-form `packages` / singleton `[registry]`, or a pre-v4 `vibe.lock` is now a hard parse error. Anyone resuming must not "add back" a reader out of a sense of politeness — the break is deliberate.
3. **`vibe-workspace` is wired into `vibe-cli` only.** The new crate is a dependency of `vibe-cli` (for `vibe workspace publish`) and nothing else. The resolver (`vibe-registry`) and the install pipeline do **not** depend on it yet — that wiring is the deferred install-integration work.
4. **The path-source resolver is done but unwired into `vibe install`.** `vibe-registry`'s `MultiRegistryResolver` can resolve a path-dep when handed a `ResolvedPathDep` (absolute dir + workspace-relative path). What does not exist yet is the code that computes those `ResolvedPathDep`s from a discovered `Workspace` and feeds them in during `vibe install`. The seam is intentional — Phase 3 built and tested the capability; the integration is Deferred #1.
5. **`scan_local_packages` in `vibe-check`** now also returns the project-root `vibe.toml` (the filename is unified). Harmless — its three consumers short-circuit on empty `[features]` / `[i18n]` / missing `subskills/` — and the doc comment says so. Noted in case a future `vibe check` change wants to filter by `is_package()`.

## Repository map

```
vibevm/
├── CLAUDE.md / AGENTS.md / GEMINI.md   # Three identical copies of the four rules.
├── CONTINUE.md                          # This file. Cold-resume snapshot.
├── ROADMAP.md                           # M1.17 — Phases 1–5 shipped; install-integration remaining.
├── CHANGELOG.md                         # [Unreleased] holds the M1.17 milestone entry.
├── VIBEVM-SPEC.md                       # Owner-frozen spec; §4.2 + §7.6 now document the workspace model.
├── vibe.lock                            # The repo's own lockfile — schema v4.
├── crates/
│   ├── vibe-core/                       # Manifest + lockfile schema. manifest/document.rs = the
│   │   │                                #   unified `Manifest`; package.rs / project.rs = sections;
│   │   │                                #   lockfile.rs = schema v4.
│   ├── vibe-workspace/                  # NEW crate (M1.17). Workspace discovery + member model
│   │   │                                #   (lib.rs) and publish selection/staging (publish.rs).
│   ├── vibe-registry/                   # MultiRegistryResolver — now path-source aware.
│   ├── vibe-resolver/                   # Depsolver + DepProvider adapters.
│   ├── vibe-publish/                    # RepoCreator + push helpers (reused by workspace publish).
│   ├── vibe-cli/                        # `vibe` binary. commands/workspace.rs = `vibe workspace publish`.
│   └── ... (vibe-install, vibe-check, vibe-mcp, vibe-graph, vibe-llm, vibe-wire)
├── spec/
│   ├── boot/{00-core,90-user}.md         # User-owned. NOTE: 00-core.md line 38 is stale — see attention.
│   ├── WAL.md                            # Living checkpoint — authoritative, supersedes this file.
│   ├── common/PROP-000…PROP-006
│   ├── modules/
│   │   ├── vibe-workspace/PROP-007-workspace.md   # The workspace contract + §9 implementation record.
│   │   ├── vibe-registry/PROP-002, PROP-008
│   │   ├── vibe-resolver/PROP-003, vibe-index/PROP-005
│   ├── research/PROP-004
│   └── design/workspace-and-qualified-naming.md   # The pre-implementation design lore.
├── docs/                                 # User guides — swept for the unified-manifest facts.
│   └── commands/workspace-publish.md     # NEW — `vibe workspace publish` reference.
├── manual-tests/                         # Runnable smoke protocols. M1.17-workspace-publish-smoke.md new.
└── services/vibe-index/                  # Separate index service (PROP-005); not in the cargo workspace.
```

## Architectural / policy decisions in force

In rough order of how often they bite a fresh contributor:

1. **Four non-negotiable rules** ([PROP-000 §12](spec/common/PROP-000.md#commits)): no AI/machine-author attribution anywhere; Conventional Commits (subject ≤ 60, body explains WHY); group commits by meaning; autonomy on routine changes only.
2. **Memory discipline.** Project facts live in the repo, not in per-machine user-memory.
3. **One unified manifest (M1.17).** Every node carries one `vibe.toml`; the role is the set of sections present. `[project]` ⊕ `[package]`; `[workspace]` composes with either or neither. There is no `vibe-package.toml`.
4. **No legacy, by design (M1.17).** All manifest and lockfile legacy forms are deleted; a removed form is a hard error. vibevm is pre-release — see Non-obvious findings #2.
5. **Vocabulary lock.** Only `flow`, `feat`, `stack`, `tool`. Never `lifecycle` / `phase` / `goal` / `plugin`.
6. **Language: Rust.** Permissive licenses only.
7. **Identity: `(kind, name, version, content_hash)`.** URL is informational. (PROP-008, not yet implemented, will change this to `(group, name, version, content_hash)` at M1.18.)
8. **Lockfile is schema v4** at the absolute workspace root; one per workspace. `source_kind` ∈ `registry` / `git` / `override` / `path`.
9. **Token secrecy** ([PROP-000 §20](spec/common/PROP-000.md#token-secrecy)). Never printed in any vibevm output.
10. **Repository hosts.** vibevm source = GitVerse. Package registry = GitHub `vibespecs` (primary) + GitVerse `vibespecs` (secondary).
11. **User-owned files** vibevm never touches: `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md` (edited this milestone only under the owner sanction recorded in PROP-007's header), `refs/book/**`.
12. **Resolution priority** (M1.17): `[[override]]` > path-source > git-source > registry-walk.
13. **Owner sanction for `VIBEVM-SPEC.md` edits** — granted for the workspace + qualified-naming refactor; recorded in the PROP-007 / PROP-008 headers.

## Recent commit chain (newest first)

```
3cb2a03 docs(wal): M1.17 Phases 1-5 checkpoint
10406a1 docs: document the M1.17 workspace model
047f92d build: sync Cargo.lock — vibe-cli now depends on vibe-workspace
b673d2b feat(cli,workspace): vibe workspace publish
98795e8 feat(core,workspace): [workspace.versions] placeholders
e9a15d2 docs(spec): VIBEVM-SPEC §7.4 — lockfile v4
ff21de3 feat(core,registry): path-source deps + lockfile v4
ece30a6 feat(workspace): discovery and the member model
9a190ff docs(spec): VIBEVM-SPEC §7 — unified vibe.toml manifest
b794e7a feat(core): unify manifests into a single vibe.toml
8de20c2 docs(wal): session-end checkpoint 2026-05-20          <- main HEAD
23a568e docs(continue): cold-resume checkpoint 2026-05-20
4d6775a docs(spec): add spec/design genre + workspace/naming design rationale
ff23a0f docs(spec): draft PROP-007 + PROP-008 — workspace & qualified naming
b44729d docs(commands,registry-redirect,changelog): redirect-update reference
3553b2e test(vibe-cli): hermetic e2e for redirect-update args-level guard rails
cce61ac feat(vibe-cli): vibe registry redirect-update command
f8af587 feat(vibe-publish): commit_and_push helper for in-place stub updates
9740c10 docs(continue,wal): session-end checkpoint 2026-05-12
4e852f0 docs(registry-redirect,changelog,wal,continue): note test-org re-home
```

Plus this session-end's docs commits (PROP-007 §9 implementation record, the WAL checkpoint, this `CONTINUE.md`). The 10 M1.17 commits are `b794e7a..3cb2a03`; everything from `8de20c2` down is on `main` already.

## Quick-start commands

```powershell
# Build everything.
cargo build --workspace

# Test gate (matches CI). NOTE: `cargo test -p vibe-install` may fail on this
# machine with `os error 740` — Windows AV blocking the test binary, NOT a code
# bug (see Non-obvious findings #1). Use `cargo build -p vibe-install --tests`
# to type-check that crate instead.
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p vibe-core -p vibe-workspace -p vibe-registry -p vibe-resolver \
           -p vibe-publish -p vibe-check -p vibe-cli -p vibe-mcp
cargo run -p vibe-cli -- check --path . --quiet

# See the new workspace command.
cargo run -p vibe-cli -- workspace publish --help
```

## Pointer

[`spec/WAL.md`](spec/WAL.md) is the canonical **living** checkpoint. If anything here disagrees with the top of the WAL, trust the WAL. The detailed implementation record for M1.17 is [PROP-007 §9](spec/modules/vibe-workspace/PROP-007-workspace.md#impl); the workspace contract is PROP-007 §1–§7; the pre-implementation design lore is [`spec/design/workspace-and-qualified-naming.md`](spec/design/workspace-and-qualified-naming.md).
