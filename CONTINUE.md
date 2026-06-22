# CONTINUE.md — cold-resume checkpoint

_Written 2026-06-22. This session landed **two owner-directed fixes** and is
**closed gate-green on `main`, pushed to both mirrors** (`origin`=gitverse
`anarchic/vibevm`, `github`=`anarchic-pro/vibevm`, both @ `2311639`). Working
tree clean. **No pending campaign work** — the open items are owner-level
deferrals (below), not a standing mandate._

> **`spec/WAL.md` is the canonical living state**; if this snapshot and the WAL
> disagree, the WAL wins. The **git log is the authoritative per-item record**.
> Boot first (`CLAUDE.md` → `spec/boot/INDEX.md` → its files → `spec/WAL.md`),
> then read this.

---

## TL;DR

Two pieces of work, both COMPLETE and on both mirrors:

1. **MCP-registration bug fixed.** `vibe mcp install` for Claude Code wrote its
   `mcpServers` block into `settings.json` — which Claude Code does **not** read
   for MCP discovery — so the install was a silent no-op. Now it writes
   `.mcp.json` (project) / the top-level `mcpServers` of `~/.claude.json` (user),
   wraps the launcher as `cmd /c vibe …` on Windows (a `.cmd` shim can't be
   spawned directly), and drops the non-portable `--path`. 7 commits
   (`072061b`→`96d43e9`).
2. **`vibe man` renamed to `vibe self`** (+ new `vibe self update`). `man`
   misread as the Unix manual page; `self` is the rustup idiom for a
   self-managing tool. Hard rename, no alias; internals `man`→`vvm`. 2 commits
   (`7cabb1a`, `2311639`).

## Where work stands

- **Branch `main`**, tip `2311639`; `main` ≡ gitverse ≡ github (mirror-synced).
- Working tree **clean**. **No auto-driven work remains.**
- **Active managed binary = instance #7** (rebuilt this session; speaks `self`).
  `vibe man` now errors "unrecognized subcommand".
- **Gate panel (green at close):** `self-check.sh` exit 0 (fmt, all tests +
  doctests, clippy `-D warnings`, `vibe check`); conform 0/0/0 (16 gated /
  4 exempt); specmap clean (545 units / 561 edges / 548 tagged / 0 suspects /
  0 orphans); test-gate green (1207, 0 failed, 3 skipped, xfail-strict);
  fast-loop 20/20.

## Active blocker & the human action that clears it

**None.** Tree clean, mirrors synced, all gates green, active binary rebuilt.

## What landed (this session)

**A — MCP registration (`fix(mcp)` spine = `f60bfbb`):**
- `agents.rs::config_path` for Claude Code → `<project>/.mcp.json` (project) and
  `~/.claude.json` (user). settings.json only *gates* `.mcp.json` servers
  (`enabledMcpjsonServers`); it never defines them.
- `build_mcp_entry` → OS-pure `build_mcp_entry_for(windows)`: on Windows the
  entry is `cmd /c vibe mcp serve` (the `vibe.cmd` shim can't be process-spawned
  by an MCP client) for **every** spawn-agent. Off Windows, plain `vibe`.
- `--path` dropped from the entry — CWD-resolved, so a committed `.mcp.json`
  stays portable across machines.
- `host_present` re-keyed off `~/.claude` (the old probe checked the parent of
  the user config; now that's `~`, which always exists → false positives).
- `build(deps)` `072061b`: `serde_json/preserve_order` so the merge appends
  rather than re-alphabetising the operator's `~/.claude.json`.
- `test(mcp)` `e62e3ce`, `docs(mcp)` `3f339d1`, `docs(research)` `96d43e9`
  (PROP-004 stale-path correction), 2 specmap regens.

**B — `vibe man` → `vibe self` (`feat(vvm)!` = `7cabb1a`):**
- Hard rename, **no alias**. CLI token via `#[command(name = "self")]` over a
  `Vvm` variant (`self` is a Rust keyword). Module + types `man`→`vvm` via
  `git mv` (history preserved). The surviving concept name is "VVM / VibeVM
  Version Manager".
- New verb **`vibe self update`** = thin shorthand over `self install latest`.
- PROP-019 §2.2 #surface bumped **r1→r2** (normative); the `man`→`self` token
  shifts in #remove/#selectors/#tools are `spec-editorial`.
- README + `tools/first-run.{sh,ps1}` moved with it. specmap regen `2311639`.

## Next steps (owner-directed — NOT a standing mandate)

No campaign is open. Candidate work, pick up only on explicit owner direction:

1. **vibe-cli pub-doctest (DBT-0021, still deferred).** vibe-cli is a bin crate
   with no lib target, so `cargo test --doc` can't compile its doctests; gating
   it would enforce uncompiled prose. Fix is structural (a `[lib]` target, or
   `pub`→`pub(crate)` tightening) — an owner call.
2. **`SkillStatus` newtype (deferred).** 9-value install/uninstall vocabulary
   shared across four serialized report types in two crates — a wire-contract +
   naming design call.
3. **MCP follow-ups now visible from this fix:** a project-level `.mcp.json`
   end-to-end smoke (today verified via `mcp status` + the live `~/.claude.json`
   connection, not an automated harness); confirm the `cmd /c` launcher on
   Claude Desktop / Cursor on a real Windows install (code is in place, only
   Claude Code was exercised live).

## Non-obvious findings (this session)

- **Claude Code MCP discovery ≠ settings.json.** Servers are *defined* in
  `.mcp.json` (project) and the top-level `mcpServers` of `~/.claude.json`
  (user) / `projects.<path>.mcpServers` (local). `settings.json` only carries
  `enabledMcpjsonServers` gating. Writing a server there is a silent no-op.
  Empirical proof on this machine: the working Blender server lives in
  `~/.claude.json`, and `claude mcp get vibevm` never saw the settings.json copy.
- **Windows `.cmd` shims can't be process-spawned by an MCP client.** `vibe` on
  this machine is `vibe.cmd`; the entry must be `cmd /c vibe …`. This is why
  `claude mcp add` writes `command:"cmd"` on Windows.
- **`serde_json` key order is global.** `preserve_order` is a workspace-wide
  feature unification, but the committed gate artefacts are immune: `specmap.json`
  serialises derived structs + pre-sorted Vecs; `vibe.lock` is TOML.
- **`self` is a Rust keyword** → the enum variant is `Vvm` with
  `#[command(name = "self")]`; you cannot name the identifier `self`/`Self`.
- **Machine quirks (unchanged):** edit via Edit/Write, never PS `Set-Content`
  (UTF-8 corruption); `git commit` via `-F - <<'MSG'`; `self-check.sh` through
  Git Bash; mirrors via `cargo xtask mirror` (ff-only), never `git push origin`.

## Repository map

```
vibevm/                      Rust workspace; binary = `vibe`; tooling = `cargo xtask`
├─ CLAUDE.md / AGENTS.md / GEMINI.md   identical; the 4 rules + boot pointer
├─ CONTINUE.md               this cold-resume snapshot
├─ specmap.json              traceability index (545 units / 561 edges)
├─ crates/
│   ├─ vibe-cli/src/
│   │   ├─ cli/vvm.rs         `vibe self` clap surface (was cli/man.rs)
│   │   ├─ commands/vvm/      THE VERSION-MANAGER MODULE (PROP-019; was commands/man/)
│   │   │   mod.rs = dispatch + run_update_cmd (NEW verb); install/store/placer/
│   │   │   source/builder/model/env/selfloc/error/remove/git/tools cells
│   │   └─ commands/mcp/      `vibe mcp install/status/upgrade/uninstall/serve`
│   └─ vibe-mcp/src/
│       agents.rs             per-agent config paths + build_mcp_entry_for (THE FIX)
│       agent_config.rs       order-preserving JSON/TOML merge
│       agentic.rs, pkgskill.rs, install.rs, tools.rs — pub-doctested
├─ spec/
│   ├─ common/PROP-019-version-manager.md   `vibe self` surface (#surface r2)
│   ├─ modules/vibe-mcp/PROP-015-mcp-integration.md  agent-config (#agent-config r2)
│   ├─ research/PROP-004-…    comparative research (MCP-path corrected)
│   └─ WAL.md                 canonical living state
├─ docs/commands/mcp-*.md     command reference (paths + Windows launcher fixed)
├─ tools/first-run.{sh,ps1}   bootstrap; now invoke `vibe self install`
└─ xtask/src/                 conform / specmap / test-gate / fast-loop / mirror / health
```

## Architectural / policy decisions in force

- **The four non-negotiable rules** (`CLAUDE.md`, PROP-000 §12): attribution
  (human-authored only), Conventional Commits, group-by-meaning, autonomy on
  routine changes only.
- **Agent MCP config = the file the agent reads for *discovery*, not a settings
  file it owns** (PROP-015 §2.5 r2). Claude Code: `.mcp.json` / `~/.claude.json`.
  The entry is scope-independent (no `--path`, CWD-resolved) and `cmd /c`-wrapped
  on Windows.
- **`vibe self` is the self-distribution command** (PROP-019 §2.2 r2) — rustup
  idiom; the module/concept stays "VVM". The install/switch unit is a whole
  immutable instance + a live `current` pointer (instant switch, no locks).
- **Source is multi-homed** (PROP-016): gitverse + github, both canonical; roll
  out with `cargo xtask mirror` (ff-only), never `git push origin`.
- **Newtypes earn their place by an invariant or a behavioral branch**, not by
  display alone.

## Recent commit chain (newest first)

```
2311639 chore(specmap): regen for the `vibe self` rename
7cabb1a feat(vvm)!: rename `vibe man` to `vibe self`, add `self update`
7906c52 chore(specmap): regen for the PROP-004 correction
96d43e9 docs(research): correct the stale MCP-config path in PROP-004
c246d57 chore(specmap): regen for the mcp registration fix
3f339d1 docs(mcp): correct the Claude Code path + launcher reference
e62e3ce test(mcp): cover corrected Claude Code paths + Windows launcher
f60bfbb fix(mcp): register Claude Code MCP where Claude Code reads it
072061b build(deps): keep JSON key order on agent-config merge
e343ad7 docs(continue): cold-resume — grammar-refactor RAID complete
896901c docs(wal): grammar-refactor RAID complete
f4dcd38 docs(terraform): close-out the grammar-refactor RAID
2cc6ab2 chore(specmap): regen for the vibe-mcp doctest line shifts
dbe593d build(conform): arm pub-doctest on vibe-mcp
273e58f docs(mcp): the server surface types teach by doctest
c5fff3a docs(mcp): the agentic relay types teach by doctest
5f74dc4 chore(specmap): regen for the P5 PROP-018 grammar
e71827f style(cli): rustfmt the agentic import block
4861cba refactor(mcp): share the dry-run status projection
7cf57c5 refactor(mcp): IntentStatus newtype for the relay mailbox state
4e249cc feat(mcp): affinity dispatcher + unified agentic transports
dee16dc test(mcp): cross-check served tools against the usage skill
cfdf213 refactor(cli): one resolve_project_root, not two
8c5c8fa chore(specmap): regen for the P3 Class-F error enums
7d59f4e style(cli,mcp): rustfmt the P3 error-enum edits
```

## Quick-start

```sh
# Tier-0 floor (run before any work — never work on a red tree)
bash tools/self-check.sh                 # via Git Bash, NOT WSL — check $?, not a tail pipe
cargo xtask conform check                # 0 new against the baseline (0/0/0)
cargo xtask specmap --check              # 0 suspects / warnings / gated orphans
cargo xtask test-gate                    # nextest, xfail-strict
cargo xtask fast-loop --enforce-budget   # every cell builds+tests < 60s

cargo xtask mirror --check               # verify both mirrors are in sync
cargo xtask mirror                       # fan main+tags to both mirrors (ff-only)

# Version manager (renamed this session):
vibe self ls            # list installed instances (* = active; #7 is current)
vibe self update        # rebuild + activate the latest in-tree version
vibe self current       # active instance id
```

Session-resume phrase: `восстанови сессию` — restores state and **reports, then
waits for direction**. With no campaign open, the candidate next steps are the
owner-directed deferrals above. The WAL supersedes this snapshot wherever they
diverge.
