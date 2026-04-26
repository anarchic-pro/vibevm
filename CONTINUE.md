# CONTINUE — cold-resume checkpoint

_Written: 2026-04-26. Owner-readable, self-contained, deliberately verbose. Pick this up with zero prior context and you should be able to continue without asking questions._

---

## TL;DR (executive summary)

We are mid-way through **M1.1-revision Phase A** — the decentralized per-package registry refactor. The full design (PROP-002) is locked, every code slice is shipped on `origin/main`, the workspace builds clean (`cargo test --workspace` ≈ 169 tests green, `cargo clippy --workspace --all-targets -- -D warnings` clean). The **only** outstanding Phase A item is the **live migration** of three v0.1.0 demo flows from the legacy monorepo `anarchic/vibespecs` into per-package repos `vibespecs/flow-wal`, `vibespecs/flow-sync-from-code`, `vibespecs/flow-atomic-commits` via the new `vibe registry publish` utility.

The migration is currently **blocked on a single human step**: GitVerse's public REST API does not expose an org-scoped repo creation endpoint (`POST /orgs/{org}/repos` returns 404 on the live host even though that is the Gitea-canonical shape). The workaround is to **manually create the three empty repos via the GitVerse web UI** (no auto-init), after which `vibe registry publish` will skip the create-leg via its `repo_exists()` check and proceed straight to push + tag.

The most recent commit is `36cbf08 feat(vibe-publish): correct GitVerse API surface from live probing` — base URL → `api.gitverse.ru`, auth → Bearer, Accept → versioned `application/vnd.gitverse.object+json;version=1`, dry-run UX bug fixed. **That commit is one ahead of `origin/main` and needs pushing** (will be pushed together with this CONTINUE.md write-up).

Once the three repos exist on GitVerse, the procedure to close Phase A is mechanical and is documented in detail in section "Live migration — exact steps" below.

---

## Where we are right now

- **Branch:** `main`, ahead of `origin/main` by 1 commit (`36cbf08`). Working tree clean.
- **Latest checkpoint:** GitVerse API discovery + `gitverse.rs` correction landed (commit `36cbf08`). Phase A code complete.
- **Workspace health:** 12 crates, ~169 tests green, clippy clean with `-D warnings`. Last full run before this checkpoint.
- **Next operation:** **wait for owner to manually create three repos on GitVerse** (see blocker), then run `vibe registry publish` for each. Non-routine per CLAUDE.md Rule 4 — needs explicit sign-off before push.

---

## The current blocker — manual repo creation on GitVerse

**Symptom.** Calling `POST /orgs/vibespecs/repos` (Gitea-canonical org-scoped repo creation) against `https://api.gitverse.ru` returns either 404 (with no `gitverse-api-version` response header → not a real route) or a WAF 403, depending on the request shape. The GitVerse public-API documentation index lists only `POST /user/repos` for repo creation; org-scoped creation is not exposed.

**Why we don't auto-fall-back to `/user/repos`.** Repos created via `/user/repos` belong to the authenticating *user*, not to the `vibespecs` org. There is no documented API to *transfer* a repo from a user namespace into an org namespace on GitVerse. Burning a user-namespace repo and re-creating in the org via web UI later is a worse UX than having the operator create the org repo directly on the web.

**Workaround.** Manual pre-create on the GitVerse web UI:

1. Log in to GitVerse as a member of the `vibespecs` org.
2. Navigate to `https://gitverse.ru/vibespecs` (or whatever the org page URL is).
3. Click "New repository". Create three repos, **all empty** (no auto-init, no README, no .gitignore — anything pre-populated will conflict with the publish utility's first push):
   - `flow-wal`
   - `flow-sync-from-code`
   - `flow-atomic-commits`
4. Visibility: public (the org is the public package registry).
5. Default branch: `main`.

After step 4 finishes, `vibe registry publish` will work end-to-end: its `repo_exists()` call hits `GET /repos/vibespecs/<repo>` and returns 200, the publisher skips the create-leg, and proceeds to `git init` (in a temp dir) → `git add .` → `git commit` → `git remote add origin <ssh-url>` → `git push -u origin main` → `git tag v0.1.0` → `git push origin v0.1.0`.

**If a future GitVerse release exposes org-scoped repo creation:** the place to fix is `crates/vibe-publish/src/gitverse.rs::create_repo` — the request shape there is already Gitea-canonical (`POST /orgs/{org}/repos` with a `CreateRepoBody` JSON body). When GitVerse adds the route, this code starts working transparently with no change.

---

## Live migration — exact steps (after blocker clears)

Run these from the repo root after the three GitVerse repos exist.

```bash
# 1. Confirm token.
test -f ~/.vibevm/git.publish.token && echo "token present"

# 2. Build a fresh release binary.
cargo build --release --workspace

# 3. Dry-run all three.
./target/release/vibe registry publish fixtures/registry/flow/wal/v0.1.0 --dry-run
./target/release/vibe registry publish fixtures/registry/flow/sync-from-code/v0.1.0 --dry-run
./target/release/vibe registry publish fixtures/registry/flow/atomic-commits/v0.1.0 --dry-run

# Expected per dry-run: "Would reuse existing repository `flow-<name>` on `gitverse.ru`"
# (because the human pre-created them). Then "would push to <ssh-url> and tag v0.1.0".

# 4. Apply (drops --dry-run).
./target/release/vibe registry publish fixtures/registry/flow/wal/v0.1.0
./target/release/vibe registry publish fixtures/registry/flow/sync-from-code/v0.1.0
./target/release/vibe registry publish fixtures/registry/flow/atomic-commits/v0.1.0

# 5. Walk the per-package smoke.
$EDITOR manual-tests/M1.5-gate-v2-per-package-smoke.md
# Follow the protocol; fill in "Last known pass" line on success.

# 6. Rotate DEFAULT_REGISTRY_URL.
$EDITOR crates/vibe-core/src/manifest/project.rs
# Change line ~284:
#   pub const DEFAULT_REGISTRY_URL: &str = "git@gitverse.ru:anarchic/vibespecs.git";
# to:
#   pub const DEFAULT_REGISTRY_URL: &str = "git@gitverse.ru:vibespecs";
# (ORG ROOT, not a package repo URL — the per-package URL is derived
# at fetch time via NamingConvention.)

# 7. Update tests that hard-code the old default and re-run.
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# 8. Commit the rotation as ONE commit (Rule 3 — one logical unit).
#    feat(core): rotate DEFAULT_REGISTRY_URL to vibespecs org root
#
# 9. Checkpoint Phase A complete:
#    - update spec/WAL.md (Phase A done section)
#    - update ROADMAP.md (M1.1-revision → done; M1.6 active)
#    - update TASKS.md (close out the live-packages line)
#    - one commit: docs(wal,roadmap,tasks): Phase A complete
#
# 10. git push.
```

If any step fails, **do not auto-rollback** — `vibe registry publish` is idempotent on its presence check (re-running picks up an existing repo and re-pushes), but a partial migration where one repo is published and two are not is a safe state to investigate from.

---

## GitVerse API discovery — findings recorded for posterity

These findings are baked into `crates/vibe-publish/src/gitverse.rs` (commit `36cbf08`). Reproducing them here so the next session doesn't re-walk the rabbit hole.

| Field | Wrong (initial guess) | Correct (live-verified) |
| --- | --- | --- |
| **Base URL** | `https://gitverse.ru/api/v1` | `https://api.gitverse.ru` |
| **Auth scheme** | `Authorization: token <T>` (Gitea legacy) | `Authorization: Bearer <T>` |
| **Accept header** | `application/json` | `application/vnd.gitverse.object+json;version=1` |
| **Versioning** | URL path (`/v1/`) | Accept header `;version=1` suffix |
| **Org-scoped repo creation** | `POST /orgs/{org}/repos` (Gitea-canonical) | **Not exposed.** Only `POST /user/repos` documented. |

**Probed shapes that returned WAF 403 / HTML 404 / no version header:**

- `POST gitverse.ru/api/v1/orgs/{org}/repos` — 404 from Next.js frontend (the `gitverse.ru` hostname is a SPA; the API lives on `api.gitverse.ru`).
- `POST api.gitverse.ru/orgs/{org}/repos` with `Authorization: token <T>` — 401 universally.
- `POST api.gitverse.ru/orgs/{org}/repos` with `Bearer` and `Accept: application/json` — 400 with empty body, response header `gitverse-api-latest-version: 1` (server hint to ask for `;version=1` in Accept).
- `POST api.gitverse.ru/orgs/{org}/repos` with correct headers — 404 with WAF response, no `gitverse-api-version` header in response → endpoint not present, not just unauthorized.

**Endpoints that DO work** (and are used by `gitverse.rs` today):

- `GET /repos/{owner}/{repo}` — repo presence check. 200 = exists, 404 = absent, 401/403 = auth issue. Works against an org as `{owner}` (i.e. `GET /repos/vibespecs/flow-wal`).

**Authoritative source of these findings:** the GitVerse public-API docs page at `https://gitverse.ru/docs/public-api/` (read 2026-04-26) and live curl probing of `https://api.gitverse.ru` from this machine.

---

## Repository map

```
vibevm/                                 (this repo)
├── CLAUDE.md / AGENTS.md / GEMINI.md   ← byte-identical, 4 rules + Memory discipline (kept in lockstep)
├── CONTINUE.md                         ← THIS FILE
├── CHANGELOG.md
├── DEV-GUIDE.md / RUNTIME-GUIDE.md     ← contributor / end-user setup docs
├── LICENSE.md                          ← proprietary placeholder, target UPL 1.0
├── MEMORY.md                           ← pointer to spec/boot/90-user.md
├── README.md
├── ROADMAP.md
├── TASKS.md                            ← active work checklist for current slice
├── VIBEVM-SPEC.md                      ← owner-frozen v1.0 spec (do not edit without sign-off)
│
├── crates/                             ← Rust workspace (12 crates)
│   ├── vibe-core/                      ← manifest types, lockfile, content_hash, capability refs, errors
│   │   └── src/manifest/project.rs::DEFAULT_REGISTRY_URL  ← line ~284, rotates after smoke passes
│   ├── vibe-cli/                       ← `vibe` binary; `commands/{init,install,list,uninstall,registry,version}.rs`
│   ├── vibe-registry/                  ← Registry trait, ShellGit/GitBackend, GitPackageRegistry, MultiRegistryResolver
│   ├── vibe-resolver/                  ← DepSolver / DepProvider traits, NaiveDepSolver (DFS), Multi/LocalRegistryProvider adapters
│   ├── vibe-install/                   ← install plan/apply/register
│   ├── vibe-publish/                   ← RepoCreator trait, GitVerseCreator, Publisher, Token (redacted)
│   │   └── src/gitverse.rs             ← API constants documented inline; first-touch file when GitVerse changes
│   ├── vibe-wire/                      ← JTD-codegen target; src/generated/ populated by `cargo xtask codegen`
│   ├── vibe-graph/                     ← (M0 placeholder; not active)
│   ├── vibe-llm/                       ← (M0 placeholder; not active)
│   └── vibe-check/                     ← (M0 placeholder; not active)
│
├── docs/                               ← user / contributor documentation
│   ├── README.md                       ← index over commands/ + authoring guides
│   ├── architecture.md                 ← contributor tour (which crate does what, traits, pipelines)
│   ├── lockfile-format.md              ← exhaustive vibe.lock v2 reference
│   ├── troubleshooting.md              ← first-aid for every error variant
│   ├── glossary.md                     ← term lookup + anti-vocabulary
│   ├── authoring-flow.md               ← per-kind authoring guides (flow / feat / stack)
│   ├── authoring-feat.md
│   ├── authoring-stack.md
│   └── commands/                       ← one reference per shipped subcommand
│       ├── init.md / install.md / list.md / uninstall.md / version.md
│       └── registry-sync.md / registry-publish.md
│
├── manual-tests/                       ← runnable smoke protocols, one .md per scenario
│   ├── M1.1-git-registry-smoke.md
│   ├── M1.5-gate-multi-package-smoke.md
│   └── M1.5-gate-v2-per-package-smoke.md   ← THE smoke that closes Phase A
│
├── fixtures/                           ← test fixtures
│   └── registry/                       ← package fixtures (relocated from packages/ — that path now reserved for dogfooding)
│       └── flow/{wal,sync-from-code,atomic-commits}/v0.1.0/   ← migration source content
│
├── schemas/                            ← JTD wire-contract schemas (7 files)
│   ├── init_report.jtd.json
│   ├── install_plan.jtd.json
│   ├── install_report.jtd.json
│   ├── list_report.jtd.json
│   ├── registry_publish_report.jtd.json
│   ├── registry_sync_report.jtd.json
│   └── uninstall_report.jtd.json
│
├── spec/                               ← project specification (PROP / FEAT documents)
│   ├── WAL.md                          ← project continuation state
│   ├── boot/                           ← session-boot snippets, read in filename order
│   │   ├── 00-core.md (user-owned) … 90-user.md (user-owned)
│   ├── common/
│   │   └── PROP-000-foundation.md      ← project rules, conventions, §15-§19 = guiding principles
│   └── modules/
│       ├── vibe-registry/
│       │   ├── PROP-001-git-backend.md  ← partially superseded; ShellGit/GitBackend authoritative
│       │   └── PROP-002-decentralized-registry.md  ← THE design lock for current refactor
│       └── …
│
├── tools/
│   └── jtd-codegen/                    ← README pins jtd-codegen 0.4.1 install procedure (binary not committed)
│
├── refs/                               ← .gitignore'd; reference reading material
│   └── book/ + cloned reference repos
│
├── xtask/                              ← `cargo xtask codegen` / `check-codegen`
└── .cargo/config.toml                  ← `xtask` alias
```

---

## Important decisions — long-form list

These are the load-bearing architectural and policy decisions made or restated during this conversation. Each is "settled" — don't unpick without owner discussion.

1. **Decentralized per-package registry** (PROP-002). One git repo per package, default naming `<kind>-<name>` under an org, versions are git tags. No monorepo registry. Avoids Nix-style host vendor lock-in (Nix → GitHub) at the design layer.

2. **Identity is content-hashed, URLs are informational.** A package's identity tuple is `(kind, name, version, content_hash)`. The `source_url` recorded in the lockfile is for human debuggability and tooling, not for resolution. Content hash is verified on every fetch. Mirror-switching, host-migration, repo-rename never churn the lockfile.

3. **`[[registry]]` array in `vibe.toml`** — never a singleton. Backed by serde alias on the v1 form. Priority-ordered. `[[mirror]]` and `[[override]]` are siblings, not nested. Schema is fully shipped in Phase A; runtime mirror dispatch lands in Phase B (M1.6).

4. **Lockfile schema v2** carries full provenance: `registry`, `source_url`, `source_ref`, `resolved_commit`, `content_hash`, `dependencies`, `overridden` per package; `[meta]` carries `schema_version`, `solver`, `root_dependencies`. v1 lockfiles auto-migrate on next write via serde aliases + defaulted fields. No flag day.

5. **Capability-based deps from day one**, not shoehorned in later. Manifests use `[provides]` / `[requires]` / `[[requires_any]]` / `[obsoletes]` / `[conflicts]` — all *semantic*, all *enforced* by the resolver. Legacy `[dependencies]` compact form migrates transparently. PROP-000 §18 sets the bar at "complexity ≥ RPM".

6. **Three guiding principles** landed in PROP-000 §15-§19:
   - §15 **Dependency weight is not a decision factor** — pick best-in-class library, reject only on license / abandonment / security / bad API.
   - §16 **JTD + codegen by default** for wire contracts (CLI `--json` events, API clients, future LLM provider wrappers).
   - §17 **Production architecture in the prototype phase** — Google-principal-engineer lens. Load-bearing surfaces ship production-quality (lockfile, registry protocol, dep-resolver, wire formats).
   - §18 **Complexity ≥ RPM** for the dep model from day one (capability-based, virtual-package-aware, disjunctions).
   - §19 **Load-bearing setup docs** (`DEV-GUIDE.md` / `RUNTIME-GUIDE.md`) at repo root; any toolchain / prereqs / env / paths change updates them in the same commit.

7. **Memory discipline** (CLAUDE.md / AGENTS.md / GEMINI.md "Memory discipline" section). Project facts (design, conventions, decisions, milestones, owner preferences governing technology choices) live **inside this repository**. Tool-specific global per-user auto-memory holds **only machine-local facts** (shell quirks, SSH-agent setup on this box). Default when uncertain: write to repo, not to global.

8. **`DepSolver` trait + first impl `NaiveDepSolver`.** DFS, no backtracking — covers today's all-empty-deps fixtures and any first-cut realistic graph. `resolvo` (BSD-3-Clause, Pixi/Rattler-scale) and `libsolv` slots reserved behind the same trait. Switching solvers is a one-impl change, not a rewrite.

9. **`RepoCreator` trait + first impl `GitVerseCreator`.** Future GitHub / Gitea / Forgejo adapters land behind the same trait. `Publisher` orchestrator does the version-aware push + tag work and is host-agnostic.

10. **`GitBackend` trait + `ShellGit` shell-out.** No in-process libgit2 / gitoxide dependency for now (PROP-001 §2.1 — size argument pruned per PROP-000 §15; Windows SSH-auth and diagnostic clarity still carry the call).

11. **Token redaction.** `vibe_publish::Token::Display` / `Debug` print `<redacted>`; bare `value()` accessor is the single de-redacted path. CLI prints token *source* (explicit / env / file path) but never the value.

12. **GitVerse public-API surface, live-verified.** Base `https://api.gitverse.ru`, `Authorization: Bearer <T>`, `Accept: application/vnd.gitverse.object+json;version=1`. Endpoints: `GET /repos/{org}/{repo}` works; `POST /orgs/{org}/repos` documented at Gitea but not exposed by GitVerse (workaround: manual web-UI create). Documented inline in `gitverse.rs` and in this CONTINUE.md.

13. **Cache layout** per PROP-002 §2.6: `~/.vibe/registries/<canonical-url-hash>/packages/<kind>-<name>/clone/` for registry-served entries, `<hash>/__overrides__/<kind>-<name>/clone/` for override-served. `VIBE_REGISTRY_CACHE` env-var overrides root.

14. **Manual-test protocol.** Runnable smoke-tests in `manual-tests/`, one file per scenario, clean-slate setup + teardown. PROP-000 §14. The Phase A close-out is gated by `M1.5-gate-v2-per-package-smoke.md` passing.

15. **Conventional Commits + group by meaning + human-only attribution + autonomy on routine only.** Rules 1-4 in CLAUDE.md / AGENTS.md / GEMINI.md, authoritative in PROP-000 §12.

16. **Default registry URL** lives at `vibe_core::manifest::project::DEFAULT_REGISTRY_URL` — single source of truth. Currently `git@gitverse.ru:anarchic/vibespecs.git` (legacy monorepo). Rotates to `git@gitverse.ru:vibespecs` (org root) after Phase A smoke passes. **NB:** the rotation target is the *org root*, not a per-package URL — per-package URLs are derived at fetch time via `NamingConvention`.

17. **JTD toolchain is scaffolded but not yet driving any consumer.** Schemas land documentation-quality first; structs migrate to JTD-derived types incrementally as consumers are touched. Manual install of `jtd-codegen` 0.4.1 per `tools/jtd-codegen/README.md` before `cargo xtask codegen` is run.

18. **Linguistic vocabulary lock.** Only `flow`, `feat`, `stack`, `tool` for package kinds. Never `lifecycle`, `phase`, `goal`, `plugin` (except as passing synonym for `package`). Anti-vocabulary catalogued in `docs/glossary.md`.

19. **Live migration is non-routine** per CLAUDE.md Rule 4 — it creates real public artefacts in a public org and was the first GitVerse-API exercise. Owner sign-off required before any push.

20. **Legacy registry stays read-only.** `git@gitverse.ru:anarchic/vibespecs.git` (HEAD `2203239`, 2026-04-23, three v0.1.0 flows) keeps existing for projects still on schema-v1 lockfiles. No new publishes there.

---

## Recent commit chain (last 25, most-recent first)

```
36cbf08 feat(vibe-publish): correct GitVerse API surface from live probing  ← unpushed at checkpoint
3e6e071 docs(glossary): vocabulary reference + anti-vocabulary
1ef9806 chore(git): linguist overrides for repo-page language stats
5731b2a test(cli): help-text smoke and version-flag parity
df074a6 docs(troubleshooting): first-aid catalog for every vibe error
725f179 docs: CHANGELOG.md — milestone-by-milestone history
488812d docs(lockfile-format): exhaustive vibe.lock v2 reference
8c770f7 docs(architecture): contributor-facing architecture tour
8ec2354 docs: top-level README at repo root
db0d754 feat(install): content_hash integrity check on plan, ContentDrift error
439e601 docs(authoring): per-kind authoring guides for flow / feat / stack
4b7eb09 docs(commands): reference pages for every shipped CLI subcommand
ee02bc0 feat(schemas): JTD schemas for every CLI --json wire format
10c8511 docs(wal,tasks): checkpoint Phase A code slice complete
d803d1f test(manual): M1.5-gate v2 per-package registry smoke protocol
bdb1e93 chore(fixtures): relocate packages/ → fixtures/registry/
028b61b build(tools): JTD codegen scaffolding — xtask, schemas/, vibe-wire crate
6ce2ed2 feat(vibe-publish): RepoCreator trait + GitVerseCreator + vibe registry publish
3798088 feat(install): transitive install via NaiveDepSolver, populate lockfile deps
e044058 feat(vibe-resolver): DepSolver trait + NaiveDepSolver + provider adapter
4742885 docs(tasks): mark per-package registry sync done
766a949 feat(registry): per-package vibe registry sync
88df86c feat(install): switch CLI to MultiRegistryResolver, populate lockfile v2
b512ea2 refactor(registry): thread lockfile-v2 provenance through CachedPackage
05ae222 feat(registry): MultiRegistryResolver — priority + override + mirror schema
```

Together, this chain delivers the entirety of Phase A: schema migration (vibe.toml v2, vibe.lock v2, capability deps), depsolver layer, multi-registry resolver, per-package registry runtime, publish utility, JTD scaffolding, fixture relocation, manual-test protocol, all user/contributor docs, plus the GitVerse API correction. Total workspace state: 169+ tests green, clippy clean.

---

## Quick-start commands

```bash
# Workspace health check.
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Scaffold a project.
cargo run -p vibe-cli -- init --path /tmp/demo

# Install a package (transitive resolve via NaiveDepSolver, lockfile v2).
cargo run -p vibe-cli -- install flow:wal --path /tmp/demo

# List installed packages.
cargo run -p vibe-cli -- list --path /tmp/demo

# Refresh per-package clones (registry sync).
cargo run -p vibe-cli -- registry sync --path /tmp/demo

# Publish a package (maintainers; needs ~/.vibevm/git.publish.token).
cargo run -p vibe-cli -- registry publish fixtures/registry/flow/wal/v0.1.0 --dry-run
cargo run -p vibe-cli -- registry publish fixtures/registry/flow/wal/v0.1.0

# JTD codegen (after one-time install per tools/jtd-codegen/README.md).
cargo xtask codegen
cargo xtask check-codegen
```

---

## Standing rules / pointers

- **Read on session boot, in this order:** `CLAUDE.md`, every file in `spec/boot/` in filename order, `spec/WAL.md`, then any PROP/FEAT documents under `spec/common/` and `spec/modules/` for the task at hand. **THEN** start work.
- **Four non-negotiable rules** (CLAUDE.md, copied in PROP-000 §12): (1) human-only attribution, (2) Conventional Commits, (3) group commits by meaning, (4) autonomy on routine changes only.
- **Memory discipline:** project facts in repo, machine-local in user-memory.
- **Setup-docs obligation** (PROP-000 §19): toolchain / prereqs / env / paths changes → `DEV-GUIDE.md` or `RUNTIME-GUIDE.md` in same commit.
- **Vocabulary lock:** `flow` / `feat` / `stack` / `tool`; never `lifecycle` / `phase` / `goal` / `plugin`.
- **User-owned files** (`vibe install` / `uninstall` never modifies): `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any 00-09 or 90-99 boot file.
- **License hygiene:** permissive only (MIT / Apache-2.0 / BSD / Unlicense; MPL-2.0 case-by-case; GPL/AGPL/LGPL forbidden).
- **Manifest format:** TOML for human-edited (`vibe.toml`, `vibe.lock`, `vibe-package.toml`); JTD+codegen for wire contracts.
- **REVIEW marker discipline:** when the spec is silent, pick the conservative interpretation, mark with `<!-- REVIEW: … -->`, surface in the session report.
- **`refs/` is gitignored** — book + cloned reference repos.

---

## Beyond Phase A — what comes next

Once Phase A closes (live migration done, smoke passes, `DEFAULT_REGISTRY_URL` rotated, WAL/ROADMAP/TASKS checkpointed):

- **M1.2 — `vibe update`.** Lock-aware version bumping, respecting capability/conflict constraints. Depsolver drives it.
- **M1.3 — `vibe check`.** Constraint validation against the current lockfile + manifest; reports missing capabilities, conflict detections, dirty pkgrefs.
- **M1.4 — `vibe show`.** Inspector for installed packages — manifest, dependency tree, source provenance, content_hash trail.
- **M1.5 — `vibe build`.** Compose a flow set into a runnable target (depends on `stack` semantics; gates on a working stack package).
- **M1.6 — multi-registry polish (Phase B of decentralized-registry refactor).** Live mirror dispatch, `vibe vendor` generator for offline mirrors, `vibe registry add/list/set-mirror` CLI surface, GitHub/Gitea/Forgejo publish adapters on demand. Parts of this depend on a second live registry being available, which depends on the v0.1.0 flows being on the new org first.
- **JTD struct migration sweep.** Hand-rolled `Serialize` structs in `vibe-cli` swap to `vibe-wire::generated::*` types one consumer at a time. Triggered by anyone touching the consumer.
- **Supply-chain attestation (sigstore or equivalent).** Out of M1 scope, noted as architectural-allowance-now.

---

## Things to be careful about (not blockers, but easy to slip on)

- **Don't `git push --force` to `main`.** Rule 4. If you need to rewrite published history, ask the owner first.
- **Don't auto-attribute commits to AI.** Rule 1. No `Co-Authored-By` trailers, no model-name in commit bodies, branches, or code comments. Single project-wide exception: this paragraph and its copy in PROP-000 §12.1 (the rule itself) are allowed to *discuss* AI tooling.
- **Don't edit `VIBEVM-SPEC.md`** without owner sign-off — owner-frozen.
- **Don't commit `refs/`** content — it's reference reading.
- **Don't put project-scoped facts in user-memory.** Memory discipline. Project facts go to the repo.
- **Don't pre-populate the GitVerse repos** before manual creation (no auto-init, no README, no .gitignore). The publisher's first push will conflict.
- **Don't skip the smoke before rotating `DEFAULT_REGISTRY_URL`.** That value steers every new project's `vibe init`. Rotating before the new registry actually works strands users.

---

## If something has changed since this checkpoint

This file is frozen at 2026-04-26. Before acting on it:

- Re-read `spec/WAL.md` (it gets updated more often than this file does — and at session-end it gets updated at the same time).
- `git log origin/main..HEAD --oneline` to see what's local.
- `git status` to see the working tree.
- `cat TASKS.md` for the latest checklist state.

If the WAL and this file disagree, **trust the WAL** — it's the canonical living state.
