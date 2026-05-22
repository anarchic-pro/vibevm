# CONTINUE.md — cold-resume checkpoint

_Written 2026-05-22 at session end (`ЗАВЕРШИ СЕССИЮ`). `main` is at
`e83c398`, even with `origin/main`, working tree clean._

> **`spec/WAL.md` is the canonical living state.** If this snapshot and the
> WAL ever disagree, the WAL wins — it is refreshed every session; this file
> is a point-in-time cold-start aid.

---

## TL;DR

This session shipped **PROP-008 Phase 2** — the group-qualified
package-identity refactor — and, by an owner decision taken mid-session,
folded **Phases 3 and 4** into the same commit. It landed on `main` as one
atomic `feat(core)` commit **`c5c4fe6`**, with a `docs(wal)` checkpoint
`e83c398` on top. `bash tools/self-check.sh` is green on all four steps.
The scaffolding branch `prop-008-phase2` is deleted.

**The change.** `PackageRef` is now
`{ kind: Option<PackageKind>, group: Option<Group>, name, version }`.
Package identity is `(group, name, version, content_hash)`; `kind`
(flow/feat/stack/tool) left identity entirely — it stays a mandatory
`[package]` field, but as pure metadata. Manifests store the kindless
qualified pkgref `org.vibevm/wal`. The registry is fully group-native:
`NamingConvention::Fqdn` is the new default (`org.vibevm.wal` repos),
resolution keys on `(group, name)`, the lockfile is schema v5,
`fixtures/registry/` is relaid-out under `org.vibevm/`.

**No blocker.** PROP-008 Phases 5–8 remain — index-backed short-name
resolution, collision detection, the `vibe-index` entry extension,
canonical-package migration + `VIBEVM-SPEC.md` + docs. The next session
picks one.

---

## Where work stands

- **Branch `main`:** at `e83c398`, even with `origin/main`, working tree
  clean. Gate green — `bash tools/self-check.sh` passes all four steps
  (`cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy
  --workspace --all-targets -- -D warnings`, `vibe check --path .`).
- **Branch `prop-008-phase2`:** **deleted** (local + origin). It was
  scaffolding for this refactor; its two WIP commits are squashed into the
  single `feat(core)` commit `c5c4fe6` on `main`.
- **Branch `m1.17-workspace`:** still retained on origin (merged long ago,
  never deleted) — harmless, ignorable.
- Test totals, all green, 0 failures: vibe-core 180, vibe-registry
  106 + 5 + 7, vibe-resolver 48, vibe-workspace 103, vibe-publish 51 + 5,
  vibe-check 27, vibe-mcp 22, vibe-index 169 (across its bin + e2e
  binaries), vibe-cli bin 124 / e2e 106 / cli_init 11 / cli_search 15
  (3 `cli_live_e2e` ignored — they need live registries).

## Active blocker

None. PROP-008 Phases 1–4 are shipped and green. Phases 5–8 are fresh
units of work, not blockers.

**Owner-only outward-facing work** (deferred, blocks nothing in-repo):
rename / re-publish the live `vibespecs` GitHub package repos and
re-lay-out the `vibespecstest1/2/3` test orgs into the `naming = "fqdn"`
shape (`org.vibevm.wal`, …). Until then, live-registry e2e against those
orgs would not resolve group-natively — but every hermetic test is
self-contained and green, so this gates nothing.

---

## Next steps — PROP-008 Phases 5–8

The identity core is done. Remaining, from PROP-008 §6's phase plan as
revised this session (Phase 4 was pulled forward into the Phase-2 commit):

- **Phase 5 — index-backed short-name resolution.** A CLI-boundary lookup:
  `vibe install wal` resolves the bare `wal` → `org.vibevm/wal` via the
  package index, then writes the qualified form into `[requires]`
  (PROP-008 §2.6). Manifests are always qualified; the short form is
  CLI-only sugar. Needs a small design pass on how the resolver enumerates
  `(*, name)` candidates across registries via the index.
- **Phase 6 — collision detection + exit code `7`.** When a short name
  matches two packages with different `group`, fail and list the
  alternatives; new exit code `7` ("ambiguous package"), distinct from `3`
  ("package conflict"). PROP-008 §2.7.
- **Phase 7 — `vibe-index` entry extension.** Add `group` and
  `workspace_origin` to the index entry schema (PROP-008 §2.8, PROP-005
  §2.6). NOTE: the index's on-disk `by-name/<kind>/<name>.json` layout
  still keys on the package's own metadata `kind` — Phase 7 should decide
  whether `by-name` re-keys on `group`/bare-`name` per §2.8.
- **Phase 8 — milestone close-out.** Migrate the three canonical packages
  (`flow-wal`, `flow-sync-from-code`, `flow-atomic-commits`) to
  `group = "org.vibevm"`; edit `VIBEVM-SPEC.md §7.1` (owner sanction is
  already recorded in the PROP-008 header — name-uniqueness moves from
  "within a kind" to "within a group", and the identity tuple + pkgref
  grammar update); update `CHANGELOG.md` and `ROADMAP.md` (neither records
  PROP-008 Phase 2 yet); docs sweep.

**Lightest starting point:** Phase 8's docs half — `CHANGELOG.md` /
`ROADMAP.md` / `VIBEVM-SPEC.md §7.1` for what already shipped — closes the
milestone's paper trail and needs no design work. Phase 5 is the next real
code unit and may want a short design pass first.

Recipe for whoever picks up cold:

1. Run the boot sequence (`CLAUDE.md` → `spec/boot/` → `spec/WAL.md`), then
   read PROP-008 (`spec/modules/vibe-registry/PROP-008-qualified-naming.md`)
   and PROP-005 (`spec/modules/vibe-index/PROP-005-package-index.md`).
2. Confirm green: `bash tools/self-check.sh`.
3. Pick a phase above; proceed under MFBT.

---

## Non-obvious findings (this session)

- **The kind/group registry tension — resolved by an owner decision.**
  PROP-008's phase plan put `naming = "fqdn"` in Phase 4, separate from
  Phase 2; but the same plan wanted Phase 2's manifests kindless
  (`org.vibevm/wal`). Those are incompatible — a kindless pkgref cannot
  resolve against a `kind-name`-keyed registry. The owner chose, via an
  explicit mid-session question, "full kindless now": pull Phase 4's
  registry-side work into the Phase-2 commit so the registry is
  group-native at once rather than half-migrated. This is why the landed
  commit `c5c4fe6` covers Phases 2 + 3 + 4.
- **`NamingConvention::Fqdn` is the new default.** Signature is now
  `repo_name(kind: Option<PackageKind>, group: &Group, name: &str)
  -> Result<String>` — `Fqdn` → `<group>.<name>` (infallible, uses only
  group). The legacy `KindName` / `Name` / `KindSlashName` conventions
  stay in the enum (PROP-008 §2.5 keeps them for non-group registries) but
  are non-default and unused by vibevm's own registries/fixtures; calling
  a legacy convention with `kind = None` is an error.
- **`vibedeps/<kind>-<name>/<version>/` slot directories kept `kind` in
  the path.** That is a PROP-009 (loading-model) schema, out of PROP-008's
  scope, deliberately left intact. Consequence: `vibe-workspace`'s
  `ResolvedDep` / `DependencyBoot` / `PublishNode` carry **both** `group`
  (identity) and `kind` (still needed to name the slot dir).
  `vibe-registry`'s `ResolvedPackage` carries only `group`.
- **`context(...)` predicates and subskill `if_present` tags keep the
  `kind:name` form.** They are activation-grammar tokens — the same opaque
  namespace as `interface:foo` / `capability:foo` — not package labels, so
  `org.vibevm/`-qualification does not apply. A conservative,
  behaviour-preserving choice; `vibe-core`'s grammar still accepts it.
- **Lockfile schema is v5.** A per-package `group` field;
  `Lockfile::find` / `find_mut` / `remove` take `(group: &Group,
  name: &str)`. The repo's own `vibe.lock` was bumped 4 → 5 (it is
  package-free, so only the schema envelope changed).
- **`RegistryError::UnqualifiedPkgref(String)`** is a new variant — raised
  when a pkgref reaches registry resolution without a `group`. A short ref
  must be qualified at the CLI boundary first; that boundary lookup is
  Phase 5, not yet built, so in practice every test uses qualified refs.
- **`fixtures/registry/` was relaid-out** from `<kind>/<name>/v<version>/`
  to `org.vibevm/<name>/v<version>/` — all six fixtures, via `git mv`
  (100 % renames, content preserved).
- **Two stale fixtures remain.**
  `fixtures/manual-test-packages/flow-vibevm-{direct-push,github}-smoke/vibe.toml`
  still use a pre-M1.18 schema; not exercised by `cargo test`; left
  untouched. Dead weight — deletion candidates.
- **The landing was a squash-merge.** `prop-008-phase2` carried two WIP
  commits (`a7d2238` + `45d9c41`); `git merge --squash` collapsed both
  into one `feat(core)` commit on `main`, so the intentionally-non-green
  WIP commit never entered `main`'s history.

---

## Repository map

```
vibevm/
├── CLAUDE.md / AGENTS.md / GEMINI.md   the four rules + boot directive (identical)
├── VIBEVM-SPEC.md                      owner-frozen implementation spec
├── ROADMAP.md  CHANGELOG.md  CONTINUE.md
├── .claude/settings.json               project Claude Code settings — bypassPermissions
├── Cargo.toml                          workspace root — members, shared deps, profiles
├── crates/
│   ├── vibe-core        core types: PackageRef/PackageKind/Group/CapabilityRef,
│   │                    the unified Manifest, Lockfile (schema v5), Purl, i18n
│   ├── vibe-cli         the `vibe` binary — every subcommand
│   ├── vibe-registry    git-backed registry, multi-registry resolver,
│   │                    IndexClient, compute_content_hash — now group-native
│   ├── vibe-resolver    dependency resolution — depsolver, features, activation
│   ├── vibe-workspace   workspace discovery, the loading model, the install
│   │                    orchestrator, vibedeps, freshness
│   ├── vibe-publish     publishing to GitHub / GitVerse, the post-publish index hook
│   ├── vibe-check       the spec linter (`vibe check`)
│   ├── vibe-index       the package index utility — server + CLI (a crates/ member)
│   ├── vibe-mcp         MCP server
│   ├── vibe-graph       task graph
│   ├── vibe-llm         LLM provider integration (M1.5 — deferred)
│   └── vibe-wire        JTD-generated wire types (src/generated/)
├── xtask/               build / maintenance tasks
├── spec/
│   ├── boot/            00-core.md, 90-user.md (authored) + generated INDEX.md
│   ├── common/          PROP-000 (process), PROP-004 (research), PROP-006 (modes)
│   ├── modules/         per-crate PROPs — PROP-008 (qualified naming) under
│   │                    modules/vibe-registry/
│   ├── design/          workspace-and-qualified-naming.md — the PROP-007/008 lore
│   ├── research/
│   └── WAL.md           the canonical living checkpoint
├── docs/                user-facing docs (commands/, loading-model.md, …)
├── fixtures/registry/   hermetic test-fixture packages — laid out
│                        org.vibevm/<name>/v<version>/ (group-native as of PROP-008)
├── manual-tests/        operator smoke recipes
├── tools/               self-check.sh, jtd-codegen
└── refs/                the owner's book + reference sources (read-only)
```

---

## Architectural / policy decisions in force

- **The four rules** (`CLAUDE.md`, authoritative `PROP-000 §12`): keep the
  repo human-authored (no AI attribution anywhere); Conventional Commits
  with a *why*-explaining body; group commits by meaning; autonomy on
  routine work only — stop and ask for history rewrites, force-push, large
  blobs, CI/signing/secrets, anything costly to reverse.
- **`.claude/settings.json` runs Claude Code in `bypassPermissions` mode**
  for this project — versioned, team-visible.
- **MFBT operating mode** (PROP-006 §2): when the owner says "move fast and
  break things", the agent works heads-down through testable phases with no
  mid-work confirmations; the four rules and the red-line escape hatch
  survive. This session's PROP-008 work ran under MFBT.
- **Language Rust, manifests TOML.** One `vibe.toml` per node; role set by
  section (`[project]` ⊕ `[package]`, optional `[workspace]`). Lockfile
  `vibe.lock`, **schema v5**. Four installable kinds: `flow` / `feat` /
  `stack` / `tool` — but `kind` is **metadata only**, not identity.
- **PROP-008 — qualified naming (M1.19): Phases 1–4 SHIPPED.** Identity is
  `(group, name, version, content_hash)`; reverse-FQDN `group` qualifier;
  pkgref grammar `[kind:][group/]name[@version]`; manifests store the
  kindless `org.vibevm/<name>`; registry is group-native with
  `NamingConvention::Fqdn` default. Phases 5–8 remain (short-name
  resolution, collision detection + exit code 7, index entry extension,
  migration + `VIBEVM-SPEC.md §7.1` + docs).
- **Loading model (PROP-009, M1.18).** Two physically separate trees —
  authored `spec/` and committed `vibedeps/`. The boot sequence is computed
  per node and projected into `spec/boot/INLINE.md` + `INDEX.md`. `vibe`
  owns one `<vibevm>` block inside `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`
  (PROP-012). The `vibedeps/<kind>-<name>/<version>/` slot layout still
  carries `kind` — a PROP-009 schema, untouched by PROP-008.
- **Decentralised registry (PROP-002).** Git-as-registry; content-hash
  identity; `[[registry]]` / `[[mirror]]` / `[[override]]`; redirect stubs.
- **Incremental install (PROP-011, M1.21).** `vibe install` is
  lockfile-respecting — skips the depsolver when `vibe.lock` is fresh,
  materialises only the changed `vibedeps/` slots.
- **The package index (PROP-005).** Opt-in; a derived hot cache — package
  repos stay authoritative, `content_hash` verified at fetch time.
- **Split-host posture.** vibevm source on GitVerse
  (`git@gitverse.ru:anarchic/vibevm.git`); the package registry org on
  GitHub (`github.com/vibespecs`).
- **M1.5 (LLM generation) is deferred.** Base-machinery-first: stabilise
  the package machinery before layering any generation on top.

---

## Recent commit chain (newest first)

```
e83c398 docs(wal): checkpoint PROP-008 Phases 1-4 shipped
c5c4fe6 feat(core): group-qualified package identity (PROP-008 Phase 2)
1ebd279 docs(wal): session-end checkpoint
744afa7 docs(continue): cold-resume checkpoint
cce7014 docs(wal): checkpoint PROP-008 Phase 2 — vibe-core migrated
8b8c4c6 docs(wal): record PROP-008 Phase 2 design + stashed WIP
73a5092 docs(wal): checkpoint PROP-008 Phase 1
9b662c5 feat(core): add the mandatory [package].group field
e167107 docs(continue): cold-resume checkpoint
7c1c090 docs(wal): session-end checkpoint
b84e61a build(self-check): gate cargo fmt --check
8cdbb65 style: apply rustfmt across the workspace
bbfc89d docs(wal): checkpoint the vibe-index fold
28172c5 docs(spec): reconcile PROP-005 and docs with the vibe-index fold
ea7e4d8 refactor(vibe-index): fold the crate into the workspace
ac5ce1d docs(changelog): record the PROP-005 package index milestone
5c4cc66 docs(wal): checkpoint the PROP-005 de-rot
40c9e0f docs(spec): reconcile PROP-005 and ROADMAP with the shipped index
9e3ee85 style(vibe-index): apply rustfmt across the standalone workspace
455795d refactor(vibe-index): retire the slice-1 skeleton scaffolding
c1f0a26 fix(vibe-index): realign the scanner with the current schema
f6e47bf docs: record the M1.5 deferral — stabilise the base first
8295333 docs(wal): checkpoint — M1.21 PROP-011 shipped
3f95333 docs: register M1.21 — incremental install
577e11d docs(spec): VIBEVM-SPEC §9.1 + PROP-011 — the shipped install contract
```

The PROP-008 Phase 2 work this session is `c5c4fe6` (the atomic refactor)
+ `e83c398` (the WAL checkpoint). The two pre-this-session WIP commits on
`prop-008-phase2` (`a7d2238`, `45d9c41`) were squashed away and the branch
deleted.

---

## Quick-start commands

```sh
# The full gate — must be green before any commit lands.
bash tools/self-check.sh

# Individual invariants.
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p vibe-cli -- check --path .

# Routine push (GitVerse SSH key picked up automatically in Git Bash).
git push origin main
```

---

## Pointer

`spec/WAL.md` is the canonical living state and supersedes this snapshot if
they diverge. The WAL's "Current phase" block carries the full PROP-008
status — Phases 1–4 shipped, the design decision that pulled Phase 4
forward, and the Phase 5–8 plan.
