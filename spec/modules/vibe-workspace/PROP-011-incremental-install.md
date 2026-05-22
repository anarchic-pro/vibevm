# PROP-011: Incremental install — skip resolution when fresh, materialise only the diff {#root}

**Milestone:** design proposal; targets a future milestone (owner to place in [`ROADMAP.md`](../../../ROADMAP.md)). It refines the install machinery of [PROP-009](PROP-009-loading-model.md) (M1.18, Phases 1–6 shipped) and has **no further dependency** — it does not wait on PROP-008 or PROP-010, so it can be scheduled early. Not implementation-locked.
**Status:** DRAFT — requirements captured in owner discussions on 2026-05-21; the three §5 design questions were resolved in an owner session on 2026-05-22 (draft 3 — see §8). Implementation-ready.
**Related:** [PROP-009](PROP-009-loading-model.md) (the loading model — `apply_resolution`, `regenerate_boot`, `vibedeps::materialise`, the `vibe install` orchestration this PROP refines; §2.10 `vibe reinstall`); [PROP-007](PROP-007-workspace.md) (workspaces — unified resolution, the matryoshka); [PROP-010](../vibe-registry/PROP-010-local-package-cache.md) (the local cache — skip-when-fresh makes the common path offline-clean for free, §2.6 there).
**Owner sanction:** this PROP changes `vibe install`'s observable contract (it becomes lockfile-respecting — §2.2) and so edits `VIBEVM-SPEC.md` §9.1. The spec edit lands at implementation time and requires explicit owner sanction — not yet granted; this PROP is the requirements record.

---

## 1. Motivation {#motivation}

PROP-009 made `vibe install` **correct**: run anywhere in a workspace, it re-resolves the whole graph, re-materialises every `vibedeps/` slot, and regenerates every node's boot artifacts. Correctness-first — "regenerate everything deterministically" is obviously right and self-healing.

It is also **whole-tree, unconditionally**. Every `vibe install` — regardless of what changed, or whether anything dependency-relevant changed at all — re-runs the depsolver (a registry walk, network), and `vibedeps::materialise` does a `remove_dir_all` followed by a full recursive copy of *every* package tree. For a large workspace that is a heavy operation paid on every invocation.

A developer — or, increasingly, an agent — iterating fast inside a large project is blocked by this. Most edits do not need `vibe install` at all: PROP-009's boot artifacts are *path manifests*, not content copies, so editing spec content never changes them — authoring is already decoupled from installing. But when `vibe install` *is* needed (a dependency declaration changed), it must be cheap, and today it is not: it pays whole-tree cost for a one-subtree change.

The fix is standard package-manager practice. `cargo build` and `npm install` treat the lockfile as a freshness oracle: work the lockfile proves unchanged is skipped; only the diff is touched. PROP-011 brings that discipline to `vibe install`.

---

## 2. Decisions {#decisions}

### 2.1 Separate resolution from application {#two-phases}

**Decision.** `vibe install` is understood as two phases, optimised independently — the current code conflates them.

- **Resolution** — the depsolver: read every node's `[requires]`, pick one version per package. It **must stay unified** (one `vibe.lock`, one version per package across the workspace — the diamond problem; PROP-007 §2.4). It cannot be computed per-subtree. But it *can be skipped entirely* when its inputs are unchanged (§2.2).
- **Application** — materialise the resolution into `vibedeps/`, then regenerate boot artifacts. This does **not** have to be whole-tree. It can be a diff: materialise only the slots that changed (§2.3); boot regeneration is cheap and stays whole-tree (§2.4).

Resolution being unified does not force application to be whole-tree. PROP-011 keeps unified resolution and makes everything around it incremental.

### 2.2 Skip resolution when the lockfile is fresh {#skip-resolution}

**Decision.** Before running the depsolver, `vibe install` performs a **freshness check**: it compares the resolution inputs — the union of every workspace node's `[requires]` (registry, git, path, and resolved `var` packages) — against what the current `vibe.lock` was generated from. If they are unchanged, the depsolver is **not run**: the resolution is exactly what `vibe.lock` already records, and the run proceeds straight to application (§2.3) against the locked versions.

This makes a `vibe install` where no dependency declaration changed cost only: discover the workspace, run the freshness check, apply. No network, no version re-selection — milliseconds even on a large workspace.

It also fixes an observable wart. Today `vibe install` always re-resolves, so it silently bumps a package within its constraint on every run (a `^0.3` pin drifts to the newest `0.3.x` available). With the freshness check, **`vibe install` becomes lockfile-respecting**: unchanged `[requires]` ⇒ the locked versions are honoured verbatim. `vibe update` remains the explicit "re-resolve and pick newer" command. This aligns `vibe install` with the `cargo build` / `npm install` contract — *install respects the lock; update moves it* — and makes a build reproducible.

When `[requires]` *has* changed, resolution runs **incrementally** (§5.3) — holding the existing `vibe.lock` pins and resolving only the delta, reusing the scoped-resolution machinery `vibe update <pkgref>` already has (PROP-009 FU3); a full re-resolve is the fallback when the delta cannot be isolated. The freshness check itself adds no `vibe.lock` field: the lockfile *is* the baseline, and the check is a `cargo`-style **satisfiability test** of the locked versions against the current `[requires]` — see §5.1.

### 2.3 Materialise only the diff {#materialise-diff}

**Decision.** The materialisation step skips any `vibedeps/` slot that already exists on disk for the resolved `(kind, name, version)`. Versions are immutable (PROP-002), so a slot present for the exact resolved version is correct content; the `remove_dir_all` + full recursive copy is pure waste. Only **new** slots and **version-bumped** slots are materialised; slot pruning (PROP-009 FU4 — dropping orphaned slots) is unchanged.

For a `vibe install` that changed one subtree, this turns a re-copy of the whole dependency corpus into a copy of just the handful of slots that actually moved.

The skip trusts slot-presence-for-a-version as a proxy for correctness — by default it does not re-hash the slot. That is deliberate: hashing every slot on every install would defeat the optimisation, and the integrity escape hatch already exists (`vibe reinstall --force`, §2.5, re-fetches and re-copies unconditionally). Whether the fast path additionally verifies a slot's `content_hash` before trusting it is a **configurable strategy** — the `slot_integrity` setting, `trust-presence` by default (§5.2).

### 2.4 Boot regeneration stays whole-tree — and why that is fine {#boot-regen}

**Decision.** Boot-artifact regeneration (`regenerate_boot` over every node) is **kept whole-tree**. It is not the expensive part, and scoping it is low-value:

- It is cheap: per node, an in-memory topological sort plus writing `INDEX.md` (a small TOML), the redirects (~1 KB each), and `INLINE.md` only when the node has inline dependencies. The cost is `O(nodes)` small operations, not the corpus-sized I/O of materialisation.
- It does not churn git: git tracks content, so regenerating a byte-identical `INDEX.md` produces no diff.
- Whole-tree regeneration is **self-healing**: a boot artifact left stale by an earlier bug is silently corrected on the next install. A scoped regeneration would preserve such staleness.

Scoping boot regeneration to the affected set — a changed node plus its ancestors, the shape PROP-009 §2.10 already specifies for `vibe reinstall` — is *possible* but **out of scope by owner decision**: it is the cheap phase, and effort belongs on §2.2 and §2.3. It is recorded here only so the option is not lost: should a workspace ever grow large enough that `O(nodes)` small writes genuinely matter, §2.10's node-plus-ancestors shape is the ready answer. It is not a deliverable of this PROP.

### 2.5 Bypasses — no new flag {#force}

**Decision.** PROP-011 adds **no force flag to `vibe install`**. The two skips (§2.2, §2.3) each already have an explicit, named bypass:

- to re-resolve even though `[requires]` is unchanged — `vibe update` (re-resolves and may pick newer versions; PROP-009 §2.7 / FU3);
- to re-materialise even though the slots are present — `vibe reinstall --force` (re-fetches from source and overwrites `vibedeps/`; PROP-009 §2.10).

The skips are safe precisely *because* these bypasses exist. Keeping them as the bypass — rather than adding `vibe install --force` — avoids a redundant flag and keeps each command's job distinct.

---

## 3. Command and crate surface {#surface}

- `vibe-workspace` — the freshness check feeding `apply_resolution`; `vibedeps::materialise` (or its caller) gains the slot-present skip; the install orchestration learns the resolution-skip path.
- `vibe-cli` — `vibe install` wires the freshness check ahead of the depsolver; its report distinguishes "unchanged — nothing re-resolved" from a real apply.
- `vibe.lock` — unchanged; the lockfile *is* the freshness baseline (§5.1), so no schema bump and no new field.
- vibevm user configuration — a new `[install] slot_integrity` key (`trust-presence` default, or `verify-hash`) selects the §2.3 fast-path integrity strategy (§5.2). Set once, persists across runs.
- No change to `vibe update` or `vibe reinstall` beyond their role as the §2.5 bypasses.

---

## 4. Migration {#migration}

None. PROP-011 is purely an optimisation of an existing, correct operation — the output of `vibe install` is unchanged for any input where `[requires]` actually changed, and for an unchanged input the output is what `vibe.lock` already pinned. The one observable change is intentional and improving: `vibe install` stops drifting versions within a constraint (§2.2). Existing lockfiles are read as-is; a freshness-input digest, if added, is an optional `meta` field absent lockfiles simply force one resolution.

---

## 5. Resolved questions {#open}

The three questions opened in draft 2 were resolved in an owner design session on 2026-05-22 (draft 3).

1. **The freshness oracle — `cargo`'s model.** No digest field is added to `vibe.lock`; the lockfile *is* the baseline. The freshness check is a **satisfiability test**, the shape `cargo` uses: re-read every node's `[requires]`, and the lock is fresh iff every declared dependency has a `[[package]]` entry whose pinned version satisfies the current constraint (registry / resolved `var`) or whose `source_url` + `source_ref` match the declared git ref or local path. The declared root set must equal `meta.root_dependencies`. Transitive packages are trusted — they were resolved from roots, and an unchanged root set cannot have produced a different transitive closure (a transitive `[requires]` lives inside a `vibedeps/` slot, immutable once materialised). The check uses what the lock already records — versions *and* per-package `content_hash` — and needs no schema bump. It is content-based, never mtime-based (§6).
2. **Slot integrity on the fast path — a configurable strategy.** The fast-path slot skip (§2.3) is governed by a **`slot_integrity` setting** in the vibevm user configuration, chosen once and persisted. Two values: `trust-presence` (the **default** — skip a slot present for the resolved version, no hashing) and `verify-hash` (additionally verify the slot's content against the lock's `content_hash` before skipping, re-materialising on a mismatch). The default keeps the fast path fast; `verify-hash` is `cargo`'s always-verify discipline for an operator who wants it without reaching for `--force`. A project-level override is a possible later extension, not v1 scope.
3. **Incremental re-resolution.** When `[requires]` has changed, resolution runs **incrementally** — holding the existing `vibe.lock` pins and resolving only the delta, reusing the scoped-resolution machinery `vibe update <pkgref>` already has (PROP-009 FU3). A full re-resolve is the fallback when the delta cannot be isolated.

Closed in draft 2: scoped boot regeneration — boot regeneration stays whole-tree (§2.4), the cheap phase is not optimised.

---

## 6. Rejected / deferred alternatives {#rejected}

- **Subset the resolution per member.** Rejected — resolution must be unified (one version per package; the diamond problem, PROP-007 §2.4). PROP-011 skips resolution when it is provably unneeded; it never resolves a subtree in isolation.
- **mtime-based freshness.** Rejected — file mtimes are not preserved across `git clone` / `git checkout`, so an mtime oracle would mis-fire constantly. The freshness check is content-based (§5.1).
- **Scope or otherwise optimise boot regeneration.** Out of scope by owner decision (§2.4) — boot regeneration is cheap, self-healing, and produces no git churn; effort goes to §2.2 and §2.3, the phases that are genuinely expensive.
- **A `vibe install --force` flag.** Rejected — `vibe update` and `vibe reinstall --force` are already the bypasses (§2.5); a third spelling would be redundant.

---

## 7. Phase plan {#phases}

1. **Skip resolution when fresh** — the content-based freshness check; `vibe install` skips the depsolver on an unchanged `[requires]`, becoming lockfile-respecting. The largest win and the observable-contract change.
2. **Materialise only the diff** — the slot-present skip in the materialisation step, with the `slot_integrity` user-config setting selecting `trust-presence` (default) or `verify-hash` (§5.2).
3. **Incremental re-resolution** — on a `[requires]` delta, hold existing pins and resolve the delta (reuse the FU3 scoped-resolution machinery).
4. **Docs + `VIBEVM-SPEC.md`** — the §9.1 edit (install respects the lock) under owner sanction; a `docs/` note.

Boot-regeneration scoping (§2.4) is out of scope by owner decision — not a phase.

---

## 8. Version history {#history}

- **2026-05-21 — draft 1.** Requirements captured in an owner discussion on incremental install: the resolution / application split (§2.1), skipping the depsolver when `vibe.lock` is fresh — which also makes `vibe install` lockfile-respecting (§2.2), materialising only changed `vibedeps/` slots (§2.3), and the deliberate decision to leave boot regeneration whole-tree because it is the cheap phase (§2.4).
- **2026-05-21 — draft 2.** Owner review: the §2.4 decision — boot regeneration stays whole-tree, the cheap phase is not optimised — confirmed and made firm; the corresponding draft-1 open question is closed. The PROP stands on its two substantive wins, §2.2 (skip resolution when fresh) and §2.3 (materialise only the diff). Three §5 open questions — the freshness oracle, slot integrity, incremental re-resolution — remain for a follow-up owner design session. Not yet implementation-ready.
- **2026-05-22 — draft 3.** The three §5 open questions resolved in an owner design session. The freshness oracle is `cargo`'s satisfiability model — the lockfile is the baseline, no new field (§5.1). Slot integrity on the fast path is a `slot_integrity` vibevm user-config setting, `trust-presence` by default (§5.2). A changed `[requires]` re-resolves incrementally, full re-resolve as fallback (§5.3). Implementation-ready.
