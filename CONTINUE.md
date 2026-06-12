# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-12 at session end. **Two efforts completed this day,
both pushed:** (1) the PROP-013 discipline-depth audit (the INT-0001
window — category E added, 12 findings, the P1 specmap-hash wipe
fixed in-run); (2) **the depth program executed to completion** — all
seven filed P2s closed in a 24-commit series (`9f06fbf` … `99f1a62`),
the full five-gate panel green on the final tree. The session closed
by authoring the next plan:
[`spec/terraforms/SHRINK-PLAN-v0.1.md`](spec/terraforms/SHRINK-PLAN-v0.1.md)._

> **`spec/WAL.md` is the canonical living state.** If this snapshot
> and the WAL disagree, the WAL wins. Boot first (`CLAUDE.md` →
> `spec/boot/INDEX.md` → its files, including the two installed
> Discipline snippets → `spec/WAL.md`), then read this.

---

## TL;DR

The owner asked how deep the AI-Native Rust adoption really goes. The
audit answered: one crate deep — and found the merge panel's first
gate silently red on `main` (the committed `specmap.json` had lost
every content hash to a post-session history rewrite; fixed by
regeneration, `9f06fbf`). The owner then set the goal «вся программа
глубины должна быть выполнена до конца», and the program ran the same
day, largely via seven parallel agents verified by centralized gates:
DBT-0019 closed (the scanner reads `VIBEVM-SPEC.md`, 90 additive
anchors), 67 unit-typing lines (REQ fabric 5 → 72), 27 `implements` +
64 `verifies` tags, the `Registry` seam cell-ified (3 cells +
oracles), six god-file cuts executed, three new conform rules + one
widening landed ratcheted (`conform freeze` is new tooling), and the
130-entry baseline became the enumerated work queue. **Headline
numbers:** items 190→337, edges 198→347, verifies 40→104, cells 4→18,
units 352→442. The shrink plan (six phases, ~14 batches, exit 130→10)
is written and WAL-linked.

## Where work stands

- **Branch `main` @ `99f1a62`**, in sync with `origin/main`; working
  tree clean. (`new`, `m1.17-workspace` remain as retained merged
  branches.)
- **No active blocker.** The next unit of work is SHRINK-PLAN Phase 0
  (self-contained, ~one sitting); everything else is owner-gated.
- **Gate panel, all green on this tree** (each gate's own exit code
  captured; specmap+conform re-certified after the final `cargo fmt`):
  `cargo xtask specmap --check` — 442 units / 337 items / 347 edges /
  0 suspects / 0 gated orphans (10 DBT-0020 dispositions, 7 exempt
  crates); `cargo xtask conform check` — 130 frozen / 0 new (9 rules);
  `cargo xtask test-gate` — 1120 results / 0 failed / 3 skipped,
  xfail-strict; `cargo xtask fast-loop --enforce-budget` — 18/18 <60s;
  `bash tools/self-check.sh` — fmt, workspace tests, clippy -D
  warnings, `vibe check` 0/0/0.

## Next steps (exact recipe)

1. **SHRINK-PLAN Phase 0** (start here; one sitting):
   - `cargo xtask conform freeze` → `git diff conform-baseline.json`
     must show exactly −3 `file-length` lines (search.rs / output.rs /
     git_registry.rs went under 600 after fmt) and nothing added.
   - Doctest on `GitBackend`
     (`crates/vibe-registry/src/git_backend/mod.rs:94`) — kills the
     lone `seam-has-doctest` entry.
   - Frontend v4: `UnwrapUse` gains `in_deviation` (track
     `spec(deviates…)` attrs like `test_depth` in
     `crates/conform-frontend-rust/src/lib.rs`); bump version "3"→"4";
     rule skips deviating sites. Prerequisite for Phase 2.
2. Then Phases 1–5 per the plan: R-001 wiring (`commands/install.rs`
   stops constructing `LocalRegistry`; the match moves to
   `crates/vibe-cli/src/registry.rs`) → 24 unwraps
   (convert/testify/cfg-test) → 68 messages to the fixed grammar
   `(violates spec://…; fix: …)` → 23 file shrinks (tests-out lever
   first) → `PackageScanner` seam. Order is load-bearing (enums
   touched once; strings settle before files move).
3. **Owner-court items** (any time): the history-rewrite question
   (what tool re-hashed the adoption-day chain and emptied
   specmap.json's hashes — audit -01 rider, still unanswered);
   publishing the two Discipline packages; production solver selection
   (R-001 flag `solver=sat`); PROP-010 design session; DBT-0020 (MCP
   spec home; two files parked behind it); the four open-instrument
   predictions; PROP-014 external-namespace amendment; Discipline
   v0.3 (inputs: `terraform/adopt-v0.3/REPORT.md` + today's audit).

## Non-obvious findings (this session)

- **A committed derived artifact can rot via history rewrite.** The
  post-session rewrite of 2026-06-11 re-hashed every adoption-day
  commit AND re-serialized `specmap.json` with all 352 `content_hash`
  fields emptied — gate #1 was red on a clean checkout while believed
  green, and editorial-drift detection had no baseline. Probe
  empirically on a clean tree; never trust "the panel was green
  yesterday" across a rewrite. (PROP-013 §2.2 E4 now encodes this.)
- **cfg(test)-scoped counting changed the unwrap story by ~50×**: the
  raw census suggested 1300+ unwraps; honest domain count in gated
  crates is **24**. Don't act on upper bounds; build the precise fact
  first.
- **R-002 catches what authors rationalize**: manifesting GitRegistry
  as a cell instantly turned an existing helper import into a
  sibling-cell violation; fixed by extracting `registry_cache.rs`.
- **Windows UAC blocks test exes named \*install\***: the e2e install
  cluster is `tests/cli_pkg_cycle.rs` (os error 740 otherwise) — same
  machine behavior PROP-007 §9.5 recorded.
- **PowerShell 5.1 corrupts UTF-8-no-BOM round-trips** (decodes as
  ANSI): repo file edits only via UTF-8-explicit writes or the
  editing tools; recover with `git restore`. One spec file was
  briefly mangled and restored this session.
- **`bash` in PowerShell resolves to WSL**, not Git Bash —
  `tools/self-check.sh` must run through Git Bash.
- **Clippy doc-lazy-continuation**: a doc line starting with `> 0`
  parses as a blockquote; rephrase ("Nonzero…").
- **`conform freeze` legality** (now tooling): rewrite the baseline
  only when a new rule lands (its pre-existing findings freeze once)
  or after work that shrank the set; the `git diff` review is the
  guard.

## Repository map (delta vs the adoption era)

```
vibevm/
├── spec/terraforms/SHRINK-PLAN-v0.1.md   ← THE NEXT PLAN (six phases)
├── conform-baseline.json                 ← 130 entries = the work queue
├── VIBEVM-SPEC.md                        ← now anchored ({#…} × 90) and scanned
├── crates/
│   ├── vibe-registry/src/
│   │   ├── local_registry.rs             ← cell: Registry/local
│   │   ├── git_registry.rs               ← cell: Registry/git-monorepo
│   │   ├── registry_cache.rs             ← shared cache helpers (R-002 fix)
│   │   ├── git_package_registry/         ← cell: Registry/git-per-package
│   │   │   └── {mod,auth,urls,fetch}.rs      (was one 2700-line file)
│   │   └── multi_registry_resolver/
│   │       └── {mod,walk,redirect_follow,sources,refresh}.rs
│   ├── vibe-check/src/
│   │   ├── lib.rs                        ← types + Check trait + all_checks()
│   │   └── checks/*.rs                   ← 11 cells: Check/<variant>
│   ├── vibe-core/src/manifest/package.rs ← 597-line hub
│   │   └── package/{when,deps,features,wire}.rs
│   ├── conform-core/src/
│   │   └── {facts,store,finding,rules,sarif,baseline}.rs  (was one file)
│   └── vibe-cli/
│       ├── src/commands/registry/        ← {mod,sync,config,publish,vendor,redirect}.rs
│       └── tests/                        ← cli_pkg_cycle / cli_redirect /
│                                           cli_workspace_publish /
│                                           cli_registry_mgmt + common/
├── terraform/registry/{debt.json,DEBT.md}  ← DBT-0019 fixed; DBT-0020 (MCP) open
└── specmap-ratchet.json                  ← 7 exempt; 10 dispositions (DBT-0020)
```

## Decisions in force (this session's additions, long form)

- **The baseline is the work queue, not absolution.** Every conform
  rule lands ratcheted; the file only shrinks outside a new-rule
  landing; every shrink is freeze + diff review.
- **Gates with their own exit codes; re-run after fmt; probe on clean
  trees.** The panel certifies a tree, not a day.
- **The product-error grammar is fixed** (SHRINK-PLAN §4): human text
  first, machine tail `(violates spec://…; fix: <hint>)` appended.
- **MCP is parked by owner instruction** (2026-06-12): DBT-0020 and
  both MCP files wait for a spec home; no honest edge → no edge.
- **`CONFORM_GATED` expansion (vibe-core, vibe-index) is the NEXT
  plan's opening move** — one queue closes before another opens.
- **Cells everywhere the seam is real**: Registry (3 variants),
  Check (11), DepSolver/DepProvider (4). Cell selection sites live in
  the R-001 registry module; the frozen R-001 finding tracks the one
  residual (LocalRegistry in install.rs — SHRINK-PLAN Phase 1).
- All prior-era decisions stand (four rules; spec-first flow; no CI by
  owner decision; xfail-strict; derived data never committed;
  `specmap.json` regenerated with every unit/tag move).

## Recent commit chain (newest first; all 2026-06-12)

```
99f1a62 docs(wal): the shrink backlog gains its plan pointer
3926b69 docs(spec): the shrink plan - draining the depth-program baseline
e93bfac docs(wal): depth-program checkpoint - executed to completion
f11ed38 docs(audit): same-day dispositions - the depth program closed its P2s
3a4521e chore(spec): index refresh - the depth program's traceability state
4157c79 chore(conform): freeze the depth-program rule baselines
9f14aed test(spec): verifies edges link the strongest suites to their REQs
1171e7d chore(spec): the affirmation sweep - implements edges past the resolver
b9be0c2 test(cli): cli_e2e.rs splits into four feature binaries
503e8df refactor(core): manifest/package.rs sheds its wire and dep tangles
cf66dc8 refactor(check): the Check seam - eleven checks become cells
da78115 refactor(conform): the engine leaves its single file
f66f9b8 refactor(cli): commands/registry.rs decomposes into six modules
e160906 refactor(registry): the Registry seam becomes three cells
fe787d0 feat(conform): the depth-program rule wave - 3 rules, 1 widening
24f668f docs(spec): type the implemented PROPs' decision units at REQ grain
3c0aecf build: lockfile entry for vibe-cli's specmark dependency
1173a76 chore(spec): close DBT-0019 - trio tagged, vibe-cli enters the gate
4f87c9a docs(spec): anchor every VIBEVM-SPEC.md heading - 90 units
543a5e1 feat(specmap): scan VIBEVM-SPEC.md as a root spec document
18f0a62 docs(wal): audit-window checkpoint
21d4769 docs(audit): 2026-06-12 run - the discipline-depth inventory
9afd8b8 docs(spec): PROP-013 gains category E - discipline depth
9f06fbf fix(spec): regenerate specmap.json - restore unit content hashes
d872832 docs(continue): session-end cold-resume checkpoint   (prior session)
```

## Quick-start

```sh
cargo xtask specmap --check              # index + orphan ratchet
cargo xtask conform check                # facts → 9 rules → SARIF → baseline
cargo xtask conform freeze               # rewrite baseline (legality: see plan)
cargo xtask test-gate                    # nextest, xfail-strict
cargo xtask fast-loop --enforce-budget   # 18 cells < 60s
bash tools/self-check.sh                 # via Git Bash, NOT WSL
cargo xtask trace explain <symbol|uri> [--text|--json|--prose]
```

Session-resume phrase: `восстанови сессию`. Resume work at
`spec/terraforms/SHRINK-PLAN-v0.1.md` Phase 0. The WAL supersedes
this snapshot wherever they diverge.
