# PROP-007: Workspace — multi-package projects, recursive nesting, selective publish {#root}

**Milestone:** design proposal; targets a new `M1.17` ([`ROADMAP.md`](../../../ROADMAP.md)). Not implementation-locked.
**Status:** DRAFT 2026-05-20 — requirements locked in an owner design session; implementation pending.
**Related:** [`VIBEVM-SPEC.md` §4.2 / §7 / §8](../../../VIBEVM-SPEC.md); [PROP-002](../vibe-registry/PROP-002-decentralized-registry.md) (identity, registry, git-source, override); [PROP-008](../vibe-registry/PROP-008-qualified-naming.md) (qualified naming — companion document, same design session); [PROP-003 §2.5](../vibe-resolver/PROP-003-dep-evolution.md) (subskills — a *distinct* concept, see §4); [PROP-005](../vibe-index/PROP-005-package-index.md) (index).
**Owner sanction:** the owner granted (2026-05-20) explicit sanction to edit any specification — including the owner-frozen `VIBEVM-SPEC.md` — for this refactor. PROP-007 + PROP-008 are the requirements record; the `VIBEVM-SPEC.md` edits (§4.2 layout, §7.3–7.5 schemas) land at implementation time.

---

## 1. Motivation {#motivation}

vibevm today knows two manifest roles, carried by two different files:

- `vibe.toml` — the **consumer** manifest. Lives at the root of a project under development. Carries `[project]`, `[requires]`, `[[registry]]`, `[active]`, `[llm]`.
- `vibe-package.toml` — the **publishable artifact** manifest. Lives at the root of a package directory (what `vibe registry publish <path>` consumes, what a registry repo carries). Carries `[package]`, `[writes]`, `[provides]`, `[requires]`, `[obsoletes]`, `[conflicts]`.

There is no notion of a *project composed of several modules*. A project is one consumer; a package is one artifact; the two never compose. Publishing is `vibe registry publish <one-path>` — one package at a time, by hand.

The owner's request (design session 2026-05-20): the Maven-multi-module + cargo-workspace shape. A project should decompose naturally into modules; each module publishes independently — or not at all, by choice; the whole structure is declared in the project manifest. Both extremes must be first-class: a project entirely local (nothing ever published, the source tree never leaves the developer's machine) and a project entirely published (every sub-package and the root in registries).

Prior art: cargo `[workspace]` (`members = [...]`, one `Cargo.lock`, `cargo publish -p`), Maven multi-module (`<modules>`, reactor build, per-module `<skip>`).

PROP-007 covers the workspace axis. The companion [PROP-008](../vibe-registry/PROP-008-qualified-naming.md) covers qualified naming (`group`, short aliases, collision detection); the two were specified together and cross-reference each other but ship as separate milestones.

---

## 2. Decisions {#decisions}

### 2.1 The `[workspace]` section {#workspace-section}

**Decision.** A `vibe.toml` may carry a `[workspace]` table declaring member packages:

```toml
[workspace]
members = [
  "packages/flow-wal",
  "packages/feat-auth",
  "packages/stack-*",          # glob permitted
]
```

- `members` — paths relative to the manifest. Glob patterns are permitted (`packages/*`).
- Each member is a directory carrying its own `vibe.toml` (§2.2).
- Membership is **explicit** — there is no auto-discovery of directories that happen to carry a `vibe.toml`. The structure is declared, per the owner's "the whole structure is in the project description" requirement.

### 2.2 Unified manifest — one `vibe.toml` {#unified-manifest}

**Decision.** `vibe-package.toml` is **retired as a distinct filename**. Every node — project root, workspace member, published package — carries a single `vibe.toml`; the role is expressed by which sections are present. This is the cargo model: one `Cargo.toml` carries `[package]` and/or `[workspace]`.

Section roles:

| Section | Presence | Meaning |
|---|---|---|
| `[package]` | optional | The node is a publishable artifact (`kind`, `name`, `group`, `version`, …). |
| `[project]` | optional | The node is a non-publishable consumer/root. |
| `[workspace]` | optional | The node coordinates members (§2.1). |
| `[requires]`, `[[registry]]`, `[active]`, `[llm]` | optional | Consumer-side configuration. |

- `[package]` and `[project]` are **mutually exclusive** in one file — a node is either a publishable package or a plain project, not both. (Decision 7-α from the design session: keep the two sections distinct rather than folding `[project]` into a `[package]` with optional `kind`. Explicitness wins; `kind` stays strictly mandatory wherever `[package]` appears.)
- `[workspace]` composes with `[package]`, with `[project]`, or with neither (a virtual workspace root — just a coordinator).

**Why one file.** A workspace member is *simultaneously* a locally-developed node and a publishable artifact. Two files would give every member both, each carrying its own `[requires]` — duplication that drifts. One `vibe.toml` with a variable section set is the only coherent shape. The owner's secondary reason, recorded verbatim: reading one file is easier for a human — and for a small/weak LLM agent — than chasing many.

**Consequence.** Registry repositories migrate `vibe-package.toml` → `vibe.toml` (a published package is a `vibe.toml` with `[package]`, no `[workspace]`). The redirect-stub marker `vibe-redirect.toml` (PROP-002 §2.4.2) is a separate concern and is unaffected. Migration detail: [PROP-008 §3](../vibe-registry/PROP-008-qualified-naming.md#migration).

### 2.3 Recursive nesting {#nesting}

**Decision.** Nested workspaces are permitted to arbitrary depth — a member may itself carry a `[workspace]` section.

The load-bearing principle that keeps this from becoming chaos:

> Nesting is **hierarchical grouping**, not independent resolution domains. The lockfile and unified resolution always live at the *absolute root* of the workspace tree. A nested `[workspace]` provides (a) the `[workspace.versions]` matryoshka (§2.6) and (b) logical grouping of members — never its own lockfile, never its own resolution pass.

- **Root discovery.** A command run inside a node walks up the directory tree, collects every `vibe.toml` carrying `[workspace]`, and selects the *topmost one that transitively includes the current node*. The lockfile lives there.
- **Standalone node.** If no enclosing `[workspace]` exists above a node (it was cloned on its own — e.g. it is just a published package), the node is its own absolute root. This is the same rule as §2.4's command-bubbling and matches cargo's behaviour for a crate cloned outside any workspace.
- **Explicit nesting.** A parent `[workspace].members` lists the nested sub-workspace among its members. No nesting is inferred from the directory tree alone.

**Cost.** Cargo forbids nested workspaces precisely to avoid "which workspace is mine" ambiguity. vibevm permits them because the "lock always at the absolute root" rule resolves that ambiguity deterministically. The price is recursion in three places — parent-chain discovery, transitive member aggregation, and placeholder resolution (§2.6) — which the implementation estimate for M1.17 must absorb.

### 2.4 Single lockfile at the absolute root {#lockfile}

**Decision.** One `vibe.lock`, at the absolute root of the workspace tree (§2.3). No per-member lockfiles.

- **Unified resolution.** All members resolve together: one version of each external dependency across the whole workspace. A "diamond" inside a workspace is impossible by construction. This is the cargo model; Maven's nearest equivalent is `<dependencyManagement>` in the parent POM (Maven has no lockfile at all — a known reproducibility gap vibevm does not inherit, since the lockfile is already load-bearing for content-hash integrity per PROP-002 §2.1).
- **Command bubbling.** A command (`vibe install`, `vibe build`) run inside a member's directory walks up to the absolute root, finds `vibe.lock`, and operates against it. The member "does not notice" it is part of something larger — this realises the owner's requirement that a developer can work inside a sub-project unaware of the surrounding workspace.

### 2.5 Cross-member dependencies — the `path` source {#path-source}

**Decision.** A third dependency source-kind joins registry-resolved (PROP-002 §2.2) and git-source (PROP-002 §2.4.1): **path-source**.

```toml
[requires.packages]
"org.vibevm/wal" = { path = "../flow-wal" }
# dual-form (recommended for any member that is itself published):
"org.vibevm/wal" = { path = "../flow-wal", version = "^0.1" }
```

- **Dual-form.** `path` is used during local development inside the workspace; `version` takes effect when the consuming node is itself published — the published copy references `org.vibevm/wal@^0.1` from a registry, not `../flow-wal` (which an external consumer does not have). This is cargo's `{ path = ..., version = ... }` shape. Dual-form is **required** for any path-dep whose consumer is publishable.
- **Resolution priority.** `[[override]]` > path > git-source > registry-walk. Path sits below override (override is a deliberate patch) and above git-source (path is the most local, most authoritative declaration).
- **Lockfile.** New `source_kind = "path"`. For a workspace-member path-dep the lockfile records a reference to the member by id within the workspace, not an external `source_url` — so the lockfile stays portable across machines (an absolute path would not).
- **path outside the workspace.** A `path` pointing at a directory that is not a member of this workspace is permitted, but a node depending on it via path-only (no `version`) is not publishable — the published copy would dangle.

### 2.6 Version placeholders — `[workspace.versions]` {#versions}

**Decision.** Named version placeholders, the equivalent of Maven `<properties>`:

```toml
# in a [workspace] manifest:
[workspace.versions]
core = "0.0.1"
ui   = "^0.3"
```

```toml
# in a member:
[requires.packages]
"org.vibevm/auth" = { version.var = "core" }
```

- **Recursive resolution (matryoshka).** A `version.var = "core"` reference is resolved bottom-up: search `[workspace.versions]` of the node's nearest enclosing workspace, then its parent, then upward to the absolute root. First hit wins — a nearer level overrides a farther one. This is the arbitrary-depth nesting the owner asked for; it depends on §2.3 permitting nested workspaces.
- **Version inheritance.** A member may write `version = { workspace = true }` in `[package]` to inherit its own version from the nearest `[workspace]` — cargo's `version.workspace = true`. Independent per-member versions remain the default; inheritance is opt-in.
- A companion mechanism, `[workspace.dependencies]` (cargo-style centralised per-pkgref defaults, ≈ Maven `<dependencyManagement>`), is noted as a possible addition but **not** the primary surface — named placeholders were the owner's explicit request and cover the stated use case ("write `0.0.1` once, reference it by name everywhere").

### 2.7 Selective publish {#selective-publish}

**Decision.** Each publishable node declares its publish posture in `[package]`:

```toml
[package]
publish = false                 # never published — workspace-internal
# or
publish = true                  # default
# or
publish = ["vibespecs"]         # only into these named registries
```

- `vibe workspace publish [--member <m>]` walks members in **topological order** (dependency-first) and skips `publish = false`.
- Publish is **not atomic**: on the first failure the command stops and reports what was already published and what remains. (Distributed publishing across N independent host repos has no transaction; a rollback would be a worse lie than a clear partial-progress report.)
- Extremes: every member `publish = false` → the project is entirely invisible, nothing leaves the machine. Every member `publish = true` → the whole project, root included (§2.9), is published.

### 2.8 Published package repositories {#published-repos}

**Decision.** The development tree is **one** source tree (one git repository, or not in git at all if the project is private). Workspace members are subdirectories; the split into packages is logical, at the vibevm resolver level. **Publishing is a separate operation that copies the content of a package's directory into a new, separate repository** in the registry org and tags the version — exactly what `vibe registry publish` does today for one package, repeated per self-published member by `vibe workspace publish`.

```
DEVELOPMENT — one tree, one git repo (or no git):
  my-project/
  ├── vibe.toml             [workspace] members = ["packages/X", "packages/Y"]
  └── packages/
      ├── X/  vibe.toml     [package] org.vibevm/X, publish = true
      └── Y/  vibe.toml     [package] org.vibevm/Y, publish = true

PUBLISH (`vibe workspace publish`) — splits into separate repos:
  packages/X/  --content copy-->  <registry-org>/org.vibevm.X   tag v…
  packages/Y/  --content copy-->  <registry-org>/org.vibevm.Y   tag v…
  The development tree is NOT modified. It stays a monorepo.
```

A nested package does **not** "surface" by moving files — only a *copy of its content* is published, into its own repository, at publish time. The source tree stays unified.

**Terminology.** The published copy is a **published package repository**; the source of truth is the **workspace** (the development monorepo). This is *not* a `[[mirror]]` (PROP-002 §2.3 — that term means an availability copy of a registry).

**Origin marker.** The published copy carries a machine-readable marker in its `vibe.toml`:

```toml
[origin]
upstream     = "https://github.com/you/my-project"   # the monorepo
path         = "packages/flow-wal"                   # path within it
generated_by = "vibe 0.x"
generated_at = "2026-…"
```

**"Do not contribute here" signalling.** A published copy whose source of truth is a monorepo should tell humans not to send pull requests. GitHub offers no "disable PRs only" switch, so the signal is layered. `vibe workspace publish` default applies layers 1–4; `--archive` adds layer 5:

| Layer | Visibility | Cost |
|---|---|---|
| README banner as the first block (vibevm already generates such banners — `build_redirect_readme` for stubs) | seen immediately on opening the repo | free |
| repo `description` = "Generated copy of `<pkgref>` — contribute at `<upstream>`" | visible in the repo header | one API call at create |
| Issues disabled (`has_issues = false`) | Issues tab disappears | one API call |
| `.github/PULL_REQUEST_TEMPLATE.md` with a STOP notice | fires at PR-creation time | free |
| `archived = true` (`--archive`) | yellow "Public archive" banner, PR/issues/push all blocked | re-publish needs unarchive→push→archive; vibevm drives that cycle |

A `[workspace]`-level setting `published_repos = "read-only" | "open"` (default `"read-only"` for workspace members) lets an operator opt into the inverse model where the split repo *is* the canonical contribution target.

**Layout recommendation.** Keep members as siblings (flat under `packages/`), not physically nested. Logical hierarchy ("X is built from Y") is expressed by a path-dependency (§2.5), not by nesting directories. If a member *is* physically inside another's directory, publishing the outer package must excise the inner sub-package's subtree from the outer's content (cargo does this with nested crates) — supported, but discouraged for the "holes in the tree" complexity it adds.

### 2.9 Root as a publishable package {#root-package}

**Decision.** The root `vibe.toml` may itself carry `[package]` alongside `[workspace]` — cargo-style. The workspace coordinator can also be a publishable artifact in its own right. (Maven's parent POM cannot; cargo's root crate can. vibevm follows cargo.)

---

## 3. Command and crate surface {#surface}

- `vibe workspace publish [--member <m>] [--archive]` — topological publish of self-publishing members (§2.7), origin-marker + signalling (§2.8).
- `vibe install` / `vibe build` bubble up to the absolute root (§2.4); `-p <member>` targets one member; run inside a member's directory they address that member's `[requires]`.
- A new `vibe-workspace` crate, or workspace functions inside `vibe-core` — decided at implementation time.

`vibe.lock` schema bumps to **v4** (v3 was git-source `source_kind`, PROP-002 §2.4.1) to carry `source_kind = "path"` and the member-reference shape (§2.5).

---

## 4. Workspace members vs subskills {#vs-subskills}

These are easy to confuse; they are different objects.

| | Workspace member (PROP-007) | Subskill ([PROP-003 §2.5](../vibe-resolver/PROP-003-dep-evolution.md)) |
|---|---|---|
| What it is | A separate package | A sub-document *inside* one package |
| Versioning | Its own `version`; published independently | Versioned together with its parent package |
| Publication | Becomes its own repository (§2.8) | Never published separately |
| Identity | Own `(group, name, version, content_hash)` | No independent identity |

A workspace member is a package. A subskill is content granularity within a package. PROP-007 does not touch the subskill design.

---

## 5. Rejected / deferred alternatives {#rejected}

- **Per-member lockfiles.** Rejected. Independent resolution per member loses unified resolution and reintroduces intra-workspace diamonds. One lock at the absolute root (§2.4) is the cargo-proven shape.
- **Two files (`vibe.toml` + `vibe-package.toml`) side by side.** Rejected. A member needs both consumer and publishable roles; two files duplicate `[requires]` and drift. §2.2 unifies into one file.
- **Physical nesting of members by default.** Discouraged, not forbidden. §2.8 recommends a flat sibling layout; physical nesting is supported with subtree excision but adds avoidable complexity.
- **Atomic `vibe workspace publish`.** Deferred / rejected as infeasible — no transaction spans N independent host repos. §2.7 ships stop-on-first-failure with a clear partial-progress report instead.

---

## 6. Open questions {#open}

1. Exact field set of `[origin]` (§2.8) — `commit` of the source monorepo too, for full provenance?
2. `vibe.lock` v4 — the precise on-disk shape of a path-member reference (§2.5). To be pinned during implementation.
3. `vibe build` semantics inside a workspace (`-p`, whole-workspace) — depends on M1.5 landing first.
4. Whether `[workspace.dependencies]` (§2.6) ships alongside named placeholders or is deferred until a concrete need surfaces.

---

## 7. Phase plan {#phases}

PROP-007 (workspace) has **no dependency on the index** and can be implemented first. The companion [PROP-008](../vibe-registry/PROP-008-qualified-naming.md) (qualified naming) depends on [PROP-005](../vibe-index/PROP-005-package-index.md) being implemented for short-name resolution. Suggested order: PROP-007 → PROP-005 implementation → PROP-008. PROP-007 alone delivers multi-package projects, local cross-member deps, selective publish, and both "entirely local" / "entirely published" extremes — the bulk of the owner's request.

`VIBEVM-SPEC.md` edits (§4.2 directory layout, §7.3–7.5 manifest/lockfile schemas) land in the PROP-007 implementation milestone under the owner sanction recorded above.

---

## 8. Version history {#history}

- **2026-05-20 — draft 1.** Initial proposal. Requirements locked in an owner design session (decisions on workspace shape, recursive nesting, unified manifest, path-source, version placeholders, selective publish, published-repo signalling). Open for review.
