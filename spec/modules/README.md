# Module-level specs

Per-crate specifications (PROP / FEAT) land here as work progresses.
Foundation decisions that cross every crate live in
[`spec/common/`](../common/). Comparative research and threat-model
backgrounder documents live in [`spec/research/`](../research/).

## Index

- [`vibe-registry/`](vibe-registry/) — registry fetch, cache, resolve.
  - [PROP-001: Git-backed registry](vibe-registry/PROP-001-git-backend.md)
    — shell-out to `git` (not `libgit2`), `GitBackend` trait, cache
    layout, Windows UX.
  - [PROP-002: Decentralized per-package registry](vibe-registry/PROP-002-decentralized-registry.md)
    — per-package repos, `[[registry]]`/`[[mirror]]`/`[[override]]`,
    content-addressed identity, lockfile v2.
  - [PROP-008: Qualified package naming](vibe-registry/PROP-008-qualified-naming.md)
    — mandatory reverse-FQDN `group`, identity tuple
    `(group, name, version, content_hash)`, optional `kind` prefix,
    `naming = "fqdn"` repo names, index-backed short-name resolution,
    collision detection. **Status: DRAFT 2026-05-20.**
- [`vibe-resolver/`](vibe-resolver/) — dep solver, features, subskills.
  - [PROP-003: Dep-model evolution](vibe-resolver/PROP-003-dep-evolution.md)
    — SAT solver via libsolv (BSD-3-Clause), cargo-style features,
    vibevm-native subskills with context-based activation, BCP-47
    sidecar i18n, lockfile v3. **Status: design proposal.**
- [`vibe-index/`](vibe-index/) — optional per-org package index + standalone server.
  - [PROP-005: Optional package index](vibe-index/PROP-005-package-index.md)
    — per-org `<org>/index` git repo with cargo-sparse-style `by-name/`
    + DNF-style `repomd.json` manifest + JSONL primary; standalone
    `services/vibe-index/` utility (one binary, two modes — CLI + HTTP
    server); single-writer in-RAM with atomic on-disk persistence;
    full-and-incremental reindex; opt-in everywhere. **Status: draft 2026-05-06.**
- [`vibe-workspace/`](vibe-workspace/) — multi-package projects.
  - [PROP-007: Workspace](vibe-workspace/PROP-007-workspace.md)
    — `[workspace] members`, one unified `vibe.toml` (retires
    `vibe-package.toml`), recursive nesting, single lockfile at the
    absolute root, `path`-source cross-member deps, `[workspace.versions]`
    placeholders, selective publish, published-package-repo signalling.
    **Status: DRAFT 2026-05-20.**
