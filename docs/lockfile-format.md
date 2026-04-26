# `vibe.lock` — schema reference

Authoritative reference for the `vibe.lock` file at the root of every vibevm project. The lockfile is the source of truth for what is installed; `vibe list` reads it, `vibe uninstall` reads it to know what files to remove, `vibe registry sync` walks it to refresh per-package clones. **It is committed to git.**

The file is TOML 1.0. Schema is defined by [`crates/vibe-core/src/manifest/lockfile.rs`](../crates/vibe-core/src/manifest/lockfile.rs); spec text in [`VIBEVM-SPEC.md` §7.4](../VIBEVM-SPEC.md). Identity model in [`PROP-002 §2.1`](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#identity).

## Top-level shape

```toml
[meta]
generated_by      = "vibe 0.2.0-dev"
generated_at      = "2026-04-25T12:00:00Z"
schema_version    = 2
solver            = "resolvo-0.x"                   # optional
root_dependencies = ["flow:wal", "stack:rust-cli"]  # optional, may be empty

[[package]]
# ... per-package fields, repeated per installed package
```

Every key is `deny_unknown_fields` — a stray field is a hard parse error, surfaced at the next `vibe install` / `vibe update` so a typo does not silently strand state.

## `[meta]` fields

| Field | Type | Required | Semantics |
| --- | --- | --- | --- |
| `generated_by` | string | yes | Identity of the writer. Production: `vibe <version>`. Tests / fixtures: anything; checked only as a debugging breadcrumb, never parsed for behaviour. |
| `generated_at` | string | yes | RFC-3339 UTC timestamp at the moment the lockfile was written. Updated on every successful `register_installed` / `unregister_installed` call. |
| `schema_version` | uint | no, default `2` | Lockfile-format major version. `1` = M0 / M1.1 (single-`source` field per package, no provenance, no transitive deps recorded). `2` = current — full identity + provenance + transitive deps + capability vocabulary. v1 files parse with `schema_version` defaulting to `2` and migrate transparently on next write (see [v1 → v2 migration](#v1--v2-migration)). |
| `solver` | string | no | Identity of the depsolver that produced this lockfile, e.g. `"resolvo-0.x"` or `"naive-1"`. Lets a future re-resolve compare-and-replay deterministically. Absent for pre-resolver Phase-A installs (the install pipeline didn't drive a solver). |
| `root_dependencies` | array of pkgref strings | no | Packages the user directly asked for (`vibe install <pkgref>` arguments), distinct from transitives the solver pulled in. Drives `vibe uninstall` semantics: removing a root drops its entry; removing a pure transitive is rejected. Empty (absent) when no `vibe install` has run yet, or when every install was via legacy paths that didn't track roots. |

## `[[package]]` entries

Each `[[package]]` block describes one installed package.

| Field | Type | Required | Semantics |
| --- | --- | --- | --- |
| `kind` | enum (`flow`, `feat`, `stack`, `tool`) | yes | Package kind per [VIBEVM-SPEC §4.1](../VIBEVM-SPEC.md). |
| `name` | string | yes | Kebab-case package name (no kind prefix). |
| `version` | semver string | yes | Resolved exact version (`"0.3.0"`). Never a constraint. |
| `registry` | string | no | The `[[registry]].name` (from `vibe.toml`) that served this package. `None` for `LocalRegistry` (`--registry <path>`), the legacy monorepo `GitRegistry`, and override-resolved entries. The single field that lets `vibe registry sync` look up which `[[registry]]` to dispatch through. |
| `source_url` | string | yes (alias `source` for v1) | URL the content was fetched from on the install that produced this entry. Informational — see [identity model](#identity-model). v1 lockfiles use the key `source`; the v2 reader accepts it via `#[serde(alias = "source")]` and writes `source_url` on the next save. |
| `source_ref` | string | no | Git ref the content was fetched at — typically `v<version>` for per-package registries; the override's ref for `[[override]]` resolutions. `None` for non-git sources (`file://...`, M0 local-directory installs). |
| `resolved_commit` | string | no | Commit hash the ref resolved to at install time. Lets a future `vibe check` detect silent tag rewrites (commit changed but `(kind, name, version)` stayed the same — a force-pushed tag). Reserved; populated by the resolver when `git rev-parse` plumbing wires through. Absent today. |
| `content_hash` | string (`sha256:<hex>`) | yes | Hash over the deterministically-ordered file tree. The **identity** half of the `(kind, name, version, content_hash)` tuple — see [identity model](#identity-model). |
| `boot_snippet` | string | no | Filename (under `spec/boot/`) of the boot snippet this package contributes. `None` for packages without a boot snippet. |
| `files_written` | array of strings | yes (may be empty) | Forward-slash-normalised relative paths, every file the install wrote. `vibe uninstall` reads this list to know what to remove. User-owned paths are never present here (filtered at plan time). |
| `dependencies` | array of pkgref strings | no, default `[]` | Transitive deps the solver chose, pinned to exact versions (`"flow:atomic-commits@=0.1.0"`). Reproduces the resolved graph on a fresh install from this lockfile. Empty for v1 lockfiles and pre-resolver Phase-A installs. |
| `overridden` | bool | no, default `false` | True iff this package was resolved through `[[override]]` rather than the registry layer. `vibe list --overrides` filters on this; the deliberate-divergence escape hatch (`--trust-mirror`, M1.6) keys off it. |

## Identity model

Per [PROP-002 §2.1](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#identity), package identity is the tuple `(kind, name, version, content_hash)`. `source_url` is **informational** — switching mirrors, migrating between hosts, or rotating an override target produces different `source_url` values for the same identity, and the lockfile is **not** churned on those changes. `vibe install` cross-source content drift detection lives at this exact boundary: when the lockfile pins a `content_hash` and a fresh fetch produces a different one for the same `(kind, name, version)`, install refuses with [`InstallError::ContentDrift`](../crates/vibe-install/src/lib.rs).

This is the structural property whose absence trapped Nix on GitHub: in Nix, `flake.lock` keys on URL + rev, so any change to the URL forces a lockfile rewrite. vibevm explicitly avoids that — the URL is a routing detail, not an identity, and the lockfile carries identity directly via `content_hash`.

## Field ordering and stability

The TOML serializer emits fields in declaration order — `[meta]` first with its own field order, then `[[package]]` blocks in install order. **`[[package]]` order is part of the lockfile contract.** Round-tripping through `read()` then `write()` preserves it; reading then re-writing without changes produces a byte-identical file (line-ending normalisation aside, which `.gitattributes` pins to LF).

`vibe install` of a new package appends; `vibe uninstall` removes the matching entry without renumbering; `vibe install` on a content-drift hit refuses entirely (no partial write). There is no entry sorting at write time — that would churn diffs unnecessarily.

## v1 → v2 migration

A v1 lockfile (M0 / M1.1 shape) parses cleanly under the v2 reader:

- Missing `[meta].schema_version` defaults to `2` (the current). The `Lockfile::looks_like_v1_on_disk()` heuristic detects v1 shape by inspecting whether the post-v1 fields are uniformly empty; it surfaces a one-shot UX nudge candidate but isn't wired into a user-visible warning today.
- Per-package `source` field is read into `source_url` via `#[serde(alias = "source")]`.
- Every other v2-only field (`registry`, `source_ref`, `resolved_commit`, `dependencies`, `overridden`) defaults to `None` / `[]` / `false`.
- `[meta].solver` and `[meta].root_dependencies` default to `None` / `[]`.

On the next `vibe install` / `vibe update` against a v1 lockfile, `register_installed` writes the v2 shape — `schema_version = 2`, `source_url` instead of `source`, populated provenance fields wherever the install path can produce them. v1 entries that aren't touched by the new install retain blank provenance until the package is re-resolved.

There is no manual migration command — migration is on the read path. To force a full migration of every entry, run `vibe registry sync && vibe install <every-existing-pkgref> --assume-yes` or, less surgically, `vibe uninstall --all && vibe install <every-pkgref>` (the former preserves bytes; the latter drops the cache).

## Tooling examples

Every `vibe.lock` is jq-friendly when piped through a TOML→JSON converter; below uses [`taplo`](https://taplo.tamasfe.dev) and `jaq`. (You can substitute any TOML/JSON tooling.)

```bash
# What's installed?
taplo get --output-format json vibe.lock | jq -r '.package[] | "\(.kind):\(.name)@\(.version)"'

# Find override-pinned packages.
taplo get --output-format json vibe.lock | jq '.package[] | select(.overridden == true)'

# Build a manifest of what to refresh ahead of an offline session.
taplo get --output-format json vibe.lock \
    | jq -r '.package[] | select(.registry != null) | "\(.registry)\t\(.kind)-\(.name)\t\(.source_ref)"'

# Sanity-check that no entry's source_url contains "anarchic/vibespecs"
# after live-migration to the per-package shape.
taplo get --output-format json vibe.lock \
    | jq -r '.package[] | select(.source_url | contains("anarchic/vibespecs")) | "\(.kind):\(.name)"'
```

For machine-to-machine consumption of `vibe list --json`, prefer the JTD schema at [`schemas/list_report.jtd.json`](../schemas/list_report.jtd.json) — same fields surface there with the same constraints.

## Worked example

A project that asked for two flows directly and pulled in one transitive dep:

```toml
[meta]
generated_by      = "vibe 0.2.0-dev"
generated_at      = "2026-04-25T12:34:56Z"
schema_version    = 2
solver            = "naive-1"
root_dependencies = ["flow:wal", "flow:atomic-commits"]

[[package]]
kind            = "flow"
name            = "wal"
version         = "0.1.0"
registry        = "vibespecs"
source_url      = "git@gitverse.ru:vibespecs/flow-wal.git"
source_ref      = "v0.1.0"
content_hash    = "sha256:7d8f…b1"
boot_snippet    = "10-flow-wal.md"
files_written   = [
    "spec/boot/10-flow-wal.md",
    "spec/flows/wal/WAL-PROTOCOL.md",
    "spec/flows/wal/morning-routine.md",
    "spec/flows/wal/session-end-hook.md",
]
dependencies    = ["flow:atomic-commits@=0.1.0"]

[[package]]
kind            = "flow"
name            = "atomic-commits"
version         = "0.1.0"
registry        = "vibespecs"
source_url      = "git@gitverse.ru:vibespecs/flow-atomic-commits.git"
source_ref      = "v0.1.0"
content_hash    = "sha256:1c4e…02"
boot_snippet    = "30-flow-atomic-commits.md"
files_written   = [
    "spec/boot/30-flow-atomic-commits.md",
    "spec/flows/atomic-commits/ATOMIC-COMMITS-PROTOCOL.md",
    "spec/flows/atomic-commits/conventional-commits.md",
    "spec/flows/atomic-commits/splitting-large-changes.md",
]
```

Reading from this:

- Both packages came from the same registry (`vibespecs`), so `vibe registry sync` will refresh both via one `MultiRegistryResolver` instance.
- `flow:wal` declared a transitive `flow:atomic-commits@^0.1`; the solver pinned it to exact `=0.1.0` (the only version available). Re-resolving against this lockfile picks the same version even if `vibespecs` later tags `v0.1.1`.
- `flow:atomic-commits` is **not** in `root_dependencies` — `vibe uninstall flow:atomic-commits` would refuse (it's a transitive, not a root). To remove it, `vibe uninstall flow:wal` first; future `vibe update --prune` will then orphan-collect `flow:atomic-commits` if no other root reaches it.
- Every `boot_snippet` matches a numeric prefix per [`VIBEVM-SPEC.md` §6.2](../VIBEVM-SPEC.md): `10-` and `30-` here, no clash.
- `content_hash` values are the identity. A re-fetch that produces a different hash for the same `(flow, wal, 0.1.0)` would trigger `InstallError::ContentDrift` on the next install of an already-locked entry.

## Changes since v1

For diff-readers landing on a project that just upgraded:

- `[meta]` gained `schema_version`, `solver`, `root_dependencies`.
- Each `[[package]]` gained `registry`, `source_ref`, `resolved_commit`, `dependencies`, `overridden`.
- `[[package]].source` was renamed to `[[package]].source_url`. Old key still parses via serde alias.
- The lockfile-content-hash invariant is enforced at plan time as `InstallError::ContentDrift` (was: silently accepted under v1 if the same `(kind, name, version)` was already present).

## Related

- [`VIBEVM-SPEC.md` §7.4](../VIBEVM-SPEC.md) — the spec-level lockfile schema.
- [`PROP-002 §2.7`](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#lockfile) — the design lock for v2 fields.
- [`PROP-002 §2.1`](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#identity) — the identity model that drives the `content_hash` field.
- [`crates/vibe-core/src/manifest/lockfile.rs`](../crates/vibe-core/src/manifest/lockfile.rs) — the Rust source of truth.
- [`schemas/list_report.jtd.json`](../schemas/list_report.jtd.json) — the JTD wire shape that surfaces these fields through `vibe list --json`.
