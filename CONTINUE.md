# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-24. This session designed and **substantially built a new
feature — bridge packages** — landing **all five spec documents** and **six
gate-green implementation slices** (9 commits, `c768f90`→`48613e4`) on `main`.
Floor is **fully green** (`self-check.sh` exit 0). The commits are **local —
not yet pushed/mirrored**. Three deeper slices remain (planned, not built)._

> **`spec/WAL.md` is the canonical living state**; if this snapshot and the WAL
> disagree, the WAL wins. The **git log is the authoritative per-item record**.
> Boot first (`CLAUDE.md` → `spec/boot/INDEX.md` → its files → `spec/WAL.md`),
> then read this.

---

## TL;DR

A **bridge package** is a maintainer's wrapper around someone else's repo. The
design was decomposed (owner's call) into **four orthogonal mechanisms**, each
usable alone, each its own spec + tests:

- **PROP-020** install-hooks · **PROP-021** submodule-sources ·
  **PROP-022** materialization-modes · **PROP-023** bridge-packages ·
  **PROP-015 §2.8** `#skill-include` (additive).

**Done + green:** all specs; the manifest schema; submodule fetch; selective
skill projection; the install-hook *runner cell*; hardlink materialization.
**Left:** the `in-place` clone-path, hook pipeline-wiring + CLI consent, and
the destructive-guard + lockfile field — all detailed below and in the WAL.

## Where work stands

- **Branch `main`**, tip `48613e4`. **9 commits ahead of `origin/main`** —
  local only, **not yet mirrored** (`cargo xtask mirror` not run this session).
- Working tree **clean**. Floor **green**: `self-check.sh` exit 0 (fmt, all
  tests + doctests, clippy `-D warnings`, `vibe check`); specmap clean
  (597 units / 567 tagged / 580 edges / 0 suspects / 0 warnings / 0 orphans).
- The four new PROP docs + the PROP-015 revision are committed.

## What landed this session (gate-green)

1. `feat(core)` `fd4c118` — `vibe.toml` schema: `[package].materialization`
   (`Materialization` enum snapshot/hardlink/in-place), `[package].bridge`
   bool, `[hooks]` table (`HooksDecl`), `[[skill]].include` globs. All
   serde-default → existing manifests/lockfiles parse unchanged.
2. `feat(registry)` `869920f` — `--recurse-submodules` on clone +
   `git submodule update --init --recursive` on update; no-op without
   submodules (the path every existing package takes).
3. `feat(mcp,cli)` `84d8045` — `install_package_skill_selecting(include)` +
   an in-crate glob; empty include keeps the whole-tree default.
4. `feat(workspace)` `ff0aed8` — the **install-hook runner cell**
   (`vibe-workspace::hooks`): OS interpreter selection, allow-list+consent
   trust gate, pre→abort / post→flag failure semantics, two injectable seams,
   11 unit tests. **Engine only — not yet wired into the pipeline.**
5. `feat(workspace)` `e238251` — `materialise_with(CopyMode)` adds `hardlink`
   (copy-fallback); `apply_resolution` selects the mode from the manifest.
6. specs `c768f90`, `style` `ae7eebc`, three `chore(specmap)` regens, fmt-drift
   `48613e4`.

## What's left (next session) — with exact insertion points

1. **`in-place` clone-path (PROP-022 §2.4).** Today `copy_mode_for` in
   `crates/vibe-workspace/src/install.rs` maps `InPlace → Copy` (a documented
   fallback — correct, not optimised). The real path clones directly into the
   slot, bypassing the cache; it needs the **git backend + source URL** in the
   install layer, which `vibe-workspace` deliberately lacks. **Decision:**
   thread a clone-seam + URL into `apply_resolution`, *or* do the in-place
   clone in `vibe-install`/`vibe-registry` and skip `materialise` for in-place
   deps. Then: unversioned slot path (`vibedeps/<kind>-<name>/`), a
   `.gitignore` entry, `git clean -dfx` reset, `resolved_commit` identity.
2. **Hook pipeline-wiring + CLI consent (PROP-020 §2.1/§2.3).** The runner is
   ready (`vibe-workspace::hooks::run_package_hook`). Wire `PreInstall` into
   the materialise loop (`install.rs:103-118`, after each `materialise_with`)
   and `PostInstall` after the lockfile write (in `vibe-install`'s apply). The
   interactive `y/n` for `HookTrust::NeedsConsent` belongs in `vibe-cli`
   (resolve trust *before* `apply_resolution`; pass approved groups /
   `allow-hooks`). `DEFAULT_ALLOWED_GROUPS` (= `["org.vibevm"]`) is in
   `hooks.rs`. A pre-install non-zero exit must roll back the slot.
3. **Destructive guard + lockfile `materialization` field (PROP-022 §2.6).**
   Add `materialization` to `LockedPackage`
   (`crates/vibe-core/src/manifest/lockfile.rs`) so uninstall/guard know a slot
   is in-place — this touches the structural initialisers in
   `crates/vibe-install/src/record.rs` and
   `crates/vibe-cli/src/commands/update.rs` (and the `vibe-index` lockfile is a
   *separate* type, leave it). Gate destructive ops on an in-place slot behind
   a confirm / `--force`; hooks + their `git clean` reset are exempt.

Also future (specified, deliberately stubbed): the **dependency-declared
submodule** form (PROP-021 §2.2) for binary packages.

## Non-obvious findings (this session)

- **`in-place` is about file *count*, not bytes.** The owner's giant-repo case
  (Chromium = millions of 1 KB files) is killed by per-file syscalls — copy
  *and* hardlink both cost hours. So in-place must never walk the tree: git
  clones once into the slot and manages it in place; identity is the commit.
  `hardlink` is the orthogonal answer for *few big* files.
- **`vibe-workspace` is registry-decoupled by design** — it has no git/URL.
  That is the one real architectural obstacle to in-place (see slice 1).
- **The cache already is a live git clone** at
  `~/.vibe/registries/<hash>/packages/<group>.<name>/clone/`; materialise
  strips `.git` into `vibedeps/`. The VVM `placer` (PROP-019 §2.15) is the
  hardlink/diff-copy prior art; VVM linked-sources (§2.16) the in-place one.
- **`git ≥2.38` blocks submodules over `file://`** by default — the positive
  submodule test is left to the §5 acceptance smoke; the unit test covers the
  no-op (no-submodule) path.
- **Machine quirks (unchanged):** edit via Edit/Write, never PS `Set-Content`
  (UTF-8 corruption); `git commit` via `-F - <<'MSG'`; `self-check.sh` through
  Git Bash; mirrors via `cargo xtask mirror` (ff-only), never `git push origin`.

## Repository map (unchanged from prior; new files noted)

```
vibevm/                      Rust workspace; binary = `vibe`; tooling = cargo xtask
├─ spec/modules/vibe-workspace/  PROP-020 (hooks), PROP-022 (materialization) NEW
├─ spec/modules/vibe-registry/   PROP-021 (submodule), PROP-023 (bridge)      NEW
├─ spec/modules/vibe-mcp/PROP-015  + §2.8 #skill-include (revised)
├─ crates/
│   ├─ vibe-core/src/manifest/    package.rs (Materialization, bridge),
│   │     package/hooks.rs NEW (HooksDecl), package/skill.rs (include)
│   ├─ vibe-registry/src/git_backend/shell.rs  (recurse-submodules)
│   ├─ vibe-mcp/src/pkgskill.rs   (install_package_skill_selecting + glob)
│   └─ vibe-workspace/src/
│         hooks.rs NEW + hooks/tests.rs NEW   (the hook runner cell)
│         vibedeps.rs (CopyMode + materialise_with), install.rs (copy_mode_for)
└─ specmap.json              traceability index (597 units / 580 edges)
```

## Recent commit chain (newest first)

```
48613e4 chore(specmap): regen for the rustfmt content-hash drift
ae7eebc style: rustfmt the bridge-packages slices
2e3710a chore(specmap): regen for the hardlink materialization edge
e238251 feat(workspace): hardlink materialization mode
9217628 chore(specmap): regen for the install-hook runner edges
ff0aed8 feat(workspace): install-hook runner cell
180b16c chore(specmap): regen for the skill-include edges
84d8045 feat(mcp,cli): selective skill projection via include globs
07bd743 chore(specmap): regen for the submodule fetch edge
869920f feat(registry): fetch submodules on clone and update
49ec465 chore(specmap): regen for the bridge-packages schema
fd4c118 feat(core): manifest schema for the bridge-packages design
c768f90 docs(spec): bridge-packages design — PROP-020/021/022/023
5bdf35c docs(continue): cold-resume — mcp fix + `vibe self` rename  (prior)
4ac5dd9 docs(wal): session-end checkpoint — mcp fix + `vibe self` rename
```

## Quick-start

```sh
bash tools/self-check.sh                 # via Git Bash — check $?, currently green
cargo xtask specmap --check              # clean (597 units / 580 edges)
cargo test -p vibe-workspace             # the hook cell + materialization live here
cargo xtask mirror                       # fan main+tags to both mirrors (NOT run yet)
```

The 9 session commits are **local**; mirror them with `cargo xtask mirror`
when ready. Session-resume phrase: `восстанови сессию`. The WAL supersedes this
snapshot wherever they diverge; the "What's left" section is the candidate
next work, not a standing mandate.
