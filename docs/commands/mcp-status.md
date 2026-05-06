# `vibe mcp status` — preview agent integration state

Read-only counterpart of [`vibe mcp install`](mcp-install.md). Walks every supported agent (Claude Code, Claude Desktop, Cursor, OpenCode, Codex), works out what `vibe mcp install --agent <name>` would write, and reports per-agent status without touching disk. Useful as a CI gate to catch config drift, or as a one-shot probe to see which agents this project + machine combination would integrate with.

Spec: [PROP-004 §5.1](../../spec/research/PROP-004-tessl-comparative-research.md), [`spec/WAL.md`](../../spec/WAL.md) (M1.7 slice 2 + 4).

## Usage

```
vibe mcp status [--path <dir>]
```

## Flags

| Flag | Description | Default |
| --- | --- | --- |
| `--path <dir>` | Project root with `vibe.toml`. | `.` |
| `--json` (global) | Structured envelope; see [Output (JSON)](#output-json). | off |
| `--quiet` (global) | One-line summary. | off |
| `--invoked-by <agent>` (global) | Stamps `invoked_by` on the JSON envelope. | (env: `VIBE_INVOKED_BY`, else absent) |

## Output

### Human-readable

```
Detected agents: claude, cursor, opencode
would-create  claude  → /home/dev/proj/.claude/settings.json
would-create  cursor  → /home/dev/proj/.cursor/mcp.json
would-create  opencode  → /home/dev/proj/opencode.json
would-create  claude-desktop  → /home/dev/.config/Claude/claude_desktop_config.json
would-create  codex  → /home/dev/.codex/config.toml
```

### Output (JSON)

```jsonc
{
  "ok": true,
  "command": "mcp:status",
  "project": "/home/dev/proj",
  "detected": ["claude", "cursor", "opencode"],
  "results": [
    {
      "agent": "claude",
      "config_path": "/home/dev/proj/.claude/settings.json",
      "status": "would-create",
      "note": "file does not exist yet"
    },
    {
      "agent": "claude-desktop",
      "config_path": "/home/dev/.config/Claude/claude_desktop_config.json",
      "status": "would-create",
      "note": "file does not exist yet"
    },
    {
      "agent": "cursor",
      "config_path": "/home/dev/proj/.cursor/mcp.json",
      "status": "would-create",
      "note": "file does not exist yet"
    },
    {
      "agent": "opencode",
      "config_path": "/home/dev/proj/opencode.json",
      "status": "would-create",
      "note": "file does not exist yet"
    },
    {
      "agent": "codex",
      "config_path": "/home/dev/.codex/config.toml",
      "status": "would-create",
      "note": "file does not exist yet"
    }
  ]
}
```

`status` is one of `would-create`, `would-update`, `unchanged` — same vocabulary as `vibe mcp install --dry-run`. Skill installation is **not** previewed by `mcp status`; use `vibe mcp install --with-skill --dry-run` for that.

## CI usage — drift gate

```bash
# Fail the build if any agent's vibevm block has drifted.
vibe --json mcp status \
  | jq -e '[.results[] | select(.status == "would-update" or .status == "would-create")] | length == 0' \
  || { echo "vibevm MCP config drift detected; run 'vibe mcp install'"; exit 1; }
```

## Related

- [`vibe mcp install`](mcp-install.md) — actually writes the configs.
- [`vibe mcp serve`](mcp-serve.md) — the server the configs point at.
