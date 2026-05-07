# Quickstart: opencode + vibevm hello-world

End-to-end walkthrough that takes a fresh machine with `opencode` already installed and ends with the agent successfully calling vibevm over MCP, querying lockfile data, and producing a hello-world artefact in a sandbox project.

This document is **dual-purpose by design**:

- **As a tutorial** — copy-paste each command, follow the prompts, you should arrive at a working setup within 5 minutes.
- **As an integration test** — the [Acceptance checklist](#acceptance-checklist) below is a machine-readable list of facts that must be true after a successful run. Run this guide before tagging a vibevm release; if any checkbox fails, slice 4 has regressed.

If you are looking for the per-command reference rather than a walkthrough, see [`docs/commands/mcp-install.md`](../commands/mcp-install.md), [`mcp-status.md`](../commands/mcp-status.md), [`mcp-serve.md`](../commands/mcp-serve.md).

Tested on: Windows 11 Pro for Workstations, PowerShell 5.1, opencode 1.14.x, GLM-4.7-Flash via LM Studio (any tool-use-capable model works). Vibevm `HEAD` ≥ `7cb1f33` (M1.7 slice 4).

---

## 0. Prerequisites

- Working `opencode` in PATH (`where.exe opencode` returns a path).
- A model with tool-use support configured in opencode (Claude / GPT-4o / Llama-3.1 / GLM-4 / etc.). Check your `~/.config/opencode/opencode.json`.
- Internet access to `github.com` (for `vibe install flow:wal`).
- Rust toolchain installed (only if you want the persistent-PATH variant in the next step).

---

## 1. Make `vibe` discoverable to opencode

opencode launches `vibe mcp serve …` as a subprocess; `vibe` must resolve in the PATH the opencode process inherits. Pick one variant:

**Variant A — persistent (recommended).** `cargo install` puts `vibe.exe` into `~/.cargo/bin/`, which is already in PATH after Rust setup:

```powershell
cd C:\Users\olegc\gits\vibevm
cargo install --path crates/vibe-cli --locked
where.exe vibe        # should print C:\Users\olegc\.cargo\bin\vibe.exe
vibe --version        # vibe 0.1.0-dev
```

**Variant B — one-session.** Add the debug build dir to PATH for this PowerShell session only:

```powershell
$env:PATH = "C:\Users\olegc\gits\vibevm\target\debug;$env:PATH"
where.exe vibe        # should print the target\debug path
vibe --version
```

With variant B, run **everything else** (including `opencode`) in the same PowerShell window — a fresh window won't inherit the PATH override.

---

## 2. Create a sandbox project

```powershell
mkdir C:\Users\olegc\hello-vibe
cd C:\Users\olegc\hello-vibe
```

---

## 3. `vibe init`

```powershell
vibe init
```

Lands `vibe.toml`, an empty `vibe.lock`, `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, and the three cross-agent rule files (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`).

---

## 4. Install a real package

```powershell
vibe install flow:wal
```

Pulls `https://github.com/vibespecs/flow-wal` at tag `v0.1.0`, materialises subskills under `spec/flows/wal/`, and writes the lockfile entry. The MCP demo needs a non-empty lockfile for `query_package` to return real data.

Sanity probes:

```powershell
vibe list                  # should show flow:wal v0.1.0
vibe show subskills        # active subskills
type vibe.lock             # lockfile entry
```

---

## 5. Wire opencode to vibevm

```powershell
vibe mcp install --agent opencode --with-skill
```

Expected output:

```
→ created mcp     opencode  → C:/Users/olegc/hello-vibe/opencode.json
→ created skill   opencode (project)  → C:/Users/olegc/hello-vibe/.opencode/skills/vibevm/SKILL.md
```

Inspect what landed:

```powershell
type opencode.json
# expected: { "$schema": ..., "mcp": { "vibevm": { "type": "local",
#            "command": ["vibe","mcp","serve","--path","C:/Users/olegc/hello-vibe"],
#            "enabled": true } } }

type .opencode\skills\vibevm\SKILL.md
# YAML frontmatter (---/name: vibevm/description: .../---) + body
```

---

## 6. Hand-test the MCP server (optional but reassuring)

```powershell
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | vibe mcp serve --path C:\Users\olegc\hello-vibe
```

Expected single-line response:

```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","serverInfo":{"name":"vibe-mcp","version":"0.1.0-dev"},"capabilities":{"tools":{"listChanged":false}}}}
```

If this works, opencode will too — the wire format is identical.

---

## 7. Launch opencode

```powershell
opencode
```

Run it from the **same** PowerShell session you used for the previous steps (matters for variant B). If opencode was already running, exit and relaunch so it re-reads the new project-level `opencode.json`. The global `~/.config/opencode/opencode.json` keeps the model setting; the project config layers `mcp.vibevm` on top.

---

## 8. Demo prompts

Pick one (or run all three in order — they escalate from cheap probe to full hello-world).

### Prompt A — minimum (probes the MCP wire)

```
Use the vibevm MCP server. Call the `query_package` tool with name "wal".
Show me the JSON it returns and unpack: which version is installed, what
content_hash, what subskills_active, what describes-PURL. Do not paraphrase
the spec — I want facts straight from the lockfile.
```

**Pass criterion:** opencode visibly issues a `query_package` tool call (TUI shows it) and returns a structure with `version: "0.1.0"`, a populated `content_hash` (`sha256:…`), and a non-empty `subskills_active` array.

### Prompt B — hello-world project (full demo)

```
This directory is a vibevm-managed project with the flow:wal package installed.
Get familiar with the WAL protocol: call query_package("wal"), then
read_subskill for every subskill returned in subskills_active. Then
build a minimal "Hello World" project:

1. Create README.md with a one-line description.
2. Create docs/hello.md saying "Hello, world!" plus one line about the author.
3. Update spec/WAL.md per the WAL protocol so it has a `## current phase`
   section reflecting that we just started the project.
4. Pass `--invoked-by opencode` on every `vibe` CLI call you issue.
5. Before suggesting any vibe subcommand, run `vibe <subcmd> --help` and
   read the actual current surface — do not invent flags from memory.

End with a short next-steps plan that follows the WAL protocol.
```

**Pass criteria:**

- opencode calls `query_package` and `read_subskill` (visible in TUI).
- `README.md` and `docs/hello.md` materialise.
- `spec/WAL.md` gains a coherent `## current phase` section.
- Any `vibe …` invocation in the transcript carries `--invoked-by opencode`.

### Prompt C — cheapest skill smoke (use if your model lacks tool-use)

```
Show me the contents of .opencode/skills/vibevm/SKILL.md. Summarise in
three lines what this skill instructs you to do when working in this
project.
```

**Pass criterion:** opencode's summary mentions the bootstrap protocol, the MCP tools, and `--invoked-by`. If it answers "I don't know," the skill did not activate — see [Troubleshooting](#troubleshooting).

---

## Acceptance checklist

Use this list as both an integration-test gate (every box must be checkable) and a triage tool (an unchecked box points at exactly which slice regressed). Copy into a PR description when shipping changes that touch slice 4 surface.

- [ ] `where.exe vibe` resolves to a built-and-installed binary.
- [ ] `vibe --version` exits 0 and prints a version.
- [ ] `vibe init` creates `vibe.toml`, `vibe.lock`, `spec/boot/`, `spec/WAL.md`, `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`.
- [ ] `vibe install flow:wal` exits 0; `vibe.lock` records `flow:wal@0.1.0` with a populated `content_hash`.
- [ ] `vibe mcp install --agent opencode --with-skill` exits 0 and reports `created mcp opencode` + `created skill opencode (project)`.
- [ ] `opencode.json` at the project root contains `mcp.vibevm` with `type: "local"`, `command: [array]`, `enabled: true`.
- [ ] `.opencode/skills/vibevm/SKILL.md` exists, starts with `---`, contains `name: vibevm`, `description: …`, the second `---`, then a body referencing `query_package`, `read_subskill`, `materialise_subskill`, `--invoked-by`, and `VIBE_INVOKED_BY`.
- [ ] The hand-test in step 6 returns a JSON envelope with `protocolVersion: "2024-11-05"`.
- [ ] After launching opencode in the project dir and running Prompt A, the TUI shows a tool call to `query_package` with the response carrying the same `content_hash` you saw in `vibe.lock`.
- [ ] After Prompt B, `README.md` + `docs/hello.md` exist with non-empty content; `spec/WAL.md` has a `## current phase` section that matches the protocol from `flow:wal`.
- [ ] Any `vibe …` lines in the opencode transcript include `--invoked-by opencode` (Prompt B requirement).
- [ ] `vibe --invoked-by opencode --json show config` emits a top-level `"invoked_by": "opencode"` field.

If you check every box on a clean machine, slice 4 (multi-agent + skill + invoked-by) is healthy end-to-end.

---

## Troubleshooting

**`opencode: command not found`** — opencode's installer didn't put it in PATH for the current shell. Use the terminal where opencode was working before.

**MCP server unreachable from opencode (`vibe not found` / spawn error).** opencode inherits PATH from its parent. With variant B (one-session PATH), launch opencode from the same PowerShell. Better: switch to variant A (`cargo install`).

**Model refuses to call tools.** Your model lacks tool-use. GLM-4.7-Flash via LM Studio normally works, but if it answers "I cannot call tools," switch the model in opencode (`/models` or edit `~/.config/opencode/opencode.json`) to a known tool-use-capable one (any modern Claude / GPT-4o / Llama-3.1+).

**Skill not auto-loading.** Verify the file path and frontmatter:

```powershell
type .opencode\skills\vibevm\SKILL.md | findstr /i "name description"
# should print: name: vibevm
#                description: Use whenever the workspace contains a `vibe.toml` file. ...
```

Newer opencode versions activate skills via an explicit agent-side `skill` tool call, not auto by description match. In that case explicitly ask: *"Load the skill named `vibevm` and follow its instructions for the rest of this conversation."*

**`vibe install flow:wal` fails with a network or auth error.** Read-only clone needs no token. Probe with `git ls-remote https://github.com/vibespecs/flow-wal`. If that fails, the issue is in your network / DNS, not in vibevm.

**`vibe mcp install` reports `no supported agents detected`.** OpenCode's project markers (`.opencode/`, `opencode.json`, `opencode.jsonc`, `AGENTS.md`) are all absent. `vibe init` ships `AGENTS.md` so the marker fires by default — re-run `vibe init` if your sandbox is unusual. Or pass `--force` to provision regardless.

**Reset and retry.** The whole sandbox lives in `C:\Users\olegc\hello-vibe`. Delete the directory and start over from step 2. If you also want to clear the per-package cache: `Remove-Item -Recurse C:\Users\olegc\.vibe`.

---

## Useful auxiliary commands

```powershell
# Detected agents on this machine + project (read-only).
vibe --json mcp status

# Resolved invoked_by with provenance.
vibe --invoked-by opencode --json show config | findstr /i invoked

# The full effective spec corpus (what an agent sees on bootstrap).
vibe show effective | more

# Lockfile in machine-readable form.
vibe show subskills --json | python -m json.tool
```

---

## Maintenance

This guide is a contract: when slice 4 surface changes, this document must change with it. Specifically:

- **CLI flag changes** (e.g. a new `vibe mcp install` flag, a renamed `--invoked-by` knob) → update sections 5 and 8 + the acceptance checklist.
- **MCP wire-shape changes** (new tool, renamed tool, field added/removed in `query_package` response) → update Prompt A and the hand-test in section 6.
- **New supported agent** (Gemini / Copilot / etc.) → file a sibling guide `docs/guides/agent-mcp-quickstart-<agent>.md` rather than expanding this one. Keep the per-agent files focused.
- **Skill template changes** (`crates/vibe-cli/src/commands/skill_template.md`) → if you edit the skill body, make sure the acceptance checklist's "SKILL.md exists / contains X" line still names something that survived the edit.
- **Hash / version drift in `flow:wal`** → step 4's `content_hash` is checked at runtime by vibevm, not pinned in this doc; if `flow:wal` re-publishes at a new content_hash, the prompts still pass — only the cross-check in Prompt A's expected output changes.

When a vibevm release ships, run this guide top-to-bottom against a clean sandbox before tagging. A failed acceptance checkbox is a release blocker.

## Related

- [`docs/commands/mcp-install.md`](../commands/mcp-install.md) — full reference for the install UX.
- [`docs/commands/mcp-status.md`](../commands/mcp-status.md) — read-only counterpart.
- [`docs/commands/mcp-serve.md`](../commands/mcp-serve.md) — server wire format.
- [`crates/vibe-cli/src/commands/skill_template.md`](../../crates/vibe-cli/src/commands/skill_template.md) — the SKILL.md body the binary ships.
- [`spec/research/PROP-004-tessl-comparative-research.md`](../../spec/research/PROP-004-tessl-comparative-research.md) §5.1 — the design rationale for the MCP integration.
- [`spec/WAL.md`](../../spec/WAL.md) — slice 4 checkpoint and forward queue.
