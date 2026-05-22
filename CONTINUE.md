# CONTINUE — cold-resume checkpoint

_Written 2026-05-21. Snapshot for resuming from a fresh context (new
machine, new session, post-compaction)._

> **The WAL is the canonical living state.** `spec/WAL.md` is authoritative;
> if this file and the WAL ever disagree, the WAL wins. This document is a
> convenience snapshot — read `CLAUDE.md`, then `spec/boot/*` in filename
> order, then `spec/WAL.md`, then the relevant PROP docs, before working.

---

## TL;DR

**M1.18 — the PROP-009 loading model — Phases 1–6 are landed.** Phase 6
(`vibe reinstall` + published-copy boot regeneration) shipped this
session. The whole workspace builds and tests green.

**The next executable unit is M1.18 Phase 7 — migration + docs — and it
is blocked on one owner action:** explicit sanction to edit
`VIBEVM-SPEC.md` (§6, §4.2, §4.6, §13.1). Until that is granted,
Phase 7's spec-edit portion cannot land.

This session also produced **two new DRAFT design proposals** —
**PROP-010** (the local package cache + an `--offline` mode) and
**PROP-011** (incremental install) — committed, pushed, and registered
in `ROADMAP.md` / `spec/WAL.md` / `spec/modules/README.md`. Each needs
an owner design session to close its §5 open questions before it can be
implemented.

Branch `m1.17-workspace`, pushed to `origin`, working tree clean, in
sync. Not merged to `main` — the owner's call.

---

## Where work stands

- **Branch:** `m1.17-workspace`. **Pushed**, `origin/m1.17-workspace`
  in sync (0 ahead / 0 behind). Working tree **clean**.
- **Not merged to `main`.** The branch carries all of M1.17 + M1.18
  (Phases 1–6) + the PROP-010 / PROP-011 drafts. Merging is the owner's
  decision.
- **Gate green** (as of the Phase 6 commits — no code has changed since;
  the three PROP commits are docs-only):
  - `cargo test --workspace` — all green, **no `--exclude` needed**.
  - `cargo clippy --workspace --all-targets -- -D warnings` — clean.
  - Test counts: vibe-cli 253 (bin 123, e2e 104, cli_init 11,
    cli_search 15, 3 ignored), vibe-core 161, vibe-workspace 69,
    vibe-registry 106 + 5 + 7, vibe-publish 51 + 5, vibe-resolver 48,
    vibe-check 25, vibe-mcp 22.
- Routine commits + pushes land without asking from this branch (Rule 4
  plus the owner's standing authorisation). Non-routine red lines
  (force-push, history rewrite, large blobs, CI/signing/secrets,
  irreversible ops) still need explicit owner confirmation.

---

## WHAT NEEDS TO BE DONE — the plan

This is the heart of the checkpoint. Read it carefully.

### A. Owner actions — these gate everything else

1. **Grant explicit sanction to edit `VIBEVM-SPEC.md`** — §6 (boot
   model), §4.2 (layout), §4.6 (effective spec), §13.1 (package
   layout) — for PROP-009. **This is the blocker for M1.18 Phase 7.**
   PROP-009 §5 item 8 records that Phases 1–6 did not need it but
   Phase 7 does. Not yet granted.
2. **Decide the `when` contract gap.** PROP-009 §2.3 shows a `when`
   activation condition on a `dynamic` `INDEX.md` entry, but §2.6 pins
   no manifest field that declares it. `vibe-workspace`'s
   `boot_artifacts.rs` renderer is `when`-ready but always leaves it
   `None`. Decide where a dynamic boot contribution's `when` is
   declared (likely `[boot_snippet]` or the `[requires.packages]`
   entry). Small; best taken alongside Phase 7.
3. **Hold an owner design session for PROP-010**
   (`spec/modules/vibe-registry/PROP-010-local-package-cache.md`) —
   close its 5 §5 open questions: cache layout (extracted
   version-keyed directories vs identity-indexed clones), command
   namespace (`vibe cache` vs `vibe registry cache`), staleness
   signalling, eviction policy, scaffolding UX hints.
4. **Hold an owner design session for PROP-011**
   (`spec/modules/vibe-workspace/PROP-011-incremental-install.md`) —
   close its 3 §5 open questions: the freshness oracle (what is
   digested, where the comparison baseline lives), slot integrity on
   the fast path (trust slot-presence vs a cheap `content_hash`
   spot-check), incremental re-resolution on a `[requires]` delta.
5. **Decide whether to merge `m1.17-workspace` → `main`.**

Items 1–2 unblock Phase 7. Items 3–4 unblock the new PROPs. Item 5 is
independent.

### B. Next executable unit — M1.18 Phase 7 (migration + docs)

Start once A.1 (sanction) is granted. Spec: PROP-009 §4 (migration) and
§7 phase 7. Concretely:

- **Existing-project migration** (PROP-009 §4) — on the first
  `vibe install` after the upgrade, `vibe` rewrites a pre-PROP-009
  project: dependency content moves out of authored `spec/` into
  `vibedeps/`; `NN-` boot files become categorised authored boot or
  generated artifacts; `INLINE.md` / `INDEX.md` are generated; the
  redirects are rewritten. There is no compatibility shim.
- **The vibevm self-migration** — this repository migrates too.
  `spec/boot/00-core.md` and `90-user.md` stay user-owned authored
  boot; generated `INLINE.md` / `INDEX.md` join them. **Delicate:**
  `CLAUDE.md` / `AGENTS.md` / `GEMINI.md` are today hand-authored
  ~200-line files carrying the four non-negotiable rules and the
  session-end command. After migration they become thin generated
  redirects, so the rules must first move into an authored boot file.
  This touches user-owned territory — **do it with the owner, not
  unilaterally.**
- **`VIBEVM-SPEC.md` edits** — §6 / §4.2 / §4.6 / §13.1, under the A.1
  sanction.
- **`ROADMAP.md` / `CHANGELOG.md`** — flip M1.18 status, log the
  shipped loading model.
- **`docs/` sweep** — including a new `docs/commands/reinstall.md`
  (the `vibe reinstall` command shipped in Phase 6 with only its clap
  `--help` text as in-binary documentation).

Phase 7 gate, as every phase: `cargo test --workspace` +
`cargo clippy --workspace --all-targets -- -D warnings` stay green.

### C. After Phase 7

M1.18 is essentially closed. **Phase 8 (the effective-spec view) is
v1.5 scope** (PROP-009 §2.8) — it shares the computed-view engine and
rides with the M1.5 "Generation" milestone; it is not a standalone unit
now.

### D. Beyond M1.18 — the fork

- **PROP-011 (incremental install)** — no dependency beyond the shipped
  PROP-009; it can be scheduled early. The strong candidate for the
  unit right after M1.18. Needs design session A.4 first. Why it
  matters: today every `vibe install` re-resolves the whole graph and
  re-copies the whole dependency corpus — PROP-011 makes it incremental
  and makes `vibe install` lockfile-respecting.
- **The cache chain:** PROP-005 (the package index) implementation →
  **M1.19 PROP-008** (qualified naming) → **M1.20 PROP-010** (the
  cache). PROP-010 is keyed by PROP-008's qualified package identity,
  so it must follow PROP-008; PROP-008 needs PROP-005 for short-name
  resolution.

---

## The loading model in one breath

Two physically separate trees: authored `spec/` (only the human writes
it) and a committed `vibedeps/` (only `vibe` writes it — one slot
`vibedeps/<kind>-<name>/<version>/` per resolved package, the package's
published tree verbatim). The boot sequence is **computed** per node
from the unified resolution: inherited foundation + the node's own boot
+ dependency boot + user overrides. `vibe install` generates, per
entry-point node, `spec/boot/INLINE.md` (verbatim concatenation of
`inline`-linked contributions — read first, the priority lane) and
`spec/boot/INDEX.md` (a TOML manifest of `static` paths + `dynamic`
INCLUDE pointers), plus thin `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`
redirects. Three inclusion types — `inline` / `static` / `dynamic` —
set per dependency in `vibe.toml` (`link = …`, default `static`). The
`NN-` filename prefix is retired; `vibe` owns ordering by
`[boot_snippet].category` band (`foundation` → `flow` → `stack` →
`user-override`). `[writes]` is retired. `vibe reinstall [<path>]
[--force]` regenerates without re-resolving (Phase 6). `vibe workspace
publish` regenerates each staged copy's boot for the standalone
published shape (Phase 6, §2.11). A single-package project is a
degenerate one-node workspace.

---

## Non-obvious findings discovered this session

- **`vibe init` creates an empty `vibe.lock`.** A `vibe.lock`-absent
  project is therefore rare. `vibe reinstall`'s `load_lockfile` is
  written tolerant of an absent lockfile (treats it as empty) — a
  `vibe reinstall` on a dependency-less project just regenerates boot
  from the authored `spec/boot/`.
- **Two cache layers, easy to conflate.** The per-project `.vibe/cache`
  (the `cache_root` passed to `resolve_and_fetch`) is the
  `LocalRegistry` path's content cache. The multi-registry path puts
  registry clones + content under the *registry-level* cache
  (`VIBE_REGISTRY_CACHE`, or `vibe_registry::default_cache_root()`).
  `vibe reinstall --force` wipes `.vibe/cache`; the registry-level
  cache's integrity is guarded by `content_hash` instead. PROP-010
  elevates the registry-level cache to a first-class store.
- **`vibe install` always re-resolves.** There is no
  lockfile-freshness gate — `install.rs` runs the depsolver every time
  and rebuilds `vibe.lock` wholesale (its own comment: "vibe install
  re-resolves the whole graph"). Consequence: `vibe install` silently
  bumps a package within its `^` constraint on every run. PROP-011 §2.2
  fixes this — skip-when-fresh makes `vibe install` lockfile-respecting.
- **`apply_resolution` re-materialises every slot unconditionally.**
  `vibedeps::materialise` always does `remove_dir_all` + a full
  recursive copy, even when the slot already holds the correct
  immutable version. PROP-011 §2.3 makes it skip unchanged slots.
- **Boot regeneration is cheap and git-churn-free.** `write_boot_artifacts`
  does plain `fs::write`; git tracks content, so a byte-identical
  regeneration produces no diff. Whole-tree boot regeneration is
  therefore fine — the expensive parts are the depsolver and
  materialisation. PROP-011 §2.4 deliberately keeps boot regeneration
  whole-tree (owner-confirmed).
- **`vibe install` is whole-workspace and location-independent.**
  `Workspace::discover` always bubbles to the absolute root; run from
  the root or from a deep member, `vibe install` does the same whole
  thing. `-p <member>` scopes resolution *reporting* only — the
  materialisation and the single root `vibe.lock` are always
  workspace-wide.
- **§2.11 published-copy interpretation.** `vibe workspace publish`'s
  `stage_node` regenerates each staged copy's boot as a *standalone
  node* — own authored boot only, no inherited foundation, no
  materialised dependencies — because a published copy carries no
  `vibedeps/` tree and a consumer regenerates its own boot on install.
  A judgment call on PROP-009 §2.11's wording; flagged to the owner,
  not objected to.
- **`node_own_boot` is now `pub(crate)`** in
  `vibe-workspace/src/install.rs` — `publish.rs`'s
  `regenerate_published_boot` reuses it.
- **The ROADMAP was stale on milestone numbers** — fixed this session.
  The milestone map is now: **M1.18 = PROP-009** (loading model),
  **M1.19 = PROP-008** (qualified naming), **M1.20 = PROP-010** (the
  cache), **M1.21 = PROP-011** (incremental install — number nominal,
  resequenceable earlier).

---

## Repository map

```
vibevm/                     Rust workspace — the `vibe` CLI (12 crates)
├── CLAUDE.md / AGENTS.md / GEMINI.md   session-boot rules (kept identical)
├── MEMORY.md               pointer to spec/boot/90-user.md
├── CONTINUE.md             this file — cold-resume snapshot
├── ROADMAP.md              milestone roadmap (M0 … M1.21, M1.5, M2, M3+)
├── CHANGELOG.md            shipped-change log
├── VIBEVM-SPEC.md          product spec (owner-sanctioned edits only)
├── Cargo.toml              workspace manifest
├── crates/
│   ├── vibe-cli/           the `vibe` binary — commands, CLI, dispatch
│   │   └── src/commands/   install / uninstall / update / reinstall /
│   │                       workspace / registry / mcp / show / …
│   ├── vibe-core/          manifest + lockfile + package types (schema)
│   ├── vibe-graph/         dependency-graph types
│   ├── vibe-registry/      registry resolution, fetch, CachedPackage,
│   │                       MultiRegistryResolver, the registry cache
│   ├── vibe-resolver/      the depsolver (NaiveDepSolver, ResolvedGraph)
│   ├── vibe-llm/           LLM-provider integration
│   ├── vibe-mcp/           the MCP server
│   ├── vibe-check/         `vibe check` — project/manifest validation
│   ├── vibe-publish/       publishing + post-hook index submission
│   ├── vibe-wire/          generated wire/JSON types
│   └── vibe-workspace/     workspace discovery + THE LOADING MODEL:
│       └── src/
│           ├── lib.rs           Workspace::discover / load, node iteration
│           ├── vibedeps.rs      the vibedeps/ materialisation layout
│           ├── boot.rs          compute_effective_boot — the view engine
│           ├── boot_artifacts.rs  render INDEX.md / INLINE.md / redirects
│           ├── install.rs       apply_resolution / regenerate_boot /
│           │                    regenerate_boot_from / node_own_boot
│           └── publish.rs       select / topo_order / stage_node
│                                (stage_node now regenerates published boot)
├── fixtures/registry/      test-fixture packages (PROP-009 manifest shape)
├── services/vibe-index/    the index service — OUTSIDE the cargo workspace
└── spec/
    ├── boot/               session-boot files (00-core.md … 90-user.md)
    ├── common/             PROP-000 (process), PROP-004 / PROP-006, …
    ├── modules/            per-crate PROPs — see spec/modules/README.md:
    │   ├── vibe-registry/  PROP-001, PROP-002, PROP-008, PROP-010
    │   ├── vibe-resolver/  PROP-003
    │   ├── vibe-index/     PROP-005
    │   └── vibe-workspace/ PROP-007, PROP-009, PROP-011
    ├── design/             non-normative rationale
    └── WAL.md              ← canonical living state. Read this.
```

---

## Architectural / policy decisions in force

- **The four CLAUDE.md rules are non-negotiable every session.** (1)
  Never attribute authorship to any AI/machine system anywhere —
  commits, trailers, branches, code, docs. (2) Conventional Commits.
  (3) Group commits by meaning, one logical unit each. (4) Autonomy on
  routine changes; stop and ask for non-routine (history rewrite,
  force-push, large blobs, CI/signing/secrets, irreversible ops).
- **`~/.vibevm/github.publish.token` is a surface-secret** — never
  printed to stdout/stderr/chat/logs/commits.
- **Project facts live in the repo**, never in a harness's global
  user-memory.
- **PROP-009 loading model** (the spine of M1.18): two trees,
  computed-per-node boot, `inline`/`static`/`dynamic` link types,
  `category` ordering bands, `vibedeps/` slots. `[writes]` and the
  `NN-` prefix are retired.
- **`vibe install` is whole-workspace, location-independent**;
  resolution is unified (one `vibe.lock`, one version per package).
- **Resolution vs application** (the spine of PROP-011): resolution
  *must* stay unified (the diamond problem — two members cannot
  silently disagree on a version); *application* (materialise +
  regenerate boot) can be made incremental. This is why per-member
  install is ruled out for resolution but per-subtree regeneration is
  a legitimate optimisation.
- **PROP-010 (DRAFT)** — the local package cache: machine-global,
  accretive, **keyed by PROP-008 qualified package identity** (so it is
  registry-config-independent and serves new modules / new projects); a
  global `--offline` policy flag; a **user-level default registry
  configuration** seeding new projects. Five §5 open questions remain.
- **PROP-011 (DRAFT)** — incremental install: skip the depsolver when
  `vibe.lock` is fresh (→ `vibe install` becomes lockfile-respecting);
  materialise only changed `vibedeps/` slots; boot regeneration stays
  whole-tree (the cheap phase). Three §5 open questions remain.
- **`vibe.lock` schema stays v4** — `LockedPackage.boot_snippet` and
  `files_written` still exist in the schema but are always `None` /
  empty under the loading model.
- **Milestone numbering:** M1.18 = PROP-009, M1.19 = PROP-008,
  M1.20 = PROP-010, M1.21 = PROP-011 (nominal).
- **MFBT** ("move fast and break things", PROP-006 §2) — a codeword
  the owner uses to pre-authorise heads-down execution with no mid-work
  confirmations; the four rules still hold.

---

## Recent commit chain (newest first)

```
987e4d4 docs: register PROP-010 / PROP-011 and align milestone numbers
040c8c3 docs(spec): PROP-011 — incremental install
9069f13 docs(spec): PROP-010 — the local package cache
95a0498 docs(wal): checkpoint — M1.18 Phase 6 complete
0706ae2 feat(workspace): regenerate published-copy boot artifacts     (Phase 6)
4606132 feat(cli): vibe reinstall — regenerate the loading model      (Phase 6)
50c2a43 docs(continue): cold-resume checkpoint — M1.18 Phase 5 done
1af02b1 docs(wal): checkpoint — Phase 5 follow-ups landed
85dbc9a feat(cli): scope vibe update to the named packages            (FU3)
b313829 refactor(cli): fold the vibe-install crate into vibe-cli      (FU5)
6ec47d2 feat(workspace): prune stale vibedeps/ slots on apply         (FU4)
1a55409 feat(cli): unified resolution across all workspace members    (FU2)
2f42776 refactor(core): retire [writes] and [boot_snippet].filename   (FU1)
b4ebd08 docs(wal): checkpoint — M1.18 Phase 5 complete
682e06d test(cli): rewrite the install e2e suite for vibedeps
72b87b9 build(install): disable the vibe-install test harness
a6e20db fix(cli): merge [requires] before regenerating boot
f208050 chore(repo): untrack .claude/settings.local.json
7347208 refactor(install): delete the [writes] mirror-layout path
93fd043 feat(cli): PROP-009 Phase 5 — rework uninstall and update
440a88c feat(cli): PROP-009 Phase 5 — vibe install onto vibedeps
830e8c1 docs(wal): checkpoint — M1.18 Phase 5 underway
f4d45a4 feat(workspace): PROP-009 Phase 5 — install orchestrator
7519d2c docs(wal): checkpoint — M1.18 Phase 4 (boot artifacts)
e06a5ff feat(workspace): PROP-009 Phase 4 — boot artifacts
```

This session added the top six: Phase 6 (`4606132`, `0706ae2`), its
WAL checkpoint (`95a0498`), the two new DRAFT PROPs (`9069f13`,
`040c8c3`), and their registration (`987e4d4`).

---

## Quick-start commands

```sh
# Build / gate the whole workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Focused
cargo test -p vibe-workspace            # the loading-model engine
cargo test -p vibe-cli --test cli_e2e   # install/uninstall/update/reinstall e2e
cargo build -p vibe-cli                 # the `vibe` binary

# Git
git status
git log --oneline -25
```

Platform note: Windows / PowerShell. `cargo test --workspace` runs
clean — the `os error 740` issue is long resolved (the `vibe-install`
crate no longer exists).
