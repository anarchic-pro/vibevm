# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-13 mid-execution of **CONVERT-PLAN v0.1** under the owner
goal «выполнить CONVERT-PLAN-v0.1 до конца, фаза 7 тоже разгейчена».
Phases 0, 1, 2 are **complete**; Phase 3 core depth (3.2, 3.4, 3.5) is
done; the large remaining phases need dedicated continuation. 30 commits
this run (`6f41359` … `4be55c6`), all on `origin/main`, panel green at
every commit._

> **`spec/WAL.md` is the canonical living state and its header is
> current.** If this snapshot and the WAL disagree, the WAL wins. Boot
> first (`CLAUDE.md` → `spec/boot/INDEX.md` → its files incl. the two
> Discipline snippets → `spec/WAL.md`), then read this. The **git log is
> the authoritative per-item record** — every commit cites its
> CONVERT-PLAN item.

---

## TL;DR

The owner set «execute CONVERT-PLAN-v0.1 to completion, Phase 7 also
un-gated». The plan (`spec/terraforms/CONVERT-PLAN-v0.1.md`) is a
~15–18-sitting full-depth conversion of strata B/C. This run executed
**Phases 0–2 completely and Phase 3's core depth (3.2, 3.4, 3.5)** to
high quality — the five-gate panel green on every commit, a full
`self-check` confirmed all-green. The remaining work — Phase 3's two
structural niceties (3.1, 3.3), and Phases 4, 5, 6, 7 — is genuinely
large (Phase 4 moves ~2.5k LOC; Phase 7 overhauls vibe-mcp), and is
mapped precisely below + in the WAL header.

## Where work stands

- **Branch `main` @ `4be55c6`**, in sync with `origin/main`, working tree
  clean. (Plus this checkpoint's commits.)
- **No blocker. The panel is green.** The next unit is the plan's next
  item, in order: Phase 3 tail (3.1 → 3.3) then Phase 4.
- **Gate panel** (each gate's own exit code, last verified): `specmap
  --check` — 442 units / 413 items / 424 edges / 0 suspects / 0 warnings;
  `conform check` — **57 frozen / 0 new** (file-length: 2 [the parked MCP
  pair]; pub-doctest: 55 [vibe-core's ratcheted type doc-debt, see
  below]); `self-check.sh` — all green.

## What this run delivered (Phases 0–2 complete + Phase 3 core)

- **Phase 0 — hygiene** (8 commits, `173bb15`…`616e9db`): PROP-014
  self-marks its six units (specmap warnings 6→0); stub-status headers;
  ServerLock → crate-level `src/lock.rs` seam; `vibe search` reads env at
  the composition root; short-name drops len-checked expects;
  conform-frontend-rust + env-audit gated → **CONFORM_GATED 12** with a
  checked `CONFORM_EXEMPT` reasons table (partition test).
- **Phase 1 — vibe-core armor** (`173bb15`…`00cf8c1`): all **7 newtypes**
  — RelPath, PackageName, CapabilityNamespace, CapabilityName,
  ContentHash, SourceUrl, TraceId — serde-transparent + `Deref<str>`
  (Deref is what collapses the read-site cascades; `from_validated`/`new`
  at trusted reconstruction seams, `parse` validates only untrusted
  input). 9 vibe-core seam doctests. **The `pub-doctest` Class-G gate**
  (`647ce68`) — froze vibe-core's 55 undocumented public *types*
  shrink-only (the plan predicted single-digit; FALSIFIED). Lockfile's
  explicit lockfile-schema edge.
- **Phase 2 — declare surfaces**: RepoCreator's 3 adapters became
  `#[cell]`s with a seam-driving oracle (cells 20→23) + the R-001
  construction moved to the registry module (`3bd4cfc`); token-redaction
  tests gained `#[verifies]` (`14ce2b0`); `Publisher::publish` got its
  error contract (`54446d0`); BootBand pins the effective-boot order
  (`8e65a1d`). **vibe-install (2.4) was already full-depth from v0.2** —
  §10 left it untouched.
- **Phase 3 — vibe-index core depth**: the in-RAM Index teaches its full
  read/write lifecycle by doctest via a new public
  `VersionEntry::minimal(kind, group, name, version)` fixture builder
  (`eb85cbb`, `bc540db`); the rate-limiter's refill is a pure runnable
  model `refilled_tokens(...)` with a doctest (`e6c0ac1`); the 21 ApiError
  RFC-7807 `detail` strings carry Class-F `(violates spec://…#http;
  fix:…)` tails (`4be55c6`).

## EXACT next-steps recipe (resume here, in plan order)

1. **Phase 3 tail** (both non-gate-forced — vibe-index is already
   gate-green; do for depth):
   - **3.1 server seams + fakes**: make `TokenStore` /
     `RateLimiter` (in `crates/vibe-index/src/server/`) trait seams with
     in-memory fakes, `AppState` holds the seam objects, handlers consume
     them, new unit tests drive handlers through the fakes; the existing
     `server_e2e` / `rate_limit_e2e` suites stay as behavioral oracles. No
     `#[cell]` (one production variant — §10); the seam + fake is the
     deliverable. *Substantial — a DI refactor of the server wiring; do
     with care.*
   - **3.3 split `types/entry.rs`** (499 lines, 15 structs) into a module
     family `types/entry/{mod,compat,provides,…}.rs`, each ≤150 lines,
     every child carrying the parent's `scope!`. *Mechanical but
     UNFORCED (entry.rs is under the 600 budget); modest value.*
2. **Phase 4 — vibe-cli facade diet (HUGE).** Move ~2.5k LOC of domain to
   gated lib crates, then flip vibe-cli into `CONFORM_GATED`:
   - 4.1 search domain (`commands/search_full_scan.rs`,
     `search_cache.rs`, the scoring half of `search.rs`) →
     `vibe-registry::search` module family, born gated (scope! + edges +
     Class-F errors + doctests). 4.2 vendor + redirect-sync →
     vibe-registry. 4.3 `apply_git_source_flag` manifest mutation →
     vibe-install typed request. 4.4 init.rs templates → data
     (`include_str!`). 4.5 Class-F on the CLI's remaining thiserror
     enums. 4.6 drain then flip `CONFORM_GATED += vibe-cli` → 13.
3. **Phase 5 — toolchain under its own law.**
   - 5.1 flip specmark + specmark-grammar into `CONFORM_GATED`. **GOTCHA
     (this run found it): NOT zero-drain.** The flip surfaces **12
     seam-has-doctest findings** — 8 on specmark-grammar's public grammar
     API (`Verb`, `SpecUri`, `EdgeSpec`, `SpecArgs`, `UriArgs`,
     `CellArgs`, `is_valid_anchor`, `parse_spec_uri`) and 4 on specmark's
     proc-macros (`spec`, `verifies`, `scope`, `cell`). Drain them with
     doctests before flipping (the *Args ones parse via `syn`, awkward;
     the proc-macros need doctests that *invoke* `#[specmark::…]` — valid
     in a proc-macro crate's doctests). Also remove both from the
     `CONFORM_EXEMPT` table when flipping (the partition test
     `every_crate_is_gated_or_exempt` enforces disjointness). The
     `scope!`-bootstrap exemption stays only in `specmap-ratchet.json`.
   - 5.2 the new `ambient-env` rule (R-001 projection): needs a frontend
     bump to emit `EnvRead` facts (env::var/var_os/set_var outside
     `#[cfg(test)]`); the rule fires on gated crates for reads outside the
     recorded roots (`vibe-cli/src/main.rs`, vibe-index reindex root) and
     outside env-audit; escape via fn-grain `#[spec(deviates)]`.
     Phase-0.4 already pre-cleaned the two known search-env reads.
   - 5.3 xtask stays exempt on the record.
4. **Phase 6 — spec layer truth pass.** *Largely satisfied already:* the
   foundation types are in the retrieval index via `scope!` inheritance;
   PROP-011 `#skip-resolution` is already pinned by freshness.rs's
   `scope!`, `#materialise-diff` by user_config.rs's. PROP-010 already
   reads DRAFT/requirements-record (6.4 is effectively done). REMAINING:
   6.1 PROP-000 kind audit (24 units — mark `req`/informative), 6.3
   process-doc `guide`/informative markers (PROP-006/013/LEDGER/
   BROWNFIELD). Low code-risk, tedious. Mind: adding an `implements` edge
   to an UNMARKED unit raises a pin-into-unmarked-unit specmap warning
   (Phase 0.3 lesson) — mark the unit `req r1` in the same change.
5. **Phase 7 — MCP endgame (OWNER UN-GATED — DBT-0020 lifted; HUGE).**
   7.1 spec home `spec/modules/vibe-mcp/PROP-0xx`. 7.2 vibe-mcp
   `tools.rs` (720 lines) → `McpTool` trait + 3 `#[cell]`s + Class-F +
   scope! + oracles. 7.3 drain the **2638-line**
   `vibe-cli/src/commands/mcp.rs` (agent detection, config writers, skill
   gen) into vibe-mcp behind typed surfaces, templates → data, CLI ≤600.
   7.4 close the ledgers: vibe-mcp exits `specmap-ratchet.json` exempt
   and enters `CONFORM_GATED`; **both `file-length` baseline entries
   drain; baseline → 0**; the 10 DBT-0020 dispositioned orphans resolve.

## Cadence (the discipline every batch follows)

Per-crate gated batch → topic commit(s) citing the CONVERT-PLAN item →
`cargo build` + crate tests + `cargo fmt --all` + `cargo xtask conform
check` (0 new) + `cargo xtask specmap --check` (regen on any tag/line
move) + a shrink-only freeze diff where the baseline is touched. Run
`bash tools/self-check.sh` (via Git Bash, NOT WSL) at phase boundaries —
**check `$?`, not a tail pipe** (a `| tail` masks the real exit code).
Any batch is a safe stopping point.

## Non-obvious findings (this run)

- **Deref<str> collapses newtype cascades.** PackageName's raw grep was
  ~50 sites; with `Deref` + `PartialEq<str>` the real edits were a
  handful of construction sites. Define ergonomic newtypes (Display,
  Deref, PartialEq against str, From) and the compiler-led cascade is
  small. The hard sites are reconstruction-from-validated-String in gated
  crates (no unwrap allowed) → `from_validated` (a documented infallible
  constructor) beats a fake-fallible signature that mints dead error
  paths.
- **The frontend already emits `is_pub` + `has_doctest`** on `Fact::Item`
  (since v2) — `pub-doctest` (1.4) needed no frontend bump. Model new
  conform rules on the existing ones in
  `crates/conform-core/src/rules/{structure,diagnostics}.rs`; wire into
  `xtask/src/conform.rs` `ConformRules` + its `refs()`; a Class-G rule
  gets its own gated list (`GATED_PUB_DOCTEST`).
- **A new conform rule's flip freezes its pre-existing findings ONCE**
  (legal growth); thereafter shrink-only. `conform freeze` is the legal
  moment. But a CRATE flip into `CONFORM_GATED` must NOT widen the
  baseline (drain first) — `pub-doctest`'s landing froze 55, but the
  conform-frontend-rust / env-audit flips drained to zero (0.7).
- **`seam-has-doctest` lens**: lib.rs items + traits anywhere. So a crate
  that re-exports its types from submodules (vibe-core) had its types
  unseen — `pub-doctest` widened the lens to all public types under
  `src/`.
- **Files at the 600 budget are landmines**: boot.rs is exactly 600 after
  2.3; an added line trips `file-length`. Use the tests-out idiom (the
  Phase-4-of-SHRINK recipe) to split, or keep additions tiny.
- Machine quirks (still true): Windows UAC blocks test exes named
  \*install\*; PowerShell 5.1 corrupts UTF-8-no-BOM round-trips (edit via
  tools, recover via `git restore`); `bash` in PowerShell = WSL, so
  `self-check.sh` runs through Git Bash; `git commit` via `-F - <<'MSG'`
  heredoc only (backtick `-m` double-corrupted messages before).

## Standing items

- **The 55-entry pub-doctest freeze is ratcheted debt** on vibe-core's
  public types (the plan's single-digit prediction was falsified — 55
  types beyond the 9 primary seams). The gate stops NEW undocumented
  types; draining the 55 is continuous ratchet work, not a phase blocker.
- **5.1 is NOT zero-drain** (12 seam-doctest findings — see Phase 5
  above); this run reverted the trial flip to keep the tree clean.
- Owner-court (carried, unchanged): the 2026-06-11 history-rewrite
  question (AUDIT 2026-06-12-01 rider); publishing the two Discipline
  packages; production solver selection; PROP-010 design session;
  Discipline v0.3 inputs.

## Recent commit chain (newest first; all this run, 2026-06-13)

```
4be55c6 docs(index): HTTP error details carry their spec tail        (3.5)
8ae3fc7 docs(wal): Phase 3 core depth done (3.2, 3.4); the rest mapped
bc540db feat(index): VersionEntry::minimal lets the index doctest …  (3.4)
e6c0ac1 feat(index): the rate-limiter's refill is a runnable model   (3.2)
eb85cbb docs(index): the in-RAM Index teaches its read path …        (3.4)
fed8737 docs(wal): Phases 1 and 2 complete, Phase 3 next
8e65a1d docs(workspace): BootBand pins the effective-boot order      (2.3)
54446d0 docs(publish): Publisher::publish carries its error contract (2.2)
14ce2b0 docs(publish): token-redaction tests verify the secrecy REQ  (2.1)
3bd4cfc feat(publish): RepoCreator's three adapters become cells     (2.1)
00cf8c1 docs(core): Lockfile carries an explicit lockfile-schema edge(1.5)
647ce68 feat(conform): pub-doctest rule gates vibe-core's type …     (1.4)
9fd2418 docs(wal): Phase 1.1 complete (all seven newtypes), 1.3 done
f99976e feat(core): SourceUrl and TraceId newtypes complete the 1.1 …(1.1)
4c46eae feat(core): ContentHash newtype for the identity digest      (1.1)
987c50b docs(wal): CONVERT-PLAN v0.1 in-progress checkpoint
17f7344 docs(core): doctests on the Manifest, Lockfile, UserConfig … (1.3)
0eac0cc docs(core): compiled doctests on the identity seams          (1.3)
0258639 feat(core): CapabilityNamespace and CapabilityName newtypes  (1.1)
7d4f041 feat(core): PackageName newtype for kebab-case package …     (1.1)
69ec16e feat(core): RelPath newtype for portable workspace …         (1.1)
616e9db chore(conform): exemptions carry recorded reasons           (0.1)
8e3ea13 chore(conform): frontend-rust and env-audit enter the gate  (0.7)
006db88 docs(conform): RustFrontend declares its seam doctest       (0.7)
555cceb refactor(cli): search reads its env at the composition root (0.4)
0392127 docs(spec): PROP-014 marks its own six units                (0.3)
413f8fd refactor(index): ServerLock becomes a crate-level lock seam  (0.6)
37e3f5c refactor(cli): short-name resolution drops len-checked …     (0.5)
173bb15 docs(graph,llm): stub crates declare their deferred scope    (0.2)
```

## Quick-start

```sh
cargo xtask specmap --check              # index + orphan ratchet
cargo xtask conform check                # facts → 9 rules → SARIF → baseline (57 frozen / 0 new)
cargo xtask conform freeze               # rewrite baseline (legal: new rule, or shrink, diff-reviewed)
cargo xtask test-gate                    # nextest, xfail-strict
cargo xtask fast-loop --enforce-budget   # cells < 60s
bash tools/self-check.sh                 # via Git Bash, NOT WSL — check $?, not a tail pipe
```

Session-resume phrase: `восстанови сессию` — **restores state and
reports, then waits for the owner's direction** (the CLAUDE.md contract).
The plan is `spec/terraforms/CONVERT-PLAN-v0.1.md`; the WAL supersedes
this snapshot wherever they diverge.
