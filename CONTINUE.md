# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-17. Branch `main` @ `e02a0d3` — a **Discipline-Sweep grammar
refactor of the new features is IN FLIGHT** (owner goal: "class F grammar —
to the end"). P0–P2 have landed (eight gate-green commits on top of checkpoint
`38eef21`); P3 (Class-F error enums — the headline), P5 (PROP-018 dispatcher +
transport-unify), P4 (doctest drain + vibe-mcp/vibe-cli gate flips), and P6
(REPORT) remain. **The WAL's "Active campaign" section is the authoritative
status** — every landed commit, the remaining phases, the CommitHash-declined
rationale, and the open docs-anomaly finding live there. This checkpoint is on
both mirrors; working tree clean._

> **`spec/WAL.md` is the canonical living state and its header is current.**
> If this snapshot and the WAL disagree, the WAL wins. Boot first
> (`CLAUDE.md` → `spec/boot/INDEX.md` → its files → `spec/WAL.md`), then read
> this. The **git log is the authoritative per-item record** — every commit
> cites its reasoning.

---

## TL;DR

The VibeVM Version Manager (VVM, `vibe man`) v2 (PROP-019) is shipped and on
both mirrors; this refresh closes a 2-commit doc lag and records **two
real-machine shim fixes** that landed after the last checkpoint. The **active
campaign is now a deep, grammar-level refactoring of the new features** (VVM
v2 / PROP-019 and PROP-018) run as a Discipline-Sweep RAID — scope+freeze,
per-layer phases, a green floor between each.

VVM is how vibevm distributes itself: the `vibe` binary builds, installs, and
switches vibevm's own versions on a machine. v2 reworked v1 after two design
flaws: console-reload friction (fixed by a live **`current` pointer file** the
shims read — switch is instant, no reload) and self-replace locks (fixed by
making the install/switch unit a whole immutable **instance**
`versions/<kind>/<id>/<instance>/` — a pointer flip overwrites nothing).
Plus diff-copy placement, `current_exe` as the truth, `vibe vars`,
git-incremental managed clone + linked rebuild, and first-run scripts.

**The two follow-on fixes** (`b22edd9`, `7550cde`) came from driving the first
install *through the shim* on real Windows — exactly the path the prior
checkpoint flagged as un-smoked:

1. **`b22edd9`** — the env persister *appended* the shim dir to PATH, so a
   stale `~/.cargo/bin/vibe` shadowed the managed shim. `ensure_on_path` now
   *prepends* (rustup/nvm-style) via the pure, unit-tested `path_with_prefix`.
2. **`7550cde`** — `derive_self` fed the `current` pointer a `\\?\` verbatim
   path from `canonicalize()` that the cmd shim cannot exec. `strip_verbatim`
   now reduces it to drive-letter form.

## Where work stands

- **Branch `main` @ `7550cde`**, level with both mirrors after this refresh's
  rollout. Working tree clean.
- **Active campaign:** deep grammar-level refactoring of the new features
  under [`DISCIPLINE-SWEEP-v0.1`](spec/terraforms/DISCIPLINE-SWEEP-v0.1.md).
  Analysis stage; the scoped RAID plan is owner-reviewed before any heavy
  refactor (the Discipline requires scope+freeze first).
- **Tier-0 floor — green at `7550cde`.** `self-check.sh` exit 0 (fmt, all
  tests, doctests, clippy `-D warnings`, `vibe check` 0/0/0); `conform check`
  0/0/0 (0 frozen, 0 new; 16 gated / 4 exempt); `specmap --check` clean
  (545 units / 545 edges / 532 tagged items / 0 suspects / 0 orphans);
  `test-gate` green (1201 results, 0 failed, 3 skipped, xfail-strict);
  `fast-loop --enforce-budget` 20/20 cells in the 60s budget, 0 red.

## Active blocker & the human action that clears it

**None.** Floor green, tree clean, mirrors synced. The one open owner decision
is approving the scoped RAID refactoring plan once it is presented (it gates
the heavy refactor; mechanical Tier-1 wins can proceed under routine autonomy).

## EXACT next-steps recipe (the refactoring campaign)

The campaign is the standing Discipline Sweep aimed at the new features, run
as a RAID. Resume order:

1. **Gather facts.** `cargo xtask health` → read `terraform/health/latest.json`
   (file-length danger band, pub-doctest coverage gaps + drain/promotion
   backlog, deviation debt, the unwrap / ambient-env / unsafe censuses,
   error-enum REQ-edge coverage). The collector is no-LLM and advisory; the
   gates are the floor.
2. **Grammar-level (Tier-3) read of the new code** — `crates/vibe-cli/src/commands/man/*`
   + `vars.rs`, and PROP-018's `crates/vibe-mcp/src/{agentic,pkgskill}.rs` —
   against: newtypes at seams (3a), cell oracle/isolation/no-stamping (3b),
   one-idiom-per-operation (3c), contract-first ordering (3d), lying prose
   (3e), closed-vocabulary naming (3f).
3. **Mechanical landmark already known:** `crates/vibe-cli/src/commands/man/mod.rs`
   is **583 lines** — in the danger band `[540, 600]`, four lines from the
   gate. Natural seams to split (each new module carries the parent
   `scope!("spec://vibevm/common/PROP-019#surface")` so it stays indexed —
   no gated orphan): read verbs (`run_ls`/`run_current`/`run_which`) →
   `read.rs`; selector→installed resolution (`resolve_installed` + `latest_of`
   + `highest_tag_record` + `by_precedence_record`) → `resolve.rs`; the
   `doctor` handler (`run_doctor_cmd` + `confirm` + `path_has_dir`) →
   `doctor.rs`. `env.rs` (486) is the next-largest.
4. **Per phase:** edit via Edit/Write only → `bash tools/self-check.sh` (Git
   Bash) → `cargo xtask conform check` / `specmap --check` (regen + commit if
   line numbers shifted) → topic commit citing
   `spec://vibevm/terraforms/DISCIPLINE-SWEEP-v0.1#<tier>` → refresh
   `terraform/health/latest.json` in the same run.

Subordinate (PROP-019 §6 far backlog, seams already cut): binary-artifact
install (`man install --binary`) + binary-only auto-prune; reflink/CoW
placement; signature verification; an automated isolated-registry end-to-end
`man use` test.

## Non-obvious findings (carried + new)

- **NEW — Windows PATH precedence (`b22edd9`).** A version manager's shim dir
  must be *prepended*, not appended: a stale `cargo install`ed `vibe` on PATH
  otherwise wins. `ensure_on_path` now dedupes + moves the shim dir to the
  front via `path_with_prefix`; POSIX rc already prepended (`$dir:$PATH`).
- **NEW — `\\?\` verbatim prefix (`7550cde`).** `current_exe().canonicalize()`
  on Windows yields a `\\?\C:\…` path the cmd shim cannot exec; it had flowed
  into the `current` pointer and every later `vibe` died with "system cannot
  find the path specified". `derive_self` now strips it (drive-letter form);
  the derive test asserts a non-verbatim result.
- **Windows `canonicalize()` also leaks `\\?\` into recorded source paths** —
  `man install` strips it before recording `source_path`
  (`source::external_path`).
- **diff-copy's dedup-skip is mtime-independent for small files** (manifest
  hashes files ≤16 MiB), so a byte-identical rebuild makes no new instance;
  large files fall back to (size, mtime), never bulk-hashed.
- **A managed `vibe` parses its own layout backwards** to find its root
  (`selfloc::derive_self`), validating the `versions`/`vibevm`/`opt` segment
  names. A dev `cargo run` (binary under `target/`) derives nothing → falls
  back to env/default, so tests and dev runs are unaffected.
- **`man use` (full) and `man doctor --fix` mutate the real durable PATH** —
  HKCU on Windows, the shell rc on POSIX. Smokes avoid them via `man install`
  (writes only `current` + the instance) and `man use … --eval` (prints,
  persists nothing).
- **Machine quirks (unchanged):** edit via Edit/Write, never PS `Set-Content`
  (UTF-8 round-trip corruption); `git commit` via `-F - <<'MSG'` heredoc;
  `self-check.sh` through **Git Bash**, never WSL; mirrors via `cargo xtask
  mirror` (ff-only), never `git push origin`; `core.filemode=false` (new
  `.sh` files commit `100644`; run scripts via `bash tools/<name>.sh`).

## Repository map

```
vibevm/                      Rust workspace; binary = `vibe`; tooling = `cargo xtask`
├─ CLAUDE.md / AGENTS.md / GEMINI.md   identical; the 4 rules + boot pointer
├─ README.md                 carries the "First run" (VVM bootstrap) section
├─ VIBEVM-SPEC.md            owner-frozen implementation spec
├─ CONTINUE.md               this cold-resume snapshot
├─ mirrors.toml              source-mirror target registry (gitverse + github)
├─ specmap.json              traceability index (545 units / 545 edges)
├─ crates/                   library/bin crates
│   ├─ vibe-cli/src/commands/man/   ← THE VVM MODULE (see table below)
│   └─ vibe-mcp/src/{agentic,pkgskill}.rs   ← PROP-018 relay + skill projection
├─ terraform/                health/ (collector snapshots), registry/, golden/
├─ tools/                    self-check.sh, first-run.{sh,ps1}, jtd-codegen
├─ spec/
│   ├─ common/PROP-019-version-manager.md   the VVM design (v2)
│   ├─ terraforms/DISCIPLINE-SWEEP-v0.1.md   the standing recurring sweep
│   └─ WAL.md                canonical living state (rewritten each session)
└─ xtask/                    project tooling (mirror, health, conform, specmap, …)
```

**The VVM (PROP-019) lives in `crates/vibe-cli/src/commands/man/`:**

| File | Lines | Holds |
|---|--:|---|
| `mod.rs` | 583 | dispatch + read verbs (`ls`/`current`/`which`) + `install`/`use`/`env`/`doctor`; `ManEnv`; selector→installed resolution **(danger band)** |
| `env.rs` | 486 | shims (read `current`) + `EnvPersister` (registry / rc); `ensure_on_path` / `path_with_prefix` |
| `model.rs` | 307 | `Kind`, `VersionId`, `Selector`, `Profile`, `Origin`, `InstallRecord`, `State` (+ `next_instance`) |
| `remove.rs` | 308 | `remove` (per-instance, safe) + `gc` (build cache / prune) |
| `source.rs` | 304 | find/clone/resolve sources; `prepare_from_mirror`, `external_path` (strip `\\?\`), `linked_source` |
| `store.rs` | 277 | install-root layout: instance dirs, the `current` pointer, `state.toml` |
| `install.rs` | 246 | `perform_install` orchestration (build → place → record → flip `current`) |
| `placer.rs` | 220 | diff-copy — `.vvm-manifest.toml`, hardlink-unchanged / copy-changed, dedup-skip |
| `tools.rs` | 171 | toolchain doctor checks |
| `selfloc.rs` | 117 | `derive_self` (current_exe → root/home, strips `\\?\`) + `same_location` |
| `builder.rs` | 89 | the build seam — `CargoBuilder` into the managed `--target-dir` |
| `git.rs` | 85 | git wrappers |
| `vibe vars` | — | `crates/vibe-cli/src/commands/vars.rs` (126) + `cli/vars.rs` (10) |

## Architectural / policy decisions in force (long form)

- **The four non-negotiable rules** (`CLAUDE.md`, PROP-000 §12): attribution
  (human-authored only), Conventional Commits, group-by-meaning, autonomy on
  routine changes only.
- **PROP-019 VVM v2 (in force 2026-06-17).** The unit of install/switch is a
  whole immutable instance; the active version is the live `current` pointer
  file (not an env var); a managed `vibe` derives root/home from `current_exe`
  (`$VIBEVM_HOME` advisory). Distributions placed by diff-copy (hardlink
  unchanged / copy changed; dedup-skip; never bulk-hash). Sources referenced
  not copied: managed = shared `src/.mirror` (git-fetch), external = the
  committer's tree built in place + remembered path → linked rebuild. Instance
  key is a monotonic counter. The shim dir is prepended to PATH (`b22edd9`);
  derived paths are plain, never `\\?\` verbatim (`7550cde`).
- **Source is multi-homed (PROP-016).** gitverse `anarchic/vibevm` + github
  `anarchic-pro/vibevm`, both public + canonical. Roll out with `cargo xtask
  mirror` (ff-only, never `--force`), NOT `git push origin`.
- **The package registry is a separate split-host** (PROP-000 §7) — github
  `vibespecs`, auth `~/.vibevm/github.publish.token`, used only by `vibe
  registry publish`. VVM never uses it. The token is surface-secret, never
  echoed; `vibe vars` never includes it.
- **Two enforcement gates** — conform (a finding fails CI; baseline only
  shrinks) + specmap orphan ratchet. resolvo (PROP-017) is the default solver.
- **The Discipline Sweep** ([`DISCIPLINE-SWEEP-v0.1`](spec/terraforms/DISCIPLINE-SWEEP-v0.1.md))
  is the standing recurring guardian above the gates: collector-first
  (`cargo xtask health`), the gates are the floor, the collector is a guide.
- **The Discipline's two laws:** idiomatic inside the file / engineered around
  it; explanation capital must be runnable capital.

## Recent commit chain (newest first)

```
7550cde fix(cli): strip the Windows \?\ verbatim prefix from derive_self   (latest)
b22edd9 fix(cli): prepend the VVM shim dir on PATH so it wins
567efce docs(continue): cold-resume checkpoint — VVM v2
705251c docs(wal): session save — VVM v2 current phase
c6e65bf docs(readme): document the VVM first run
eecb46e chore(tools): add first-run bootstrap scripts
f106683 feat(cli): VVM v2 — git-incremental clone + linked rebuild
8910f8e feat(cli): vibe vars — reconcile actual vs environment
f70a922 feat(cli): VVM v2 — current_exe truth + stale-env warning
34c8250 feat(cli): VVM v2 core — instances, live current, diff-copy
d6b1039 docs(spec): PROP-019 v2 — instances, live current, diff-copy
6c7d6ae feat(cli): vibe man install — clone path and full selector resolution
21d6930 feat(cli): vibe man remove + gc — safe removal and disk reclaim
73f0f83 feat(cli): vibe man doctor — verify toolchain and environment
a458340 feat(cli): vibe man use — activation via shim + VIBEVM_HOME
67428bc feat(cli): vibe man install — in-tree build pipeline
ef22a2a feat(cli): vibe man — VVM scaffold + read-only verbs
d605fac docs(spec): PROP-019 — VibeVM Version Manager (VVM)
7250af8 docs(continue): session-save cold-resume rewrite
cfb7e11 docs(wal): session save — PROP-018 MVP on both mirrors
ee9c62e docs(spec): add the General Discovery Prompt v3
bd26156 docs(continue): PROP-018 MVP banner
7d5aaaa docs(wal): PROP-018 agentic + standalone modes — MVP checkpoint
050b150 docs(agentic): reframe — vibevm authors the instruction, agent executes
911409e fix(cli): rename relay drain variant for clippy enum-variant-names
```

## Quick-start

```sh
# Tier-0 floor (run before any sweep work — never sweep on a red tree)
bash tools/self-check.sh                 # via Git Bash, NOT WSL — check $?, not a tail pipe
cargo xtask conform check                # 0 new findings against the baseline (0/0/0)
cargo xtask specmap --check              # 0 suspects / warnings / gated orphans
cargo xtask test-gate                    # nextest, xfail-strict
cargo xtask fast-loop --enforce-budget   # every cell builds+tests < 60s

# Discipline Sweep (the campaign)
cargo xtask health                       # advisory facts → terraform/health/latest.json
cargo xtask tripwire                     # which debt.json entries the change set touches

# Mirrors
cargo xtask mirror --check               # verify both source mirrors are in sync
cargo xtask mirror                       # fan main+tags to both mirrors (ff-only)

# VVM (PROP-019)
cargo run -p vibe-cli -- man install     # build this checkout → an instance
cargo run -q -p vibe-cli -- man ls       # list instances; * = active
vibe man use <selector>                  # switch live (no reload)
vibe vars [diff|full|full diff]          # actual (current_exe) vs environment
```

Session-resume phrase: `восстанови сессию` — **restores state and reports,
then waits for the owner's direction** (the CLAUDE.md contract). The WAL
supersedes this snapshot wherever they diverge.
