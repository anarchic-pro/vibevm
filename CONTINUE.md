# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-11 at session end. **The Discipline v0.2 adoption
(TERRAFORM-PLAN-v0.3) is COMPLETE**: all phases 0–7 plus the
priority-cell sweep executed this day on `main`, 22 commits
(`e3f06ec` … `1792c14`), pushed. Close-out:
[`terraform/adopt-v0.3/REPORT.md`](terraform/adopt-v0.3/REPORT.md).
This file carries everything a cold session needs._

> **`spec/WAL.md` is the canonical living state.** If this snapshot
> and the WAL disagree, the WAL wins. Boot first (`CLAUDE.md` →
> `spec/boot/INDEX.md` → its files → `spec/WAL.md`), then read this.
> Note the boot sequence now includes the installed Discipline
> snippets: 00-core → `vibedeps/flow-discipline-core/0.2.0/boot/…` →
> `vibedeps/stack-rust-ai-native/0.2.0/boot/…` → 90-user.

---

## TL;DR

The session opened on the owner's drop of the Discipline v0.2
package (which it committed on sight, DBT-0016 watch) and then ran
`spec/terraforms/TERRAFORM-PLAN-v0.3.md` to its §5 exit criteria in
one continuous effort. The core move was **self-hosting**: the
Discipline became two installed vibevm packages
(`flow:org.vibevm/discipline-core@0.2.0`,
`stack:org.vibevm/rust-ai-native@0.2.0`) resolved from the in-repo
`packages/` local registry — vibevm installs the Discipline through
the Discipline's own tool. All nine scaffold cards were applied with
implemented checkers; **DBT-0011 closed** (the `Sat` backtracking
solver, dominance-differential-pinned); composition predicates
ratified PROP-003 r1-planned → r2; 55+ canonical doctests now ride
the per-cell loop; the conform gate runs six rules over seven gated
crates. The prediction ledger holds 12 falsifiable predictions; the
REPORT's eight-item honest list is the input to Discipline v0.3.

## Where work stands

- **Branch `main` @ `1792c14`**, in sync with `origin/main`; working
  tree clean. (`new` and `m1.17-workspace` remain as retained merged
  branches from earlier eras.)
- **No active blocker.** Everything open is owner-gated or
  measurement-gated (next section); nothing is mid-flight.
- Gate panel, all green on `main` (run on the final tree, statuses
  captured — see the gate-invocation lesson below):
  `cargo xtask specmap --check` — 352 spec units / 190 tagged items /
  198 edges / 0 suspects / 6 known warnings; orphan ratchet 0 gated
  (6 DBT-0019 dispositions, 8 reasoned exemptions);
  `cargo xtask conform check` — 8 frozen / 0 new (rules: R-001,
  R-002, unsafe-gate, seam-has-doctest, error-enum-cites-req,
  cell-has-oracle; gated: vibe-resolver, conform-core, specmap-core,
  vibe-registry, vibe-workspace, vibe-check, vibe-publish);
  `cargo xtask test-gate` — xfail-strict green;
  `cargo xtask fast-loop --enforce-budget` — 18/18 cells < 60s;
  `bash tools/self-check.sh` — all four steps.

## Next steps (all owner- or measurement-gated; exact entry points)

1. **Publish the Discipline packages** to the public `vibespecs`
   registry — token, outward-facing, owner-only. Sources:
   `packages/org.vibevm/{discipline-core,rust-ai-native}/v0.2.0/`;
   recipe precedent in `spec/boot/90-user.md`.
2. **Production solver selection** — wire `Sat` vs `NaiveDepSolver`
   through the R-001 selection registry
   (`crates/vibe-cli/src/registry.rs`, flags are data with
   provenance/birth/sunset). `Sat` lives at
   `crates/vibe-resolver/src/sat.rs`; resolvo adoption stays open
   behind the `deviates` edge on its `DepSolver` impl.
3. **The four open-instrument predictions** (P2-1 iterations-to-green,
   P4-1 the central C-7 transfer test, P5-1 behavior half, P6-1
   parameterization half) await a measured weak-agent run — the
   instrumentation ships with the repo
   (`terraform/adopt-v0.3/PREDICTIONS.md`).
4. **PROP-010 design session** (INT-0003) — now with the new input
   that directory registries are `--registry`-flag-only
   (`[[registry]].url` accepts only git-cloneable URLs).
5. **`VIBEVM-SPEC.md` unit-ification** (DBT-0019) — unchanged;
   unblocks vibe-cli item-grain backfill and retires the six ratchet
   dispositions.
6. **PROP-014 external-namespace amendment** — now has a working
   precedent: the `discipline://<package>/<doc>#<anchor>` citation
   namespace used by conform diagnostics (recorded in
   `spec/discipline/README.md`); v0.3 must decide units-vs-citations
   for packaged cards.
7. **Discipline v0.3 revision** — input:
   `terraform/adopt-v0.3/REPORT.md` §"What the adoption taught"
   (8 items). The discipline content now lives in
   `packages/org.vibevm/*` — the owner's tree by the same convention
   that governed `spec/neworder/`.
8. Debt candidate, not yet filed as DBT: `vibe.lock` records
   machine-absolute `file:///` source_urls for local-registry
   installs (committed noise on multi-machine teams).

## Non-obvious findings (this session)

- **Gate verdicts can be silently dropped by the caller** — three
  times: a gate behind `| tail -1` returns the pipe's exit code; a
  gate from a stale shell cwd never runs; a panel run before the
  final commit certifies the wrong tree. One clippy-red commit got
  pushed (`a65c706`, fixed by `fcdf636`). Standing practice now:
  `set -e` + redirect to a log + tail the log separately — capture
  the gate's OWN exit status. This is REPORT item #1 for v0.3.
- **The differential oracle out-thought its author twice.** The
  strict naive≡sat equality draft was falsified by proptest within
  seconds (generated worlds DO contain naive's first-pick-wins trap;
  sat solves them — that asymmetry became the dominance contract).
  Earlier, an input-side roots-uniqueness contract was killed by the
  existing suite (duplicate roots are legal input). Wrong beliefs
  cost a red test in the loop, not an incident.
- **nextest exits 4 on a zero-test crate** — `--no-tests=pass` is
  required in per-cell loops; a zero-test cell's build IS its first
  signal.
- **`#[cell]` needs `use specmark::cell;`** — a separate attribute
  macro from `spec`; the codemod's first live run caught this in its
  own template via the post-check and rolled back cleanly.
- **The conform facts cache is sound but its consumers weren't**:
  the Phase-0 "stale cache" suspicion resolved to panel-run ordering
  (see gate lesson); today's engine gives identical cached and clean
  results, with the store key `(content-hash, producer-id-version)`
  proven by the v2 frontend bump retiring all old slots.
- **Line-keyed fingerprints rot** — conform baseline entries are now
  `rule|file|context#ordinal` (shift-stable); the 33→35 stop.rs
  incident is the canonical example.
- **`vibe-workspace` doctests can be fully runnable** — tempfile is
  a REGULAR dep there (production staging uses it), so hermetic
  TempDir examples beat `no_run`.
- **Widened gates out-scope per-agent briefs**: the F-rule flagged
  `HookError` minutes after the sweep agents finished — it lives
  outside lib.rs, which their brief excluded. Centralized checkers
  beat per-agent scope judgment.

## Repository map (delta vs the v0.2-terraform era)

```
vibevm/
├── packages/org.vibevm/              ← in-repo local registry: the Discipline
│   ├── discipline-core/v0.2.0/       (manifesto, card format, scaffold catalog,
│   │                                  raid playbook, cards/, appendix/, legacy-projections/)
│   └── rust-ai-native/v0.2.0/        (rust/GUIDE-AI-NATIVE-RUST.md, rust/tools/vibe-tcg.md)
├── vibedeps/                         ← committed install slots (PROP-009); boot snippets live here
├── vibevm.discipline.lock            ← pins the piloted Discipline revision; ledger epoch input
├── spec/
│   ├── discipline/                   ← retained mechanisms: PROP-014, BROWNFIELD,
│   │                                  ENGINE-CONFORM, LEDGER-INTENT (+README with the
│   │                                  discipline:// citation convention)
│   ├── terraforms/TERRAFORM-PLAN-v0.3.md   ← the executed adoption plan
│   └── neworder/README.md            ← shim (where everything went + reinstall recipe)
├── terraform/adopt-v0.3/             ← LOG.md, PREDICTIONS.md (12 entries), REPORT.md
├── crates/vibe-resolver/
│   ├── src/sat.rs                    ← the backtracking Sat cell (DBT-0011 fixed)
│   ├── src/fixpoint_model.rs         ← the runnable fixpoint reference model (Class H)
│   ├── src/activation.rs             ← CapabilityTag newtype on the seam (Class B)
│   ├── src/conditional.rs            ← and/or/not composition (PROP-003 r2)
│   └── tests/{solver_properties,fixpoint_conformance,compile_fail}.rs
└── xtask/                            ← + fast-loop, + codemod add-cell; conform check
                                        runs six rules over seven gated crates
```

## Decisions in force (adoption legacy, beyond the four rules)

- Gates are the merge criterion, in run order: `cargo xtask specmap
  --check` → `cargo xtask conform check` → `cargo xtask test-gate` →
  (at structure changes) `cargo xtask fast-loop --enforce-budget` →
  `bash tools/self-check.sh`. **Invoke gates with their own exit
  status captured; never behind a pipe; re-run the panel on the
  final tree of a series.**
- The Discipline is consumed as installed packages; its files are
  not bent to fit vibevm (the owner's product constraint). Reinstall
  recipe: `vibe install flow:org.vibevm/discipline-core
  stack:org.vibevm/rust-ai-native --registry ./packages
  --assume-yes` (documented in the shim README).
- Conform rule authoring: messages speak `violates REQ <uri>: <why>;
  fix surface: <where>` (renderer/acceptor in
  `conform_core::rules`); fingerprints must be coordinate-free;
  baseline only shrinks (set-wise; line-shift corrections are
  legal).
- New cells arrive via `cargo xtask codemod add-cell` (atomic,
  `--spec-uri` required — A1 by construction); every `#[cell]` type
  must be referenced by an integration test (`cell-has-oracle`).
- Replacing a solver-class cell requires the dominance differential
  in `tests/solver_properties.rs` (naive-solvable ⇒ identical;
  sat-fail-where-naive-solves ⇒ bug).
- xfail-strict test semantics; the tests-baseline shrinks only via
  the promotion protocol. Tripwires are read, not muted; owner
  spec-drops are committed on sight. No CI by standing owner
  decision; every gate is a local command.
- `.ledger/`, `target/conform/`, `target/fast-loop/` are derived
  data, never committed. `specmap.json` is committed and
  regenerated with every change that moves units/tags.

## Recent commit chain (newest first)

```
1792c14 docs(wal): adoption COMPLETE checkpoint - v0.3 ran to its exit criteria
09d0da5 docs(terraform): the adoption REPORT - close-out and v0.3 input
8b27213 docs(spec): the priority-cell sweep - 25 seam doctests, REQ-edged errors
f977951 docs(terraform): Phase 7 close-out - DBT-0011 fixed, P7-1 held
63012b9 feat(resolver): boolean composition in context() predicates
eb66e13 feat(resolver): the Sat cell - backtracking lands on the naive checker
7466a0c feat(xtask): codemod add-cell - one checked atomic multi-file edit
84f92fb feat(resolver): the fixpoint simulator - a runnable reference model
fcdf636 fix(resolver): drop the unused SolveError import from the property suite
a65c706 test(resolver): property net + differential socket; cell-has-oracle rule
c76a359 chore(spec): index refresh after the import-placement fix
d4f2e34 fix(resolver): test-only CapabilityTag import moves into the test module
f9e5ac4 feat(resolver): CapabilityTag types the activation seam; contracts land
a059876 docs(spec): 30 canonical doctests close the seam-has-doctest gap
fe7eac7 feat(conform): Class F+G rules - REQ-citing diagnostics, seam doctests
ca1c5bf feat(xtask): fast-loop - the Class-E cell checker lands green
5fcdd36 docs(terraform): adopt-v0.3 working state - log, predictions, registry
583d2ee fix(conform): correct the frozen unsafe-gate line for stop.rs (33->35)
bf8bfd0 feat(spec): mechanisms re-anchor at spec/discipline; neworder shims
9608e0e feat(spec): the Discipline ships as packages; vibevm self-hosts
b7fcd36 docs(spec): terraform plan v0.3 - adopt Discipline v0.2
e3f06ec docs(spec): Discipline v0.2 BETA supersedes the v0.1 package
1eda79b docs(continue): session-end cold-resume checkpoint
61b4abf docs(wal): session-end checkpoint
e1da0c4 Merge branch 'new': the Discipline terraform — complete
```

## Quick-start

```sh
cargo xtask specmap --check          # index + orphan ratchet
cargo xtask conform check            # facts → 6 rules → SARIF → baseline
cargo xtask test-gate                # nextest, xfail-strict
cargo xtask fast-loop --enforce-budget   # per-cell isolation + budget (+doctests)
bash tools/self-check.sh             # fmt, tests, clippy -D warnings, vibe check
cargo xtask trace explain <symbol|uri> [--text|--json|--prose]
cargo xtask codemod add-cell --crate-dir <dir> --cell <name> \
  --seam <Trait> --variant <label> --spec-uri "spec://…"
cargo xtask tripwire                 # debt watches over the change set
```

Session-resume phrase: `восстанови сессию`. The WAL supersedes this
snapshot wherever they diverge.
