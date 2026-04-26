# vibevm — architecture

A contributor-facing tour of the workspace. For the canonical specification, read [`VIBEVM-SPEC.md`](../VIBEVM-SPEC.md); for the design decisions on each subsystem, read the PROP documents under [`spec/common/`](../spec/common/) and [`spec/modules/`](../spec/modules/). This document is the connective tissue: how the crates fit together, what the key traits are, and where each pipeline walks.

## The mental model

Three concepts together describe almost everything vibevm does:

1. **Package** — a `(kind, name, version)` triple plus a content tree. Identity is `(kind, name, version, content_hash)` ([PROP-002 §2.1](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#identity)). Four kinds: `flow`, `feat`, `stack`, `tool`. Authoring is documented per kind in [`authoring-{flow,feat,stack}.md`](README.md).

2. **Registry** — a hosting organization URL with one git repo per package underneath. `vibe.toml`'s `[[registry]]` array lists registries in priority order; `[[mirror]]` adds transparent fallbacks; `[[override]]` short-circuits the registry layer for specific packages. Detailed in [PROP-002](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md).

3. **Pipeline** — every CLI command is one of a few well-defined stages. `install` is the load-bearing one: **resolve → fetch → plan → confirm → apply → register**. `publish` is its mirror image on the maintainer side: **read → check-or-create-repo → stage → push → tag**.

Everything below is plumbing connecting these three concepts.

## Workspace crates

| Crate | What lives here | Depends on |
| --- | --- | --- |
| `vibe-core` | Manifest schemas (`PackageManifest`, `ProjectManifest`, `Lockfile`), package identity (`PackageRef`, `CapabilityRef`, `VersionSpec`, `PackageKind`), error types. The shared vocabulary every other crate speaks. | (no internal deps) |
| `vibe-graph` | Task-graph builder and sequential runner. Drives the `install` and (eventually M1.5) `build` workflows. | `vibe-core` |
| `vibe-registry` | Git operations behind the `GitBackend` trait (`ShellGit` impl). `LocalRegistry`, `GitRegistry` (legacy monorepo, retired), `GitPackageRegistry` (per-package, current), `MultiRegistryResolver`. The `Registry` trait + `CachedPackage` value type. | `vibe-core` |
| `vibe-resolver` | `DepProvider` / `DepSolver` traits. `NaiveDepSolver` (DFS, no backtracking) is today's impl; resolvo / libsolv slots reserved. `MultiRegistryProvider` and `LocalRegistryProvider` adapt the registry layer for the solver. | `vibe-core`, `vibe-registry` |
| `vibe-install` | `plan_install` / `apply_install` / `register_installed` / `unregister_installed`. The user-owned-paths guard, boot-snippet-prefix collision detection, content_hash integrity check. | `vibe-core`, `vibe-registry` |
| `vibe-publish` | `RepoCreator` trait + `GitVerseCreator` impl. `Publisher` orchestrator. `Token` (with debug/display redaction). Inline `git_publish` module for staging / push / tag. | `vibe-core`, `vibe-registry` |
| `vibe-llm` | LLM provider abstraction. Stubs today; M1.5 lights it up with Anthropic / OpenAI / OpenRouter / Ollama adapters. | `vibe-core` |
| `vibe-check` | Spec linter (`vibe check`). Stubs today; M1.3 implements the full §12 check list. | `vibe-core` |
| `vibe-wire` | JTD-codegen'd wire types. Empty placeholder until `jtd-codegen` is installed and `cargo xtask codegen` populates `src/generated/`. | `serde`, `serde_json` |
| `vibe-cli` | The `vibe` binary. clap argument parsing, command dispatch, output formatting, `InstallResolver` enum bridging `LocalRegistry` and `MultiRegistryResolver` paths. | almost everything |
| `xtask` | `cargo xtask codegen` / `check-codegen`. Dev-only; excluded from `default-members`. | `clap`, `anyhow` |

Dependency direction is strictly downward — no cycles. A change in `vibe-core` rebuilds the whole tree; a change in `vibe-cli` rebuilds only the CLI.

## Key traits

These are the abstraction seams. Each one was introduced so a future implementation can replace the current one without touching consumers — see [PROP-001 §2.2](../spec/modules/vibe-registry/PROP-001-git-backend.md#backend-trait) for the design pattern.

### `GitBackend` ([`vibe-registry::git_backend`](../crates/vibe-registry/src/git_backend))

Every git operation goes through this trait. Two implementations:

- **`ShellGit`** (current) — spawns the system `git` binary. Default-friendly on Windows because it picks up the user's existing SSH-agent identity and credential helper. Used in production.
- **`LibGit2` slot** — feature-gated for a future swap to `libgit2` if shell-out ever becomes the wrong choice. Not implemented today.

Methods:

- `bootstrap` / `update` — clone or fast-forward a repo.
- `list_tags` — `git ls-remote --tags`, deduped peeled-form. No clone.
- `fetch_file_at_ref` — `git archive` over the wire to read a single file from a tag without cloning. Used by `GitPackageRegistry::fetch_dep_manifest` so the resolver can read N candidate manifests with N HTTP round-trips, not N clones.

### `Registry` ([`vibe-registry`](../crates/vibe-registry/src/lib.rs))

`list_versions` / `resolve` / `fetch`. Three implementations:

- **`LocalRegistry`** — M0 local-directory layout (`<root>/<kind>/<name>/v<ver>/...`). Used by `--registry <path>` and the in-tree `fixtures/registry/` for hermetic e2e tests.
- **`GitRegistry`** (legacy) — clones one big monorepo, treats its working tree as a `LocalRegistry`. M1.1-shipping. Retired in favour of `GitPackageRegistry`; kept around until consumer lockfiles in v1 schema have all migrated.
- **`GitPackageRegistry`** (current) — one repo per package under an organization URL. Versions are git tags. Cache layout `<bucket>/packages/<kind>-<name>/clone/`.

### `MultiRegistryResolver` ([`vibe-registry::multi_registry_resolver`](../crates/vibe-registry/src/multi_registry_resolver.rs))

Sits on top of an ordered set of `GitPackageRegistry` instances and threads `[[mirror]]` and `[[override]]` resolution. `resolve(pkgref)` returns a `MultiResolution` with provenance (which registry served, source URL, source ref, override flag). `fetch(&MultiResolution)` materialises a `CachedPackage` with the full lockfile-v2 provenance fields filled. `refresh_lockfile_clones` drives `vibe registry sync`.

### `DepProvider` / `DepSolver` ([`vibe-resolver`](../crates/vibe-resolver/src/lib.rs))

`DepProvider` — what the solver needs from the registry layer (`resolve_version`, `fetch_manifest`). Implemented by `MultiRegistryProvider` (production) and `LocalRegistryProvider` (`--registry <path>` path). Test fakes implement it directly.

`DepSolver` — what the install pipeline calls (`solve(roots) -> ResolvedGraph`). `NaiveDepSolver` is today's impl; resolvo / libsolv slots reserved per [PROP-002 §2.8](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#solver).

### `RepoCreator` ([`vibe-publish`](../crates/vibe-publish/src/lib.rs))

Host-specific operations for the publish flow: `host_name`, `repo_exists`, `create_repo`. One impl per supported git host. Today: `GitVerseCreator` (Gitea-compatible REST API). GitHub / Gitea / Forgejo land as adopters request them, each as one new file in `vibe-publish`.

## Pipelines

### `vibe install <pkgref> [<pkgref> ...]`

```
[CLI parse]
    │
    ▼
[InstallResolver::solve(roots)]                 ← vibe-resolver::NaiveDepSolver
    │  via MultiRegistryProvider (production)
    │  or LocalRegistryProvider (--registry <path>)
    ▼
[ResolvedGraph]                                 ← roots first, transitive after
    │
    ▼
for each node in graph:
    [InstallResolver::resolve_and_fetch(pkgref, cache_root)]
    │   - resolve via [[override]] short-circuit OR [[registry]] priority walk
    │   - fetch via GitPackageRegistry::fetch (clone-if-absent / update-if-present)
    │   - copy worktree into project cache, strip .git/
    │   - compute content_hash
    ▼
    [CachedPackage]                              ← provenance: registry_name, source_url, source_ref, overridden
    │
    ▼
    [vibe-install::plan_install]
    │   - lockfile content_hash integrity check  ← PROP-002 §2.1
    │   - regular writes vs. user-owned-paths guard
    │   - boot snippet name + numeric prefix collision detection
    │   - cross-plan target conflict detection
    ▼
    [InstallPlan]
    │
    ▼
[present plans, ask user (or --assume-yes / --json)]
    │
    ▼
for each plan:
    [vibe-install::apply_install]                ← writes files, no lockfile mutation
    │
    ▼
    [vibe-install::register_installed]
        - LockedPackage built with full v2 provenance
        - lockfile.meta.root_dependencies merged
    │
    ▼
[lockfile.write]
    │
    ▼
[CLI report — human / JSON / quiet]
```

### `vibe registry publish <path>`

```
[CLI parse]                                     ← vibe registry publish ./fixtures/registry/flow/wal/v0.1.0
    │
    ▼
[load token]                                    ← env > ~/.vibevm/git.publish.token
    │
    ▼
[GitVerseCreator::new(token)]                   ← reqwest blocking + rustls
    │
    ▼
[Publisher::publish(config)]
    │
    ├─ [PackageManifest::read]                  ← legacy [dependencies] migrates inline
    │
    ├─ [extract_org_segment(org_url)]           ← strips git+ prefix, ssh shorthand, scheme
    │
    ├─ [creator.repo_exists(org, repo)]
    │     ├─ exists → reuse
    │     └─ missing → [creator.create_repo]    ← POST /api/v1/orgs/<org>/repos
    │
    └─ [git_publish::push_release]
          ├─ temp working tree
          ├─ copy contents (skip .git/)
          ├─ git init --initial-branch=main
          ├─ commit "Release <name>@<version>"
          ├─ git tag -a v<version>
          ├─ git push -u origin main             ← classified errors → PushDenied / HostUnreachable
          └─ git push origin <tag>               ← classified errors → TagCollision / etc.
```

### `vibe registry sync`

```
[CLI parse]
    │
    ▼
[load Lockfile + ProjectManifest]
    │
    ▼
[MultiRegistryResolver::open(registries, mirrors, overrides)]
    │
    ▼
[MultiRegistryResolver::refresh_lockfile_clones(&lockfile)]
    │
    ▼
for each lockfile entry:
    │
    ├─ entry.overridden = true
    │     → ensure_clone_at(__overrides__/<kind>-<name>/clone)
    │
    ├─ entry.registry = Some(name)
    │     → registry = registry_by_name(name)
    │     → registry.refresh_package(kind, name, source_ref)
    │
    └─ otherwise (legacy / local)
          → SkippedEntry { reason }
    │
    ▼
[RefreshReport]                                 ← Vec<RefreshedEntry> + Vec<SkippedEntry>
    │
    ▼
[CLI report]
```

## Wire formats

| Format | Where |
| --- | --- |
| **TOML** for human-edited configs: [`vibe.toml`](../VIBEVM-SPEC.md), [`vibe.lock`](../VIBEVM-SPEC.md), [`vibe-package.toml`](../VIBEVM-SPEC.md). Schemas in `VIBEVM-SPEC.md` §7; serde-driven via `vibe-core::manifest`. |
| **JTD** for machine-to-machine wire contracts: every CLI `--json` output, every HTTP API request/response, future LLM provider wrappers, future telemetry. Schemas committed under [`schemas/`](../schemas/); generated Rust under `crates/vibe-wire/src/generated/` once `jtd-codegen` is installed. |

The split is deliberate per [PROP-000 §16](../spec/common/PROP-000.md#jtd) — TOML for humans, JTD for machines.

## Cache layout

Per-user, under `~/.vibe/registries/` (override via `VIBE_REGISTRY_CACHE`):

```
~/.vibe/registries/
└── <canonical-url-hash>/
    ├── meta.toml                              # canonical URL, last-mirror-used, last_synced_at
    └── packages/
        └── <kind>-<name>/
            ├── clone/                         # per-package git working tree
            └── meta.toml                      # source_url_last_used, last_synced_at, last_known_tags
```

Per-project, under `<project>/.vibe/cache/`:

```
<project>/.vibe/cache/
└── <kind>/
    └── <name>/
        └── v<version>/                        # materialised package contents (no .git)
            ├── vibe-package.toml
            └── …
```

The per-user cache is keyed on **canonical** registry URL — a `[[mirror]]` doesn't invalidate the cache, the project just gets to use a different mirror's bytes that hash to the same content_hash.

The per-project cache is the lockfile's mirror — every entry there has a corresponding `<kind>/<name>/v<version>/` payload. `vibe uninstall` does NOT purge per-project cache; reinstalling is one round trip cheaper that way.

## File-tree quick reference

| Looking for… | …go to |
| --- | --- |
| The CLI's flag table | [`crates/vibe-cli/src/cli.rs`](../crates/vibe-cli/src/cli.rs) |
| Manifest schemas | [`crates/vibe-core/src/manifest/`](../crates/vibe-core/src/manifest) |
| Git ops | [`crates/vibe-registry/src/git_backend/`](../crates/vibe-registry/src/git_backend) |
| Per-package registry | [`crates/vibe-registry/src/git_package_registry.rs`](../crates/vibe-registry/src/git_package_registry.rs) |
| Multi-registry resolver | [`crates/vibe-registry/src/multi_registry_resolver.rs`](../crates/vibe-registry/src/multi_registry_resolver.rs) |
| Solver | [`crates/vibe-resolver/src/naive.rs`](../crates/vibe-resolver/src/naive.rs) |
| Install pipeline | [`crates/vibe-install/src/lib.rs`](../crates/vibe-install/src/lib.rs) |
| Publish pipeline | [`crates/vibe-publish/src/lib.rs`](../crates/vibe-publish/src/lib.rs) |
| GitVerse adapter | [`crates/vibe-publish/src/gitverse.rs`](../crates/vibe-publish/src/gitverse.rs) |
| JTD schemas | [`schemas/`](../schemas) |
| xtask | [`xtask/src/main.rs`](../xtask/src/main.rs) |

## Reading order for a new contributor

1. [`README.md`](../README.md) at repo root — what is this, status, quick start.
2. [`CLAUDE.md`](../CLAUDE.md) — the four non-negotiable rules. Read before your first commit.
3. [`spec/boot/00-core.md`](../spec/boot/00-core.md) and [`90-user.md`](../spec/boot/90-user.md) — project boot snippets.
4. [`VIBEVM-SPEC.md`](../VIBEVM-SPEC.md) §1–§4 — what vibevm is, the package model.
5. This document.
6. [`spec/WAL.md`](../spec/WAL.md) — current state.
7. [`TASKS.md`](../TASKS.md) — what's queued.
8. [`spec/common/PROP-000.md`](../spec/common/PROP-000.md) — foundational decisions.
9. The PROP for whichever subsystem you're touching.
