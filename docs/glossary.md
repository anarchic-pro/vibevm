# Glossary

Definitive vocabulary for the vibevm project. Spec-text, PROPs, code, docs, and commit messages all draw from this list — when a term appears with a specific meaning here, that's the meaning everywhere. If you see a synonym in the wild that isn't on this page, it's drift.

Where the canonical decision lives, the entry links to it.

---

### apply (install pipeline stage)

Stage 4 of the `vibe install` pipeline. After plan + confirm: writes files, updates the lockfile. Reverse-rollback on partial failure (best-effort). Spec: [`VIBEVM-SPEC.md` §5.6](../VIBEVM-SPEC.md).

### authoring

Writing a new package. Per-kind guides under [`docs/authoring-flow.md`](authoring-flow.md), [`authoring-feat.md`](authoring-feat.md), [`authoring-stack.md`](authoring-stack.md).

### boot snippet

A markdown file under `<project>/spec/boot/` named `<NN>-<topic>.md` that the AI agent reads at session start, in numeric-prefix order. `00-09` and `90-99` are user-owned; `10-89` are package-contributed. Spec: [`VIBEVM-SPEC.md` §6](../VIBEVM-SPEC.md).

### canonical URL (registry)

The `[[registry]].url` value verbatim, before mirror substitution. The cache bucket is keyed on `sha256(normalize(canonical_url))`, NOT on whichever mirror URL actually answered the fetch. Mirror swaps therefore don't invalidate the cache — see [PROP-002 §2.6](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#cache).

### capability

An abstract interface a package can `[provides]` and another package can `[requires]`. Syntax: `<namespace>:<name>[@<version>]`, e.g. `ui:landing-page-host@^0.1`, `db:any@>=1.0`. The depsolver matches consumer capabilities against producer capabilities at install time. See [`CapabilityRef`](../crates/vibe-core/src/capability_ref.rs) and [PROP-002 §2.9](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#capability).

### content_hash

`sha256:<hex>` over the deterministically-ordered file tree of a package. **The identity** half of `(kind, name, version, content_hash)`. Computed by [`vibe-registry::compute_content_hash`](../crates/vibe-registry/src/lib.rs); recorded per-`[[package]]` in `vibe.lock`. PROP-002 §2.1 makes content_hash the load-bearing identity field; URL is informational only.

### content drift

Mismatch between a `vibe.lock` entry's pinned `content_hash` and the freshly-fetched bytes' hash for the same `(kind, name, version)`. Surfaces as `InstallError::ContentDrift`; refused at plan time. Catches force-pushed tags, malicious mirrors, override-source rotations.

### `flow` (kind)

Discipline / process module. Specs read at session boot that govern *how the team works* (commit conventions, WAL protocol). Authoring: [`authoring-flow.md`](authoring-flow.md). Examples: `flow:wal`, `flow:atomic-commits`.

### `feat` (kind)

Functional feature. The *what* of a project, expressed as specification (purpose, behaviour rules, acceptance criteria). Stack-agnostic at authoring time. Authoring: [`authoring-feat.md`](authoring-feat.md). Examples (planned M1.5): `feat:welcome-page`, `feat:user-authentication`.

### `stack` (kind)

Language / framework target. The *how* a feat becomes real software. Authoring: [`authoring-stack.md`](authoring-stack.md). Examples (planned M1.5): `stack:rust-cli`, `stack:rust-axum`, `stack:typescript-next`.

### `tool` (kind)

Reserved for v2+. Not yet authorable.

### identity (of a package)

The tuple `(kind, name, version, content_hash)` per [PROP-002 §2.1](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#identity). Two installs with the same identity are *the same install* regardless of which URL served them — that's the property that makes mirrors transparent and host-migration cheap.

### kind

One of `flow`, `feat`, `stack`, `tool`. Closed enum; adding a fifth is a spec change. Defined in [`VIBEVM-SPEC.md` §4.1](../VIBEVM-SPEC.md).

### lockfile

`vibe.lock` at the project root. Records exactly what is installed, with full provenance (registry name, source kind, source URL, source ref, resolved commit, content hash, transitive deps, override flag). Schema v4 today. Reference: [`docs/lockfile-format.md`](lockfile-format.md).

### manifest

The TOML schema describing a vibevm node. Every node carries one `vibe.toml`; its role is set by which sections it carries — `[project]` (a consumer), `[package]` (a publishable artifact), `[workspace]` (a coordinator). The lockfile `vibe.lock` is the third TOML schema. Schemas: [`VIBEVM-SPEC.md` §7](../VIBEVM-SPEC.md), Rust source: [`crates/vibe-core/src/manifest/`](../crates/vibe-core/src/manifest).

### mirror

Transparent fallback URL for a registry. `[[mirror]] of = "<name>" url = "<alt>" priority = N` adds an alternative source for the named registry; `of = "*"` matches any. The lockfile records the *canonical* URL only — mirror identity does not leak to lockfile. Runtime fallback chain lands in M1.6 (Phase B); schema is wired today. [PROP-002 §2.3](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#mirror).

### mirror layout

Convention that every entry in `[writes].files` is *both* the source path inside the package and the target path inside the consumer project. No path mapping. The single exception: boot snippets carry an explicit `[boot_snippet].source` field. Pinned in [PROP-000 §13](../spec/common/PROP-000.md#package-layout).

### `naming` (registry naming convention)

Per-registry rule for mapping a `<kind>:<name>` pkgref to a per-package repo name under the registry's org URL. Three values:

- `kind-name` (default): `flow:wal` → `<org>/flow-wal.git`. Used by `vibespecs`.
- `name`: `flow:wal` → `<org>/wal.git`. Legal when names are globally unique across kinds in the registry.
- `kind/name`: `flow:wal` → `<org>/flow/wal.git`. Requires host support for nested repo paths.

A property of the registry, not a global CLI rule. [PROP-002 §2.2](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#registry-model).

### override

`[[override]]` entry in `vibe.toml`. Surgical pin that bypasses the registry layer entirely for a specific pkgref — `vibe install <pkgref>` resolves through the override's `source_url` / `ref` directly. Lockfile entry carries `overridden = true`. Use case: pinning a fork during an upstream PR, internal forks of public packages. [PROP-002 §2.4](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#override).

### pkgref (package reference)

A string `<kind>:<name>[@<version>]`. Variants:

- `flow:wal` — latest stable.
- `flow:wal@0.3.0` — exact version.
- `flow:wal@^0.3` — semver caret range.
- `flow:wal@>=0.2, <1.0` — compound semver constraint.

Type: [`PackageRef`](../crates/vibe-core/src/package_ref.rs). Defined in [`VIBEVM-SPEC.md` §7.1](../VIBEVM-SPEC.md).

### plan (install pipeline stage)

Stage 3 of `vibe install`. After resolve + fetch, before confirm. Computes the file-level diff: which files would be created, which boot snippet contributed, conflicts against already-installed packages or the user-owned-paths guard. Output: [`InstallPlan`](../crates/vibe-install/src/lib.rs).

### priority (registry / mirror)

The `[[registry]]` array order is priority order — first registry whose `GitPackageRegistry::resolve` succeeds wins. Within a registry, `[[mirror]]` entries try in `priority` ascending order before the canonical URL. PROP-002 §2.2 (registries), §2.3 (mirrors).

### PROP

Project Proposal — a binding architectural decision document. Lives under `spec/common/` (cross-cutting) or `spec/modules/<crate>/` (subsystem-specific). PROP-000 is the foundation; subsequent PROPs assume it. PROP-001 is the git-backend decision; PROP-002 is the decentralized-registry refactor. New PROPs require explicit owner approval.

### registry

A git-hosted organization URL with one repository per package underneath. Modern (per-package) form; the legacy single-repo monorepo form lives only in M1.1-shipping consumers until they migrate. [PROP-002 §2.2](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#registry-model).

### repo (per-package)

A git repository hosting one vibevm package. Under a registry's organization URL, named per the registry's `naming` convention. Versions are git tags (`v<semver>`); content lives at the repo root (no per-version subdirectory). [PROP-002 §2.5](../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#layout).

### resolve (install pipeline stage)

Stage 1 of `vibe install`. The depsolver expands user-typed roots into the full transitive graph; per-pkgref version pick happens against the configured registries / overrides. Output: [`ResolvedGraph`](../crates/vibe-resolver/src/lib.rs).

### root dependency

A package the user *directly* asked for, as opposed to a transitive dep the solver pulled in. `vibe.lock`'s `[meta].root_dependencies` records them. `vibe uninstall <root>` works; `vibe uninstall <transitive>` is rejected — transitives are managed by the solver, not by direct user action.

### `source_url`

URL the package's content was fetched from on the install that produced this lockfile entry. **Informational** — package identity does not depend on it. Mirror-switching, host-migration, and override pins all change `source_url` without changing identity.

### `source_ref`

Git ref the content was fetched at. Typically `v<version>` for per-package registries; the override's ref for `[[override]]`-resolved entries; `None` for non-git sources. Recorded per-`[[package]]` in `vibe.lock`.

### `transitive` (dep)

A dep the solver pulled in because some other dep declared it, not because the user typed it on the command line. Tracked by `LockedPackage.dependencies` (resolved exact-version pin) and **not** in `[meta].root_dependencies`.

### user-owned (file)

Files vibevm-managed commands NEVER write or remove: `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any `00-` or `90-` boot file. The boundary that separates "owned by the project" from "owned by vibevm tooling". Pinned at [`vibe-install::USER_OWNED_PATHS`](../crates/vibe-install/src/lib.rs).

### `[package]` (manifest table)

The table in a node's `vibe.toml` that marks it as a publishable artifact. Present at the root of every per-package repo and at every `<root>/<kind>/<name>/v<version>/` directory in a M0 / fixture-shape registry. A node carries `[package]` XOR `[project]`. Schema: [`VIBEVM-SPEC.md` §7.3](../VIBEVM-SPEC.md), Rust source: [`crates/vibe-core/src/manifest/package.rs`](../crates/vibe-core/src/manifest/package.rs).

### vibe.lock

The project lockfile. Schema v4 today; an older schema version is rejected, not migrated. Reference: [`docs/lockfile-format.md`](lockfile-format.md).

### vibe.toml

The single manifest file every vibevm node carries. Its role is set by which tables it carries: `[project]` (a non-publishable consumer) XOR `[package]` (a publishable artifact); `[workspace]` composes with either or neither. Other sections: `[active]`, `[llm]`, `[[registry]]`, `[[mirror]]`, `[[override]]`, `[requires.packages]`, `[origin]`. Schema: [`VIBEVM-SPEC.md` §7.5](../VIBEVM-SPEC.md), Rust source: [`crates/vibe-core/src/manifest/project.rs`](../crates/vibe-core/src/manifest/project.rs).

### `vibevm`

The project. The CLI binary it produces is `vibe`.

### WAL (Write-Ahead Log)

Two distinct meanings:

1. The **flow** package `flow:wal` — a discipline module (the "log every session's intent before committing" protocol).
2. The **file** `spec/WAL.md` at the root of every vibevm project — a checkpoint of current project state, rewritten each session, not appended. The structural counterpart of the flow.

Both come from book chapter 4 ("the discipline of writing what you intend before doing it"). The flow installs the protocol; the file holds the live state.

---

## Anti-vocabulary

Words used in adjacent ecosystems that we deliberately do **not** use, with their vibevm equivalents:

| Don't say | Say | Why |
| --- | --- | --- |
| "lifecycle" | the relevant install / build / sync stage | Maven-ism. Vibevm has graphs, not lifecycles. |
| "phase" | install stage / build node | Same — Maven baggage. |
| "goal" | task graph node | Same. |
| "plugin" | package | "Plugin" is a passable synonym for `package` in casual use; never use it for "code that contributes graph nodes" — that's a v2 contribution model that has its own name when it ships. |
| "module" | crate (Rust) / spec module (`spec/modules/<name>/`) | "Module" overloads. Use one of the two specific forms. |
| "vendor" (verb, for packages) | mirror, or `vibe vendor` (when M1.6 ships) | "Vendoring" connotes copy-into-tree; we use mirror for transparent alternative sources. |

Vocabulary lock pinned in [`spec/WAL.md`](../spec/WAL.md) §Constraints.

---

## Related

- [`VIBEVM-SPEC.md` §15`](../VIBEVM-SPEC.md) — the canonical glossary the spec ships with; this page expands on it.
- [`PROP-000`](../spec/common/PROP-000.md) — foundational decisions; many terms originate here.
- [`docs/architecture.md`](architecture.md) — for the bigger picture of how these terms relate.
