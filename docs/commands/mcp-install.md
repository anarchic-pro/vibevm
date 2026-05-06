# `vibe mcp install` — wire vibevm into a coding agent

Detects supported coding agents on this machine + the current project and writes the per-agent MCP server configuration so the agent picks up vibevm automatically on its next session start. Optionally also installs the `vibevm` SKILL.md — a short, binding instruction file the agent loads when its description matches the user's task.

Idempotent — already-correct configs surface as `unchanged`.

Spec: [PROP-004 §5.1](../../spec/research/PROP-004-tessl-comparative-research.md), [`spec/WAL.md`](../../spec/WAL.md) (M1.7 slice 4).

## Supported agents

Five agents land in slice 4. Detection runs against project markers (project-scoped agents) or the existence of the agent's user-config dir (user-scoped agents).

| Agent | Markers | Config file | Format | Skill loader |
| --- | --- | --- | --- | --- |
| `claude` | `.claude/`, `CLAUDE.md` | `<project>/.claude/settings.json` | JSON | yes — `~/.claude/skills/` or `<project>/.claude/skills/` |
| `claude-desktop` | (user-level only) `<config-dir>/Claude/` exists | `<config-dir>/Claude/claude_desktop_config.json` | JSON | no |
| `cursor` | `.cursor/`, `.cursorrules` | `<project>/.cursor/mcp.json` | JSON | no |
| `opencode` | `.opencode/`, `opencode.json`, `opencode.jsonc`, `AGENTS.md` | `<project>/opencode.json` | JSON (key `mcp`) | yes — `<project>/.opencode/skills/` or `<config-dir>/opencode/skills/` |
| `codex` | (user-level only) `~/.codex/` exists | `~/.codex/config.toml` | TOML (key `mcp_servers`) | yes — `<project>/.agents/skills/` or `~/.agents/skills/` |

`<config-dir>` resolves through `dirs::config_dir()` — `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, `~/.config` on Linux.

## Usage

```
vibe mcp install [--path <dir>]
                 [--agent <FILTER> | --auto]
                 [--with-skill | --without-skill]
                 [--skill-scope project | user]
                 [--dry-run]
                 [--force]
```

Without flags, drops into an interactive multi-select picker (TTY required). For CI / scripts use `--auto` (install everywhere, skill on by default) or `--agent <name>` (one explicit target).

## Flags

| Flag | Description | Default |
| --- | --- | --- |
| `--path <dir>` | Project root with `vibe.toml`. | `.` |
| `--agent <FILTER>` | One of `all`, `claude`, `claude-desktop`, `cursor`, `opencode`, `codex`. Conflicts with `--auto`. | (interactive if absent) |
| `--auto` | Detect every supported agent and install in all of them. No prompts. Skill defaults to on; `--without-skill` overrides. Conflicts with `--agent`. | off |
| `--with-skill` | Also write `vibevm` SKILL.md alongside the MCP config. Cursor / Claude Desktop are reported as `skipped`. Conflicts with `--without-skill`. | (mode-dependent — see below) |
| `--without-skill` | Install only the MCP server entry. Conflicts with `--with-skill`. | (mode-dependent) |
| `--skill-scope project | user` | Where SKILL.md lands. `project` writes under the per-agent project skill dir (committed to git); `user` writes to the operator's home / config dir. | `project` |
| `--dry-run` | Print what would be written without touching disk. | off |
| `--force` | Provision the agent's config even when no presence marker is detected. | off |
| `--json` (global) | Emit a structured envelope; see [Output (JSON)](#output-json). | off |
| `--quiet` (global) | One-line summary `N MCP configs + M skill files written`. | off |
| `--invoked-by <agent>` (global) | Stamps `invoked_by` on the JSON envelope; see [`vibe show config`](show.md). | (env: `VIBE_INVOKED_BY`, else absent) |

### Skill toggle defaults

When neither `--with-skill` nor `--without-skill` is set, the resolved value depends on the install mode:

- `--auto` → on (CI / first-run scripts get the full opinionated setup).
- explicit `--agent <name>` → off (advanced operators opt in).
- interactive → asks via a `Yes/No` prompt.

## Examples

```bash
# Interactive — pick agents + skill toggle. TTY required.
vibe mcp install

# CI / first-run script — install MCP + skill in every detected agent.
vibe mcp install --auto

# CI variant — MCP only, no skill.
vibe mcp install --auto --without-skill

# Single explicit target — Claude Code, MCP only.
vibe mcp install --agent claude

# Single explicit target — OpenCode with skill in user-level scope.
vibe mcp install --agent opencode --with-skill --skill-scope user

# Dry-run preview before committing changes.
vibe mcp install --auto --dry-run

# Force-provision even when the agent's marker dir is absent.
vibe mcp install --agent claude --force

# Pass the calling agent's identity so envelopes carry attribution.
vibe --invoked-by opencode mcp install --auto --json
```

## Output

### Human-readable

```
→ created mcp     claude  → /home/dev/proj/.claude/settings.json
→ created skill   claude (project)  → /home/dev/proj/.claude/skills/vibevm/SKILL.md
→ unchanged mcp   opencode  → /home/dev/proj/opencode.json
→ skipped skill   cursor (project)  → (no skill loader) (agent `cursor` does not load filesystem skills)
```

### Output (JSON)

```jsonc
{
  "ok": true,
  "command": "mcp:install",
  "project": "/home/dev/proj",
  "detected": ["claude", "cursor", "opencode"],
  "targeted": ["claude", "cursor", "opencode"],
  "results": [
    {
      "agent": "claude",
      "config_path": "/home/dev/proj/.claude/settings.json",
      "status": "created",
      "note": "file does not exist yet"
    },
    {
      "agent": "cursor",
      "config_path": "/home/dev/proj/.cursor/mcp.json",
      "status": "unchanged",
      "note": null
    },
    {
      "agent": "opencode",
      "config_path": "/home/dev/proj/opencode.json",
      "status": "created",
      "note": "file does not exist yet"
    }
  ],
  "skill_results": [
    {
      "agent": "claude",
      "scope": "project",
      "path": "/home/dev/proj/.claude/skills/vibevm/SKILL.md",
      "status": "created",
      "note": null
    },
    {
      "agent": "cursor",
      "scope": "project",
      "path": null,
      "status": "skipped",
      "note": "agent `cursor` does not load filesystem skills"
    },
    {
      "agent": "opencode",
      "scope": "project",
      "path": "/home/dev/proj/.opencode/skills/vibevm/SKILL.md",
      "status": "created",
      "note": null
    }
  ],
  "skill_scope": "project",
  "install_skill": true,
  "mode": "auto",
  "dry_run": false,
  "invoked_by": "opencode"
}
```

`status` vocabulary:

- `created` — file did not exist; we wrote it.
- `updated` — file existed but differed; we rewrote it. (For the JSON / TOML mergers, foreign keys outside the `mcpServers` / `mcp` / `mcp_servers` block are preserved.)
- `unchanged` — byte-identical block already on disk; nothing written.
- `would-create` / `would-update` — `--dry-run` previews of `created` / `updated`.
- `skipped` — agent does not support the requested action (skill writes for Cursor / Claude Desktop).

`mode` records the path the operator took:

- `auto` — `--auto` was used.
- `agent-flag` — `--agent <FILTER>` was used.
- `interactive` — neither; the multi-select picker ran.

## What gets written

### Claude Code / Claude Desktop / Cursor (JSON, `mcpServers`)

```jsonc
{
  "mcpServers": {
    "vibevm": {
      "command": "vibe",
      "args": ["mcp", "serve", "--path", "/home/dev/proj"]
    }
  }
}
```

### OpenCode (JSON, `mcp`, command-array shape)

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "vibevm": {
      "type": "local",
      "command": ["vibe", "mcp", "serve", "--path", "/home/dev/proj"],
      "enabled": true
    }
  }
}
```

### Codex (TOML, `mcp_servers`)

```toml
[mcp_servers.vibevm]
command = "vibe"
args = ["mcp", "serve", "--path", "/home/dev/proj"]
```

### SKILL.md (Claude Code, OpenCode, Codex)

A short opinionated MD — YAML frontmatter (`name: vibevm`, description matching every vibevm signal) plus a body that:

- pins the bootstrap protocol (`CLAUDE.md` / `AGENTS.md` first, then `spec/boot/*`, then `spec/WAL.md`, then relevant `PROP-*` / `FEAT-*`),
- documents the three MCP tools (`query_package`, `read_subskill`, `materialise_subskill`) and *requires* the agent to call them before guessing about installed packages,
- *requires* the agent to pass `--invoked-by <YOUR-AGENT>` (or `VIBE_INVOKED_BY=<your-agent>`) on every `vibe` invocation,
- *requires* the agent to consult `vibe <subcommand> --help` before suggesting a command (the CLI's truth is its own help text, not training data),
- inherits the four non-negotiable rules (no AI attribution, Conventional Commits, group commits by meaning, ask before destructive operations).

The exact body is the `crates/vibe-cli/src/commands/skill_template.md` file, vendored at compile time so it ships byte-identical inside the `vibe` binary.

## Edge cases

- **No agents detected, no `--force`.** Empty `targeted` list; the run succeeds with a "no supported agents detected" summary. Pass `--force` to provision configs even without markers.
- **Non-TTY without `--auto` / `--agent`.** The interactive picker refuses with a hint pointing at `--auto` / `--agent`. Useful in CI where stdin is piped.
- **User-level `--auto` writes.** Claude Desktop and Codex configs live in the operator's home / config dir, not the project tree. `--auto` will touch them when their parent dir exists. `--dry-run` is the safe way to preview before committing.
- **Stale skill content.** `install_skill` overwrites stale on-disk SKILL.md with the current template — the contract is set by the binary, not the operator. Run `vibe mcp install --with-skill` after upgrading vibevm to re-sync.
- **Foreign keys.** The JSON / TOML mergers preserve every key outside the `mcpServers` / `mcp` / `mcp_servers` block. An existing `[provider.lmstudio]` in `~/.config/opencode/opencode.json` survives a vibevm install.

## Related

- [`vibe mcp status`](mcp-status.md) — same shape, no writes.
- [`vibe mcp serve`](mcp-serve.md) — the JSON-RPC server itself, invoked from each agent's MCP config.
- [`vibe show config`](show.md) — surfaces the resolved `--invoked-by` value with provenance.
