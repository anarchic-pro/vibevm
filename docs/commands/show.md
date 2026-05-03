# `vibe show` — inspect computed project state

Pure-inspection commands that materialise the project's current state for a human or a downstream tool. No mutation, no network.

Spec: [`VIBEVM-SPEC.md` §4.6](../../VIBEVM-SPEC.md) (effective spec), [§9.5](../../VIBEVM-SPEC.md) (configuration sources / provenance), [ROADMAP §M1.4](../../ROADMAP.md#m14--vibe-show-).

## Subcommands

| Command | Purpose |
| --- | --- |
| [`vibe show effective`](#vibe-show-effective) | Concatenate every spec/boot file + every installed package's `files_written` (in lockfile order) with `spec://` provenance headers. The cold-reader view of the project's spec corpus. |
| [`vibe show config`](#vibe-show-config) | Effective configuration: every `[[registry]]` / `[[mirror]]` / `[[override]]` plus runtime env-vars, each tagged with provenance (`vibe.toml`, `env`, `redacted`, `default`). |

The runner-aware subcommands listed in `VIBEVM-SPEC.md` §9.1 — `vibe show graph`, `vibe show node`, `vibe show plan` — are deferred to **M1.5**; they need the LLM-build pipeline's task-graph runner before they have anything meaningful to render.

## `vibe show effective`

```
vibe show effective [--path <dir>] [--json | --quiet]
```

### Output

For each section (a single file's content):

```
--- spec://project/boot/00-core.md (user)
--- path: spec/boot/00-core.md

(file body verbatim)

--- spec://project/boot/10-flow-wal.md (package:flow:wal@0.1.0)
--- path: spec/boot/10-flow-wal.md

(file body verbatim)

--- spec://project/WAL (wal)
--- path: spec/WAL.md

(file body verbatim)

--- spec://flow/wal/0.1.0/flows/wal/WAL-PROTOCOL.md (package:flow:wal@0.1.0)
--- path: spec/flows/wal/WAL-PROTOCOL.md

(file body verbatim)
```

The walking order is stable:

1. `spec/boot/*.md` sorted by filename (matches the canonical `NN-` prefix order operators read at session boot).
2. `spec/WAL.md` if it exists.
3. Per `[[package]]` in lockfile order: every `files_written` path that doesn't start with `spec/boot/` (those already landed in step 1), sorted within the package.

Provenance origins:

| Origin | Meaning |
| --- | --- |
| `user` | `spec/boot/00-core.md`, `spec/boot/90-user.md`, or any boot file not claimed by a lockfile entry's `boot_snippet`. |
| `wal` | `spec/WAL.md`. |
| `package:<kind>:<name>@<version>` | File contributed by an installed package (mirror layout / boot snippet). |
| `package:<kind>:<name>@<version> (MISSING ON DISK)` | Lockfile claims this file but it isn't on disk. `vibe check` is the dedicated path for surfacing this — show effective is best-effort and continues. |

### JSON shape

```jsonc
{
  "ok": true,
  "command": "show:effective",
  "project": "/abs/path",
  "sections": [
    {
      "spec_uri": "spec://project/boot/00-core.md",
      "path": "spec/boot/00-core.md",
      "origin": "user",
      "body": "<full file contents>"
    },
    ...
  ]
}
```

### Examples

```bash
# Whole effective spec to stdout.
vibe show effective

# Just paths and origins, machine-parseable.
vibe show effective --json | jq '.sections[] | {path, origin}'

# Cold-reader handoff: write the effective spec to a file an
# external session can paste into context.
vibe show effective > /tmp/handoff.md
```

## `vibe show config`

```
vibe show config [--path <dir>] [--json | --quiet]
```

### Output

```
Project: demo 0.0.1 (/abs/path)

Registries (1; primary first):
  1. vibespecs (primary)
     url:    https://github.com/vibespecs
     ref:    main
     naming: kind-name
     source: vibe.toml

Mirrors (0):
  (none configured)

Overrides (0):
  (none configured)

Environment:
  VIBE_REGISTRY_CACHE  [source: default]
    Override the default `~/.vibe/registries/` cache root.
    (unset; using built-in default)
  VIBE_LOG  [source: env]
    Tracing filter (reads `tracing-subscriber::EnvFilter`).
    `vibe_registry=info`
  VIBEVM_PUBLISH_TOKEN  [source: redacted]
    Publish token for `vibe registry publish` (host-agnostic; ...).
    (redacted; set in environment)
```

### Provenance values

| Provenance | When |
| --- | --- |
| `vibe.toml` | Read from the project manifest. Every `[[registry]]` / `[[mirror]]` / `[[override]]` block sources here in v0. |
| `env` | Set in the environment, raw value safe to print. |
| `redacted` | Set in the environment, but the value is sensitive (token-shaped); the real bytes are never displayed — `vibe show config` prints `(redacted; set in environment)` instead. Same secrecy invariant `vibe registry publish` applies. |
| `default` | Not set in the environment; the runtime falls back to its built-in default. |

User-level `~/.config/vibe/config.toml` and CLI overrides will surface as additional provenance values once those layers ship — `VIBEVM-SPEC.md` §9.5 lists the full precedence chain.

### JSON shape

```jsonc
{
  "ok": true,
  "command": "show:config",
  "project": "/abs/path",
  "project_name": "demo",
  "project_version": "0.0.1",
  "registries": [
    { "name": "vibespecs", "url": "...", "ref": "main", "naming": "kind-name", "provenance": "vibe.toml" }
  ],
  "mirrors": [],
  "overrides": [],
  "env": [
    { "name": "VIBEVM_PUBLISH_TOKEN", "value": "(redacted; set in environment)", "provenance": "redacted", "description": "..." }
  ]
}
```

### Examples

```bash
# Eyeball the whole config.
vibe show config

# Programmatic: which registry would `vibe registry publish` target?
vibe --json show config | jq -r '.registries[0].url'

# CI gate: every env var with a known role must be either env or default
# (catches a misspelled env var that landed nowhere).
vibe --json show config | jq -e '.env[] | select(.provenance == "env" or .provenance == "default" or .provenance == "redacted")'
```

## Limitations (v0)

- `vibe show graph`, `vibe show node`, `vibe show plan` are deferred to M1.5 alongside the LLM-build runner.
- User-level `~/.config/vibe/config.toml` is not read in v0 — the only `provenance` value beside `env` / `default` / `redacted` is `vibe.toml`. Adding the user-level layer is a follow-up commit.
- The effective-spec view does not parse markdown; it concatenates raw file content. `vibe check` v1+ will add anchor-aware analysis, but `vibe show effective` is intentionally byte-faithful.

## Related

- [`vibe list`](list.md) — concise lockfile-only view of installed packages.
- [`vibe check`](check.md) — runs the spec-consistency linter; pairs naturally with `vibe show effective` when handing off context to a different session.
- [`vibe registry list`](registry-list.md) — registry / mirror / override block dump (a subset of what `show config` emits, focused on the registry layer).
