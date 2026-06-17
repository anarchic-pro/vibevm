# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-17 at session save. Branch `main` @ `c6e65bf`, level with
both mirrors (`gitverse` = `anarchic/vibevm`, `github` = `anarchic-pro/vibevm`)
— this save's own WAL/CONTINUE commits roll out as the final step. Working
tree clean. Full gate panel green._

> **`spec/WAL.md` is the canonical living state and its header is current.**
> If this snapshot and the WAL disagree, the WAL wins. Boot first
> (`CLAUDE.md` → `spec/boot/INDEX.md` → its files → `spec/WAL.md`), then read
> this. The **git log is the authoritative per-item record** — every commit
> cites its reasoning.

---

## TL;DR

This session **rebuilt the VibeVM Version Manager (VVM, `vibe man`) to v2**
(PROP-019) and pushed it to both mirrors. VVM is how vibevm distributes
itself: it builds, installs, and switches vibevm's own versions on a machine.

v2 reworks v1 after the owner found two design flaws:

1. **Console-reload friction** — switching a version forced opening a new
   shell. Fixed: the active version is a live **`current` pointer file**; the
   shims read it, so `man install` / `man use` flip it and the *next* `vibe`
   in the same shell uses it — no reload.
2. **Self-replace locks** — reinstalling the running version locked the whole
   distribution (and would lock future DLLs). Fixed: the install/switch unit
   is a whole immutable **instance** (`versions/<kind>/<id>/<instance>/`);
   switching is a pointer flip and nothing in use is ever overwritten.

Plus: **diff-copy** placement (hardlink unchanged / copy changed, dedup-skip
on a byte-identical rebuild), **current_exe** as the truth (env demoted to
advisory + a stale-`$VIBEVM_HOME` warning), **`vibe vars`** (actual vs
environment), **git-incremental** managed clone + **linked rebuild** from a
remembered external source path, and **first-run scripts** + README.

Five gate-green commits (`34c8250` … `c6e65bf`), all on both mirrors.

## Where work stands

- **Branch `main` @ `c6e65bf`**, level with both mirrors after this save's
  rollout. Working tree clean.
- **No campaign in flight.** The PROP-019 v2 MVP is the last work; the next
  session picks the owner's next goal.
- **Gate panel — green.** `self-check.sh` exit 0 (fmt, all tests, doctests,
  clippy `-D warnings`, `vibe check`); `conform check` 0/0/0; `specmap
  --check` clean (545 units / 543 edges / 0 suspects / 0 orphans).

## Active blocker & the human action that clears it

**None.** Panel green, tree clean, mirrors synced. (One standing owner
decision, not a blocker: whether to start the PROP-019 §6 far-backlog or a
fresh goal.)

## EXACT next-steps recipe (candidate work — the owner chooses)

No plan is mid-execution. Candidates:

1. **PROP-019 §6 far backlog** (seams already cut):
   - **Binary-artifact install** (`man install --binary`) + the binary-only
     auto-prune-on-install. Today only source builds exist; the placer +
     instance model already handle multi-file distributions, the monotonic
     instance counter is in place, and the `Origin::Binary` variant exists.
   - **reflink / CoW placement** — the placer hardlinks unchanged files today;
     a copy-on-write clone (where the FS supports it) would let changed files
     share blocks too.
   - **Signature verification** of fetched sources / artifacts.
2. **End-to-end `man use` + shim-exec test** — not smoke-tested on Windows
   this session (it writes the real registry PATH). Either a non-Windows
   integration test or a Windows test with an isolated registry.
3. **A fresh owner goal** — boots from this checkpoint with no debt.

## Non-obvious findings (this session)

- **Windows `canonicalize()` returns a `\\?\` verbatim path.** Recording it
  as an external source path leaks the ugly prefix and some tools choke on
  it; `man install` strips `\\?\` before recording `source_path`
  (`source::external_path`). Verified: the recorded path is now clean.
- **diff-copy's dedup-skip is mtime-independent for small files.** The
  manifest hashes files ≤16 MiB, so an unchanged rebuild (cargo bumps the
  binary's mtime but the bytes are identical) still matches → no new instance.
  Large files fall back to (size, mtime), never bulk-hashed (the 2 GB /
  binary-dist concern).
- **A managed `vibe` parses its own layout backwards** to find its root:
  `…/<base>/opt/vibevm/versions/<kind>/<id>/<instance>/vibe[.exe]`
  (`selfloc::derive_self`), validating the `versions`/`vibevm`/`opt` segment
  names. A dev `cargo run` (binary under `target/`) derives nothing → falls
  back to env/default, so tests and dev runs are unaffected.
- **`man use` (full) and `man doctor --fix` mutate the real durable PATH** —
  the registry on Windows (HKCU), the shell rc on POSIX. Smokes this session
  used `man install` (writes only the `current` file + the instance, no env)
  and `man use … --eval` (prints, persists nothing) to avoid touching the
  real machine. The shim-reads-`current` loop is unit-tested (shim content +
  `current`), not smoked end-to-end on Windows.
- **`core.filemode=false` on this machine** → new `.sh` files commit as
  `100644` (matching `self-check.sh`). The repo convention is to run scripts
  via `bash tools/<name>.sh`, not rely on the exec bit; the README's
  first-run commands use `bash …` / `.\…ps1` accordingly.
- **Machine quirks (unchanged):** edit via Edit/Write, never PS `Set-Content`
  (UTF-8 round-trip corruption); `git commit` via `-F - <<'MSG'` heredoc;
  `self-check.sh` through **Git Bash**, never WSL; mirrors via `cargo xtask
  mirror` (ff-only), never `git push origin`.

## Repository map

```
vibevm/                      Rust workspace; binary = `vibe`; tooling = `cargo xtask`
├─ CLAUDE.md / AGENTS.md / GEMINI.md   identical; the 4 rules + boot pointer
├─ README.md                 now carries the "First run" (VVM bootstrap) section
├─ VIBEVM-SPEC.md            owner-frozen implementation spec
├─ CONTINUE.md               this cold-resume snapshot
├─ mirrors.toml              source-mirror target registry (gitverse + github)
├─ specmap.json              traceability index (545 units / 543 edges)
├─ crates/                   library/bin crates
│   └─ vibe-cli/src/commands/man/   ← THE VVM MODULE (see table below)
├─ tools/                    self-check.sh, first-run.sh, first-run.ps1, jtd-codegen
├─ spec/
│   ├─ common/PROP-019-version-manager.md   the VVM design (v2)
│   └─ WAL.md                canonical living state (rewritten each session)
└─ xtask/                    project tooling (incl. `cargo xtask mirror`)
```

**The VVM (PROP-019) lives in `crates/vibe-cli/src/commands/man/`:**

| File | Holds |
|---|---|
| `mod.rs` | dispatch + read verbs (`ls`/`current`/`which`) + `install`/`use`/`env`/`doctor`; `ManEnv`; selector→installed resolution |
| `model.rs` | `Kind`, `VersionId`, `Selector`, `Profile`, `Origin`, `InstallRecord` (instance + provenance), `State` (+ `next_instance` counter) |
| `store.rs` | the install-root layout: instance dirs, the `current` pointer, `state.toml`, `mirror_dir` |
| `selfloc.rs` | `derive_self` (current_exe → root/home) + `same_location` |
| `builder.rs` | the build seam — `CargoBuilder` into the managed `--target-dir` |
| `source.rs` | find/clone/resolve sources; `prepare_from_mirror` (git-fetch), `external_path` (strip `\\?\`), `linked_source` |
| `placer.rs` | diff-copy — `.vvm-manifest.toml`, hardlink-unchanged / copy-changed, dedup-skip |
| `install.rs` | `perform_install` orchestration (build → place → record → flip `current`) |
| `env.rs` | shims (read `current`) + `EnvPersister` (registry / rc) |
| `remove.rs` | `remove` (per-instance, safe) + `gc` (build cache / prune instances) |
| `git.rs` `tools.rs` | git wrappers; toolchain doctor checks |
| `vibe vars` | `crates/vibe-cli/src/commands/vars.rs` + `cli/vars.rs` |

## Architectural / policy decisions in force (long form)

- **The four non-negotiable rules** (`CLAUDE.md`, PROP-000 §12): attribution
  (human-authored only), Conventional Commits, group-by-meaning, autonomy on
  routine changes only.
- **PROP-019 VVM v2 (NEW, in force 2026-06-17).** The unit of install/switch
  is a whole immutable instance; the active version is the live `current`
  pointer file (not an env var); a managed `vibe` derives root/home from
  `current_exe` (`$VIBEVM_HOME` advisory). Distributions are placed by
  diff-copy (hardlink unchanged / copy changed; dedup-skip; never bulk-hash).
  Sources are referenced not copied: managed = shared `src/.mirror`
  (git-fetch), external = the committer's tree built in place + remembered
  path → linked rebuild. The instance key is a monotonic counter (content-hash
  rejected for the 2 GB / binary-dist future). Auto-prune-on-install is
  binary-only (§6).
- **Source is multi-homed (PROP-016).** gitverse `anarchic/vibevm` + github
  `anarchic-pro/vibevm`, both public + canonical for reading. Roll out with
  `cargo xtask mirror` (ff-only, never `--force`), NOT `git push origin`.
- **The package registry is a separate split-host** (PROP-000 §7) — github
  `vibespecs`, auth `~/.vibevm/github.publish.token`, used only by `vibe
  registry publish`. VVM never uses it (clones via SSH / public HTTPS). The
  token is a surface-secret, never echoed; `vibe vars` never includes it.
- **Two enforcement gates.** conform (a finding fails CI; baseline only
  shrinks) + specmap orphan ratchet. resolvo (PROP-017) is the default solver.
- **The Discipline's two laws:** idiomatic inside the file / engineered around
  it; explanation capital must be runnable capital.

## Recent commit chain (newest first)

```
c6e65bf docs(readme): document the VVM first run               (this session)
eecb46e chore(tools): add first-run bootstrap scripts          (this session)
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
7250af8 docs(continue): session-save cold-resume rewrite        (prior session)
cfb7e11 docs(wal): session save — PROP-018 MVP on both mirrors
ee9c62e docs(spec): add the General Discovery Prompt v3
```

## Quick-start

```sh
cargo xtask specmap --check              # traceability index + orphan ratchet
cargo xtask conform check                # facts → rules → SARIF → baseline (0/0/0)
bash tools/self-check.sh                 # via Git Bash, NOT WSL — check $?, not a tail pipe
cargo xtask mirror --check               # verify both source mirrors are in sync
cargo xtask mirror                       # fan main+tags to both mirrors (ff-only)

# VVM (PROP-019) — first run from a source checkout
bash tools/first-run.sh                  # build + install first version + shims/PATH
cargo run -p vibe-cli -- man install     # (or by hand) build this checkout → instance 1
cargo run -q -p vibe-cli -- man ls       # list instances; * = active
vibe man use <selector>                  # switch live (no reload); `man current` / `which`
vibe vars [diff|full|full diff]          # actual (current_exe) vs environment
```

Session-resume phrase: `восстанови сессию` — **restores state and reports,
then waits for the owner's direction** (the CLAUDE.md contract). The WAL
supersedes this snapshot wherever they diverge.
