# CONTINUE — cold-resume checkpoint

_Written: 2026-05-07. Owner-readable, self-contained. Pick this up with zero prior context._

---

## TL;DR (executive summary)

**M1.7 slice 4 landed end-to-end — vibevm now ships a five-agent MCP integration with skill, agent-context attribution, and an interactive install UX.** Six commits this session, all pushed to `origin/main`. Working tree clean.

What changed:

1. **Five agents in `vibe mcp install`** (was two): Claude Code, Cursor, **Claude Desktop** (user-level GUI), **OpenCode** (TUI), **Codex** (CLI). Per-agent profile drives detection / config-path / wire-format / payload shape. JSON merger generalised + new TOML merger for Codex's `~/.codex/config.toml`. OpenCode uniquely uses `mcp.<name>.{type, command:[…], enabled}` — separate command-array shape; everyone else uses `mcpServers.<name>.{command, args}`.
2. **Global `--invoked-by <agent>` flag + `VIBE_INVOKED_BY` env** stamps every JSON envelope with the calling agent's identity. Resolution `flag > env > unset`. `vibe show config` exposes the resolved value with provenance (`cli-flag` / `env` / `default`).
3. **`vibevm` SKILL.md template** (`crates/vibe-cli/src/commands/skill_template.md`, vendored via `include_str!`). Hard-binding instructions: bootstrap protocol, "use MCP, don't guess", required `--invoked-by`, required `vibe <subcmd> --help` consultation, four non-negotiable rules. Lands at `<scope>/<agent-skills-dir>/vibevm/SKILL.md` for the three agents that load FS skills (Claude Code, OpenCode, Codex). Cursor / Claude Desktop reported as `skipped`.
4. **New `vibe mcp install` UX** — `--auto` (no-prompt, all detected agents, skill on), `--with-skill` / `--without-skill` (mutually exclusive), `--skill-scope project|user`, `--agent <FILTER>` now optional. Without flags drops into `dialoguer::MultiSelect` (TTY required); non-TTY without flags refused with a hint pointing at `--auto` / `--agent`.
5. **Documentation surface**: three new reference files (`docs/commands/mcp-install.md`, `mcp-status.md`, `mcp-serve.md`) + new directory `docs/guides/` with the first guide `agent-mcp-quickstart-opencode.md` — dual-purpose tutorial + integration-test acceptance gate. ROADMAP §M1.7 + §M1.11 marked closed; `spec/WAL.md` checkpoint at the top documents the slice in newest-first order.

Workspace state at HEAD (`3bf2462`):

- **vibe-cli at 158 hermetic + 3 ignored** (+27 since slice 3's 131).
- `cargo test --workspace` all green across all crates.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `vibe check --path . --quiet` reports `0 errors, 0 warnings, 0 info` (self-host).
- Working tree clean. Only `.claude/settings.local.json` untracked (per-machine harness state, not committed).

Push to `git@gitverse.ru:anarchic/vibevm.git` is current. `origin/main = 3bf2462`.

---

## Where we are right now

- **Branch:** `main`. Working tree clean.
- **Latest commit:** `3bf2462 docs(guides): opencode + vibevm hello-world quickstart + acceptance gate`.
- **Ahead / behind origin/main:** `0 / 0`.
- **Last six commits this session (newest first):**

  ```
  3bf2462 docs(guides): opencode + vibevm hello-world quickstart + acceptance gate
  7cb1f33 docs(commands,roadmap,wal): M1.7 slice 4 — multi-agent + skill + invoked-by
  71229eb feat(vibe-cli/mcp): interactive install + --auto + --with/without-skill
  d384a96 feat(vibe-cli/mcp): vibevm SKILL.md template + per-agent writer
  2eaf544 feat(vibe-cli): --invoked-by global flag + VIBE_INVOKED_BY env
  05ce2e4 feat(vibe-cli/mcp): claude-desktop, opencode, codex + JSON/TOML mergers
  ```

- **Active blocker:** none. The slice landed clean; tests + clippy + self-host check all green.

---

## What to do first in the next session

Pick whichever of these matches the owner's interest:

### Option 1 — walk the new guide on a clean sandbox

Run `docs/guides/agent-mcp-quickstart-opencode.md` top to bottom on a fresh machine (or a fresh `C:\Users\olegc\hello-vibe\` sandbox). Tick every box in the "Acceptance checklist" section. The walk doubles as a release-readiness gate for slice 4 — if any checkbox fails, slice 4 has regressed.

This is the lowest-risk first step in any new session: you confirm the build is healthy AND that the documented contract still holds.

### Option 2 — extend the agent matrix to Gemini / Copilot

The `Agent` enum in `crates/vibe-cli/src/commands/mcp.rs:60` has the per-agent profile slot ready. Add new variants the same way slice 4 added Claude Desktop / OpenCode / Codex:

1. Add the variant to the enum + `Agent::ALL`.
2. Fill in `as_str` / `parse_filter` / `presence_markers` / `config_format` / `config_location` / `config_path` / `mcp_section_key` / `build_mcp_entry` / `supports_skill` / `skill_path`.
3. Mirror the unit-test pattern (look at `opencode_entry_uses_command_array_with_type_local`, `codex_entry_returns_toml_table_with_command_and_args`, `merge_toml_creates_mcp_servers_table_for_codex` — each new agent should add at least one test that pins its wire-shape uniquely).
4. Update `docs/commands/mcp-install.md` agent matrix table.
5. Add a sibling guide `docs/guides/agent-mcp-quickstart-gemini.md` (or `-copilot.md`).

ROADMAP §M1.11 and §M1.7's "out of slice 4" list call this out as the natural follow-up.

### Option 3 — `query_capabilities` / `list_subskills` MCP tools

Currently `vibe-mcp` exposes three tools (`query_package`, `read_subskill`, `materialise_subskill`). PROP-004 §5.1 also calls out `query_capabilities` and `list_subskills`. Wiring is straightforward:

1. Pattern after existing tools at `crates/vibe-mcp/src/tools.rs`.
2. `list_subskills` walks `lockfile.subskills_active` per package, returns the list with delivery + describes.
3. `query_capabilities` matches a search string against the lockfile's union of capabilities (per package).
4. Add tests the same way `slice 1 vibe-mcp tests` did.
5. Update SKILL.md template (`crates/vibe-cli/src/commands/skill_template.md`) to mention the new tools by name — the agent reads this contract verbatim.

### Option 4 — comment-preserving Codex TOML edits

Currently `merge_toml` (in `crates/vibe-cli/src/commands/mcp.rs`) uses `toml = "0.9"` round-trip via `toml::Value`. This loses comments in handcrafted `~/.codex/config.toml`. If a Codex operator complains, swap to `toml_edit` for `merge_toml` only (the JSON path is fine via `serde_json` which has no comment concept). Add `toml_edit` to workspace deps; preserve the `merge_json` path unchanged.

### Option 5 — manual smoke walk

`manual-tests/M1.7-mcp-claude-code-smoke.md` was envisioned in M1.7 ROADMAP but never written. Now that slice 4 closed, the smoke could become "M1.7-mcp-multi-agent-smoke.md" walking install + skill + a real agent round-trip — but the new `docs/guides/agent-mcp-quickstart-opencode.md` already does most of this in a more useful form. Decide whether the manual-test file adds value beyond the guide.

---

## Non-obvious findings from this session

These cost time / hit edge cases — write them down so a future session does not re-derive.

### `console::user_attended_stdin` does not exist

The `console` crate (v0.16) exposes `user_attended` (stdout TTY) and `user_attended_stderr` but **not** `user_attended_stdin`. The interactive-mode TTY gate in `vibe mcp install` uses `std::io::IsTerminal::is_terminal()` on `stdin()` instead — pulled into `crates/vibe-cli/src/commands/mcp.rs` as the local helper `stdin_is_tty()`. If you ever need to gate a different command on stdin-TTY, use this helper, not console.

### OpenCode `command` field is a single array, not split

OpenCode's MCP-server entry uses `command: ["vibe", "mcp", "serve", "--path", "<project>"]` — one array including the binary AND the args. Every other agent (Claude Code, Claude Desktop, Cursor, Codex) uses split `command: "vibe"` + `args: [...]`. The `Agent::build_mcp_entry` match arm for `Agent::OpenCode` constructs the array shape; the `decide_action` / `merge_json` paths are agnostic.

OpenCode also requires `type: "local"` (discriminator) and `enabled: true` (defaults to false if omitted, which silently disables the server). Both are mandatory.

### OpenCode `AGENTS.md` marker is intentionally a false-positive

`Agent::OpenCode.presence_markers` includes `AGENTS.md`. Every vibevm project ships `AGENTS.md` (the cross-agent rule-file copy of `CLAUDE.md`), so **every** vibevm project is detected as having OpenCode regardless of whether OpenCode is actually used. This was an explicit owner decision when shaping slice 4 ("маркеры OpenCode живут в .opencode/ + opencode.json/.jsonc, в AGENTS.md"). It means `--auto` will provision opencode.json by default; that file is harmless if OpenCode is not installed.

### Codex / Claude Desktop are user-level only

These two agents have no project-tree presence markers. Their `presence_markers()` returns `&[]`. Detection probes the existence of their user-level config dir (`~/.codex/` for Codex, `<config-dir>/Claude/` for Desktop — `<config-dir>` resolves through `dirs::config_dir()`: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, `~/.config` on Linux).

This means **`vibe mcp install --auto` will mutate user-level configs outside the project tree** when those dirs exist. Documented in `docs/commands/mcp-install.md` "User-level `--auto` writes" edge case. `--dry-run` is the safe preview path.

### `vibe show config` envelope has TWO `invoked_by` fields

The top-level `invoked_by: "<agent>"` is stamped by `output::Context::stamp_invoked_by` on every JSON envelope. The `show config` envelope additionally has its own detail block — originally named `invoked_by` (struct with `value` + `provenance` + `description`). The outer stamp's `Map::entry().or_insert` shape would NOT clobber the inner block — but the field-key collision was confusing. Renamed to `invoked_by_resolution`. The top-level stamp continues to be a flat string; the detail block is a structured object under a distinct key.

If you add an envelope-level `invoked_by` field anywhere, do not collide with the top-level stamp — pick a different name (e.g. `invoked_by_<context>`).

### `toml = "0.9"` round-trip strips comments

The Codex TOML merger uses `toml::Value` for parse + `toml::to_string_pretty` for serialise. This **drops comments and reflows whitespace**. Acceptable trade-off because `~/.codex/config.toml` is usually not handcrafted with comments. If an operator complains, swap to `toml_edit` for the merge_toml path only (notes in ROADMAP §M1.7 "Open follow-ups").

### `--agent` lost its default `"all"`

Slice 2 had `#[arg(long, default_value = "all")]`. Slice 4 made it `Option<String>` so absence triggers interactive mode. **This is technically a breaking change** for any script doing `vibe mcp install` without `--agent` — they now get an interactive prompt (or non-TTY error). Two existing e2e tests had to be updated to pass `--agent claude` / `--agent cursor` explicitly. The breakage scope is bounded — `vibe mcp install` is young (slice 2 landed in the same release window), no published consumers.

---

## Repository map

```
vibevm/
├── CLAUDE.md / AGENTS.md / GEMINI.md   # Three identical copies of the four rules + memory discipline.
├── CONTINUE.md                          # This file. Cold-resume snapshot.
├── ROADMAP.md                           # Milestone-oriented plan; M1.7 closed via slice 4.
├── VIBEVM-SPEC.md                       # Owner-frozen spec; do not edit without explicit instruction.
├── DEV-GUIDE.md / RUNTIME-GUIDE.md      # Per-machine setup docs.
├── crates/
│   ├── vibe-cli/                        # `vibe` binary entry point. clap dispatch + per-subcommand modules.
│   │   └── src/commands/
│   │       ├── mcp.rs                   # Slice 4's home: 5-agent matrix, JSON+TOML mergers, install_skill.
│   │       └── skill_template.md        # Vendored SKILL.md body (include_str! into the binary).
│   ├── vibe-core/                       # Manifests (vibe.toml, vibe-package.toml), lockfile schema v3, user_config.
│   ├── vibe-graph/                      # In-memory dep graph helpers.
│   ├── vibe-registry/                   # GitPackageRegistry, mirrors, MultiRegistryResolver, IndexClient.
│   ├── vibe-resolver/                   # Feature expansion + activation evaluation (PROP-003).
│   ├── vibe-install/                    # Install pipeline: plan_install → apply → register.
│   ├── vibe-llm/                        # LLM provider abstraction. Skeleton only — real impls land in M1.5.
│   ├── vibe-mcp/                        # JSON-RPC MCP server. 3 tools today: query_package, read_subskill, materialise_subskill.
│   ├── vibe-check/                      # Spec-consistency linter (`vibe check`).
│   ├── vibe-publish/                    # GitHubCreator / GitVerseCreator / DirectGitCreator publishers.
│   └── vibe-wire/                       # JTD-codegen'd wire types.
├── services/
│   └── vibe-index/                      # Standalone PROP-005 utility: per-org package index. Own Cargo workspace.
├── spec/
│   ├── boot/{00-core,90-user}.md        # Read at every session start. 90-user is the user-owned layer.
│   ├── WAL.md                           # Living checkpoint of project state. Authoritative if it diverges from this file.
│   ├── common/PROP-000…PROP-006         # Foundation policy + operating modes.
│   ├── modules/                         # Per-crate PROPs (PROP-001 git backend, PROP-002 decentralised registry, PROP-003 dep model, PROP-005 index).
│   └── research/PROP-004                # Tessl comparative research.
├── docs/
│   ├── README.md                        # User-doc index.
│   ├── architecture.md / lockfile-format.md / glossary.md / troubleshooting.md
│   ├── commands/                        # Per-subcommand reference. mcp-install.md / mcp-status.md / mcp-serve.md landed slice 4.
│   ├── guides/                          # Long-form walkthroughs. agent-mcp-quickstart-opencode.md landed slice 4.
│   └── authoring-{flow,feat,stack}.md
├── manual-tests/                        # Runnable smoke protocols. Walked manually before tagging milestones.
├── fixtures/registry/                   # Hermetic per-package registry fixtures (used by cargo test).
├── tools/                               # self-check.sh + jtd-codegen install README.
└── xtask/                               # `cargo xtask codegen` / `check-codegen`.
```

---

## Architectural / policy decisions still in force

In rough order of how often they bite a fresh contributor:

1. **Four non-negotiable rules** (CLAUDE.md / AGENTS.md / GEMINI.md identical copies, authoritative reference [PROP-000 §12](spec/common/PROP-000.md#commits)):
   1. **No AI / machine-author attribution** anywhere — no commit messages, trailers, comments, branch names. The CLAUDE.md attribution paragraph is the only place that topic is discussed in the repo.
   2. **Conventional Commits.** Subject ≤ 60 chars (hard limit 72), body explains WHY.
   3. **Group commits by meaning**, never by file or by time. Mixed working trees split into N commits.
   4. **Autonomy on routine changes.** Routine work commits + pushes without asking; non-routine red lines (history rewrite, `--force` push, large blobs, CI / signing / secrets, irreversible ops) STILL require explicit owner sign-off.

2. **Memory discipline.** Project facts live in the repo (`spec/`, `MEMORY.md` pointer, `CLAUDE.md`, this `CONTINUE.md`). Per-machine facts only live in tool-specific user-memory.

3. **Vocabulary lock.** Only `flow`, `feat`, `stack`, `tool`. Never `lifecycle` / `phase` / `goal` / `plugin` (except as passing synonym for `package`). See `VIBEVM-SPEC.md` §4.

4. **Language: Rust.** Permissive licenses only (MIT / Apache-2.0 / BSD / Unlicense; MPL-2.0 case-by-case; GPL / AGPL / LGPL forbidden). `dependency weight is not a decision factor` per PROP-000 §15 — pick best-in-class.

5. **Manifest format: TOML for human-edited (`vibe.toml`, `vibe-package.toml`, `vibe.lock`); JTD+codegen for wire contracts** (`schemas/`, `crates/vibe-wire/src/generated/`).

6. **Identity: `(kind, name, version, content_hash)`.** URL is informational. Mirror-switching and host-migration never invalidate `content_hash`. PROP-002 §2.1.

7. **Token secrecy** (PROP-000 §20). `~/.vibevm/<host>.publish.token` files are surface-secret. Never printed in stdout, stderr, error messages, JSON envelopes, lockfiles, commits. The `redacted` provenance in `vibe show config` is the only legal way to reference token state.

8. **Repository hosts.** vibevm source = GitVerse (`git@gitverse.ru:anarchic/vibevm.git`). Package registry = GitHub (`https://github.com/vibespecs`). Reason: GitVerse public REST API does not expose org-scoped repo creation, so the registry org migrated to GitHub on 2026-04-29 while the source repo stays on GitVerse. Documented in [PROP-000 §7](spec/common/PROP-000.md#registry).

9. **User-owned files** (vibevm install/uninstall NEVER touches): `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any `00-09` or `90-99` boot file.

10. **PROP-006 codewords.** Owner can flip the session into an alternate posture via codeword (`«move fast and break things»` is the first one). Codewords never override the four rules — only Rule 4's "ask before routine" subclause is suspended; red-line list still gates non-routine ops.

---

## Recent commit chain (last 25, newest first)

```
3bf2462 docs(guides): opencode + vibevm hello-world quickstart + acceptance gate
7cb1f33 docs(commands,roadmap,wal): M1.7 slice 4 — multi-agent + skill + invoked-by
71229eb feat(vibe-cli/mcp): interactive install + --auto + --with/without-skill
d384a96 feat(vibe-cli/mcp): vibevm SKILL.md template + per-agent writer
2eaf544 feat(vibe-cli): --invoked-by global flag + VIBE_INVOKED_BY env
05ce2e4 feat(vibe-cli/mcp): claude-desktop, opencode, codex + JSON/TOML mergers
8ce7b6a docs(commands): refresh vibe search reference for purl/full-scan/cache
e4000c3 feat(vibe-cli): vibe search --purl + --full-scan + persistent cache
7745b19 feat(vibe-registry): IndexClient::lookup_purl + Serialize on results
c585437 docs(commands): vibe search reference
506dcf2 feat(vibe-cli): vibe search command (ROADMAP §M2.10)
622ea55 feat(vibe-registry): IndexClient::search via /v1/packages?q=
c54fa51 docs(wal): rate-limiter slice + parked §9 open questions
039bd96 feat(services/vibe-index): per-token + per-IP rate limiter
ae990ae docs(wal): PROP-005 trailing-fixup slices 16–19
867ab97 feat(services/vibe-index): structured stub envelope for --from-gitverse
6e7487d feat(services/vibe-index): init writes README.md + .gitignore
7665af2 feat(services/vibe-index): by-cap + by-purl inverted index files
da25eca feat(services/vibe-index): primary.jsonl.gz sibling + serve route
e0b156a docs(wal): PROP-005 closed end to end (slices 8–11)
db26a63 docs(vibe-index): operator handbook + consumer protocol + format + smoke
86e3a16 feat(vibe-registry): index-aware list_versions fast path (PROP-005 slice 10)
97cdb9d feat(vibe-publish,vibe-cli): post-publish index hook (PROP-005 slice 9)
f217178 feat(services/vibe-index): reindex --from-github via REST API + clone (slice 8)
398d2a1 docs(wal): PROP-005 slices 1–7 + PROP-006 codeword landings
```

---

## Quick-start commands

```powershell
# Build everything.
cargo build --workspace

# Full test gate (matches CI).
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p vibe-cli -- check --path . --quiet

# Or one-shot via the bundled script.
bash tools/self-check.sh

# Install vibe into ~/.cargo/bin/ (recommended for any agent integration walk).
cargo install --path crates/vibe-cli --locked

# Walk the slice-4 acceptance gate (clean sandbox + opencode quickstart).
# See docs/guides/agent-mcp-quickstart-opencode.md.
```

---

## Pointer

`spec/WAL.md` is the canonical **living** checkpoint of project state. If anything in this `CONTINUE.md` disagrees with the top of `spec/WAL.md`, trust the WAL — it gets bumped every session, this file gets bumped only at session-end. The WAL's `## Current phase` block is rewritten newest-first; older checkpoints are preserved below for context.
