# CONTINUE — cold-resume checkpoint

_Written: 2026-04-29. Owner-readable, self-contained, deliberately verbose. Pick this up with zero prior context and you should be able to continue without asking questions._

---

## TL;DR (executive summary)

**M1.1-revision Phase A is done.** Decentralized per-package registry shipped end-to-end on its production host. All three v0.1.0 demo flows live on GitHub:

- <https://github.com/vibespecs/flow-wal>
- <https://github.com/vibespecs/flow-sync-from-code>
- <https://github.com/vibespecs/flow-atomic-commits>

Each tagged `v0.1.0`. Anonymous `vibe init` → `vibe install flow:wal` / `flow:sync-from-code` / `flow:atomic-commits` resolves all three from `https://github.com/vibespecs`, populates `vibe.lock` v2 with `registry = "vibespecs"` / GitHub `source_url`s / `content_hash`s, and `vibe registry sync` refreshes per-package clones. Workspace `cargo test --workspace` ≈ 210+ tests green; `cargo clippy --workspace --all-targets -- -D warnings` clean.

**Host posture (decided 2026-04-29).** vibevm tool source stays on **GitVerse** (`git@gitverse.ru:anarchic/vibevm`); package registry organization moved to **GitHub** (`https://github.com/vibespecs`). Reason: GitVerse's public REST API does not expose org-scoped repo creation (`POST /orgs/{org}/repos`), which `vibe registry publish` needs to drive end-to-end. The split is documented in [PROP-000 §7](spec/common/PROP-000.md#registry) and [PROP-002 §2.10](spec/modules/vibe-registry/PROP-002-decentralized-registry.md#publish).

**Token discipline (PROP-000 §20).** Publish-token loader walks: `VIBEVM_PUBLISH_TOKEN` env → `~/.vibevm/<host-prefix>.publish.token` (e.g. `github.publish.token`) → legacy `~/.vibevm/git.publish.token`. Token is **never** displayed in any vibevm-produced output — CLI prints the *source* of the token (env-var name or file path) but never the value; modern git ≥ 2.31 redacts URL passwords in its own logs; `redact_credentials(s)` helper scrubs anything credential-shaped before it reaches a `PublishError` message. Adapter scope: each `RepoCreator` impl refuses operations outside the org named in the project's `[[registry]].url`.

The most recent commit chain (newest first) covers the migration:

```
86dfae3 fix(vibe-registry): clone fallback and tag-aware update for GitHub
6e1bb3a fix(vibe-publish): redact credentials from git error messages
39a2152 feat(core,cli): rotate DEFAULT_REGISTRY_URL to GitHub vibespecs
ab0a3d4 feat(vibe-publish,cli): GitHub host adapter and per-host token loader
72dae08 docs(spec,guides,manual-tests): migrate registry org to GitHub
```

Plus the WAL/CONTINUE/TASKS/ROADMAP close-out commit landing alongside this file. After that lands, push the slice to `origin/main` (still GitVerse — the project repo doesn't move).

---

## Where we are right now

- **Branch:** `main`. Working tree is the close-out commits + this CONTINUE rewrite.
- **Latest checkpoint:** `86dfae3 fix(vibe-registry): clone fallback and tag-aware update for GitHub` (the second of the bug-fixes that surfaced during the live-migration smoke).
- **Workspace health:** 12 crates, ~210+ tests green, clippy clean with `-D warnings`. `vibe-publish` alone has 30 unit tests covering host adapter selection, token redaction (including `Token::Display` / `Debug` invariants and the new `redact_credentials` helper), scope-violation guards, and per-host token-file precedence.
- **Live state:** three GitHub repos exist, populated, tagged. Smoke ran end-to-end against the live host inside the migration session; the markdown protocol in `manual-tests/M1.5-gate-v2-per-package-smoke.md` is the same shape, just not formally walked top-to-bottom this session — see "Optional follow-up" below.

---

## What changed in the migration slice

### 1. The split-host posture (PROP-000 §7)

The vibevm project source repository (`anarchic/vibevm`) and the package registry organization (`vibespecs`) live on **separate hosts** by deliberate decision. PROP-000 §7 carries the long-form rationale; the short version:

- vibevm tool source stays on **GitVerse** — contributor SSH keys, mirroring posture, Russian-jurisdiction hosting all already wired.
- Package registry on **GitHub** — `POST /orgs/{org}/repos` works, `git clone` over public HTTPS works, `git ls-remote --tags` works, the API surface is well-documented and stable.

`spec/boot/90-user.md` carries the operator-facing version of the rule for this machine.

### 2. The token-secrecy invariant (PROP-000 §20)

Publish tokens / API tokens / future LLM keys are surface secrets. They MUST NOT appear in any human- or machine-readable surface vibevm produces — stdout, stderr, JSON event stream, error messages, panic traces, telemetry, lockfile, committed files, the `.vibe/` cache. The single sanctioned at-rest location is `~/.vibevm/<host>.publish.token` (per-user, chmod-protected). The single sanctioned process-boundary crossing is the host API's `Authorization: Bearer …` header (over TLS) or the `https://x-access-token:<TOKEN>@host/…` URL embed handed to one `git push` invocation (modern git ≥ 2.31 redacts URL passwords in its own log output to `***`).

**Operator discipline:** never `cat` / `head` / `tail` / `echo` / `grep` the token file, never paste it into chat / log / shell snippet / video overlay / bug report. The CLI prints `Loaded publish token from <path> (value redacted)` so the operator sees auth provenance without seeing the value.

**Adapter scope:** each `RepoCreator` impl is constructed with an `expected_org` and refuses operations targeting any other org. Belt-and-suspenders on top of the CLI boundary that derives the org from the registry URL.

PROP-000 Invariant #7 in the bottom-of-document list pins this as a global rule, not a module-local one.

### 3. The host adapter pattern (PROP-002 §2.10)

`RepoCreator` trait gained two methods:

- `push_url(org, name) -> String` — returns the URL `git push` should target. SSH-auth hosts (GitVerse) return the bare SSH URL; HTTPS-token-auth hosts (GitHub) return the URL with credentials embedded for the duration of the push.
- `expected_org() -> Option<&str>` — drives the default `validate_scope(org)` guard.

Two concrete impls today:

- **`GitHubCreator`** (`crates/vibe-publish/src/github.rs`, ~330 lines including tests). `https://api.github.com`, `Authorization: Bearer <T>`, `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`, User-Agent `vibe-publish/<crate-version>`. `repo_exists` via `GET /repos/{owner}/{repo}`, `create_repo` via `POST /orgs/{org}/repos` with `auto_init = false`.
- **`GitVerseCreator`** (`crates/vibe-publish/src/gitverse.rs`, retained). Constructor signature widened to take `expected_org` so the legacy adapter participates in the scope-guard discipline. SSH push URL behaviour preserved (`git@gitverse.ru:<org>/<repo>.git`).

Adapter selection at the CLI layer: `creator_for_url(org_url, expected_org, token)` factory pulls the host segment from the URL and dispatches. Unknown host → `PublishError::UnsupportedHost` with a clean error message.

### 4. The two latent bugs GitHub flushed out

**Bug 1 — `git archive --remote` is not exposed by GitHub.** GitHub's smart-HTTPS protocol responds with `HTTP 422` and the local git reports `expected ACK/NAK, got a flush packet`. The existing `fetch_file_at_ref` classifier did not match this shape, so the failure landed as `CommandFailed` and the resolver mis-classified the package as missing. Fixed in `git_backend/shell.rs::fetch_file_at_ref`: two new substring matchers (`http 422` + `git archive`, and `git archive` + `expected ack/nak` + `flush packet`) surface `ArchiveUnsupported`. `GitPackageRegistry::fetch_dep_manifest` now catches `ArchiveUnsupported` and falls back to a per-package shallow clone at the requested tag, reading the manifest from the working tree. The clone lands in the same per-package cache directory the install path would use anyway, so the fallback also pre-warms the cache.

**Bug 2 — `update()` couldn't reset to a tag ref.** The previous implementation ran `git fetch --prune origin` (no `--tags`) then `git reset --hard origin/<refname>`. That works for branches but not for tags (tags don't get an `origin/` prefix). Per PROP-002 §2.5 every per-package version is a git tag, so the M1.1-revision world is *almost entirely* tag refs. M1.1-monorepo masked the bug because the registry was a single repo with `main` as its only ref of interest. Fix: `update()` now runs `git fetch --prune --tags origin` and tries `refs/tags/<refname>` first, falling back to `origin/<refname>`.

### 5. Per-host token-file precedence

`vibe-publish::token::load_token_for_host(host)` walks:

1. `VIBEVM_PUBLISH_TOKEN` env (highest, useful for CI).
2. `~/.vibevm/<host-prefix>.publish.token` — per-host file. Prefix is the first label of the host (`github` for `github.com`, `gitverse` for `gitverse.ru`).
3. `~/.vibevm/git.publish.token` — legacy host-agnostic fallback. Kept so existing GitVerse-only setups keep working without rename.

`load_token(host)` is retained as a back-compat alias.

### 6. Default registry URL rotation

`vibe_core::manifest::project::DEFAULT_REGISTRY_URL` rotated from `git@gitverse.ru:anarchic/vibespecs.git` to `https://github.com/vibespecs`. Per-package URLs are derived at fetch time via the `naming` convention (default `kind-name` produces `<org>/<kind>-<name>`). `DEFAULT_REGISTRY_NAME` rotated from generic `default` to descriptive `vibespecs` so fresh `vibe init` projects visibly say which org they target.

---

## Repository map

```
vibevm/                                 (this repo — stays on GitVerse)
├── CLAUDE.md / AGENTS.md / GEMINI.md   ← byte-identical, 4 rules + Memory discipline
├── CONTINUE.md                         ← THIS FILE
├── CHANGELOG.md
├── DEV-GUIDE.md / RUNTIME-GUIDE.md     ← contributor / end-user setup docs (token paths updated)
├── LICENSE.md                          ← proprietary placeholder, target UPL 1.0
├── MEMORY.md                           ← pointer to spec/boot/90-user.md
├── README.md
├── ROADMAP.md
├── TASKS.md
├── VIBEVM-SPEC.md                      ← owner-frozen v1.0 spec (do not edit without sign-off)
│
├── crates/                             ← Rust workspace (12 crates)
│   ├── vibe-core/
│   │   └── src/manifest/project.rs::DEFAULT_REGISTRY_URL  ← `https://github.com/vibespecs`
│   ├── vibe-cli/                       ← `vibe` binary; `commands/{init,install,list,uninstall,registry,version}.rs`
│   │   └── src/commands/registry.rs    ← host-aware adapter selection via creator_for_url()
│   ├── vibe-registry/
│   │   ├── src/git_backend/shell.rs    ← classifier recognises GitHub-shape archive failure; update() fetches with --tags
│   │   └── src/git_package_registry.rs ← fetch_dep_manifest falls back to clone on ArchiveUnsupported
│   ├── vibe-resolver/
│   ├── vibe-install/
│   ├── vibe-publish/
│   │   ├── src/lib.rs                  ← RepoCreator trait + push_url() + expected_org() + creator_for_url() factory
│   │   ├── src/github.rs               ← NEW — GitHubCreator
│   │   ├── src/gitverse.rs             ← retained; constructor takes expected_org
│   │   ├── src/git_publish.rs          ← redact_credentials() scrubs URLs in error messages
│   │   └── src/token.rs                ← load_token_for_host() with per-host file precedence
│   ├── vibe-wire/
│   ├── vibe-graph/                     ← (M0 placeholder; not active)
│   ├── vibe-llm/                       ← (M0 placeholder; not active)
│   └── vibe-check/                     ← (M0 placeholder; not active)
│
├── docs/
│   ├── README.md
│   ├── architecture.md
│   ├── lockfile-format.md
│   ├── troubleshooting.md
│   ├── glossary.md
│   ├── authoring-flow.md / authoring-feat.md / authoring-stack.md
│   └── commands/
│       ├── init.md / install.md / list.md / uninstall.md / version.md
│       ├── registry-sync.md
│       └── registry-publish.md         ← host-adapter table, per-host token precedence, dry-run scope
│
├── manual-tests/
│   ├── M1.1-git-registry-smoke.md
│   ├── M1.5-gate-multi-package-smoke.md
│   └── M1.5-gate-v2-per-package-smoke.md   ← REWRITTEN for the GitHub host
│
├── fixtures/
│   └── registry/
│       └── flow/{wal,sync-from-code,atomic-commits}/v0.1.0/   ← migration source content (still here)
│
├── schemas/                            ← JTD wire-contract schemas
│
├── spec/
│   ├── WAL.md                          ← Phase A close-out checkpoint
│   ├── boot/
│   │   ├── 00-core.md (user-owned) … 90-user.md (user-owned, GitHub-aware)
│   ├── common/
│   │   └── PROP-000.md                 ← §7 split-host posture, §20 token-secrecy
│   └── modules/
│       └── vibe-registry/
│           ├── PROP-001-git-backend.md
│           └── PROP-002-decentralized-registry.md  ← §2.10 GitHubCreator alongside GitVerseCreator
│
├── tools/
│   └── jtd-codegen/                    ← README pins jtd-codegen install procedure
│
├── refs/                               ← .gitignore'd; reference reading material
├── xtask/                              ← `cargo xtask codegen` / `check-codegen`
└── .cargo/config.toml
```

---

## The live registry on GitHub

Public read access — no auth needed for `vibe install`. Three repos with `v0.1.0` tags:

```
$ git ls-remote --tags https://github.com/vibespecs/flow-wal
8cd45d900275d130425b5733f9845e5612da0fab	refs/tags/v0.1.0
1c3a1355f023c6dfd610dc73c909012bc83f9784	refs/tags/v0.1.0^{}

$ git ls-remote --tags https://github.com/vibespecs/flow-sync-from-code
e1f8f7a187ac6124542ac05d3dce909e9b989c5f	refs/tags/v0.1.0
a620157d628187f7f72bf3a5dc8ba1617e700067	refs/tags/v0.1.0^{}

$ git ls-remote --tags https://github.com/vibespecs/flow-atomic-commits
141ec8fd4a3ff9901c2fb601a1f5955b5f82fdd9	refs/tags/v0.1.0
d7651203497eb1e5f3fea5fe16c15a4906d361d7	refs/tags/v0.1.0^{}
```

Peeled commit SHAs (`^{}` suffix) are what the consumer-side install ends up at. If these change without a corresponding new release commit, that's a force-push and `content_hash` will mismatch on the next install — by design, the integrity check fails hard rather than silently substitute content.

**Lockfile shape after install (verified end-to-end on 2026-04-29):**

```toml
[[package]]
kind = "flow"
name = "wal"
version = "0.1.0"
registry = "vibespecs"
source_url = "https://github.com/vibespecs/flow-wal.git"
source_ref = "v0.1.0"
content_hash = "sha256:8136ecdbc25d4555cbab6e9574f153b252a05c62b55b5e0255def645458c9544"
```

Same shape for `flow-sync-from-code` (`content_hash = sha256:6b02b4dd…`) and `flow-atomic-commits` (`content_hash = sha256:60354a7e…`).

---

## Important decisions — long-form list

These are the load-bearing architectural and policy decisions made or restated during this conversation. Each is "settled" — don't unpick without owner discussion.

1. **Decentralized per-package registry** (PROP-002). One git repo per package, default naming `<kind>-<name>` under an org, versions are git tags. Avoids Nix-style host vendor lock-in at the design layer.

2. **Identity is content-hashed, URLs are informational.** A package's identity is `(kind, name, version, content_hash)`. The migration from GitVerse to GitHub mid-Phase-A tested this in anger — `source_url` rotates, `content_hash` does not.

3. **Split-host posture** (PROP-000 §7, decided 2026-04-29). vibevm source on GitVerse, registry org on GitHub. Each host chosen on its own merits. The vibevm project itself is **not** moving.

4. **`[[registry]]` array in `vibe.toml`** — never a singleton. Backed by serde alias on the v1 form. Priority-ordered. `[[mirror]]` and `[[override]]` are siblings, not nested. Schema is fully shipped in Phase A; runtime mirror dispatch lands in Phase B (M1.6).

5. **Lockfile schema v2** carries full provenance: `registry`, `source_url`, `source_ref`, `resolved_commit`, `content_hash`, `dependencies`, `overridden` per package; `[meta]` carries `schema_version`, `solver`, `root_dependencies`. v1 lockfiles auto-migrate on next write.

6. **Capability-based deps from day one.** PROP-000 §18 sets the bar at "complexity ≥ RPM".

7. **Six guiding principles** in PROP-000:
   - §15 Dependency weight is not a decision factor.
   - §16 JTD + codegen by default for wire contracts.
   - §17 Production architecture in the prototype phase.
   - §18 Complexity ≥ RPM.
   - §19 Load-bearing setup docs.
   - §20 Token secrecy and adapter scope (NEW 2026-04-29).

8. **Memory discipline** (CLAUDE.md / AGENTS.md / GEMINI.md). Project facts in repo, machine-local in user-memory.

9. **`DepSolver` trait + first impl `NaiveDepSolver`** (DFS, no backtracking). `resolvo` and `libsolv` slots reserved.

10. **`RepoCreator` trait + two impls** (`GitHubCreator`, `GitVerseCreator`). Adapter pattern; new hosts land as one new impl.

11. **`GitBackend` trait + `ShellGit` shell-out.** No in-process libgit2 / gitoxide.

12. **Token redaction** (PROP-000 §20). `Token::Display` / `Debug` print `***`; `redact_credentials(s)` scrubs URLs in error messages; CLI prints token *source* only.

13. **GitVerse public-API surface, live-verified 2026-04-26.** Base `https://api.gitverse.ru`, Bearer auth, versioned Accept header. `GET /repos/{org}/{repo}` works; `POST /orgs/{org}/repos` does not — the trigger for the host migration.

14. **GitHub public-API surface, live-verified 2026-04-29.** Base `https://api.github.com`, Bearer auth, `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`. Both endpoints work; `git archive --remote` is *not* exposed (HTTP 422), handled by clone fallback.

15. **Cache layout** per PROP-002 §2.6: `~/.vibe/registries/<canonical-url-hash>/packages/<kind>-<name>/clone/`. Bucket-hash now keys off `https://github.com/vibespecs` after the rotation.

16. **Manual-test protocol** (PROP-000 §14). Runnable smoke-tests in `manual-tests/`, one file per scenario.

17. **Conventional Commits + group by meaning + human-only attribution + autonomy on routine only.** Rules 1-4 in CLAUDE.md.

18. **Default registry URL** lives at `vibe_core::manifest::project::DEFAULT_REGISTRY_URL` — single source of truth. Now `https://github.com/vibespecs`.

19. **JTD toolchain** is scaffolded but not yet driving any consumer. Manual install of `jtd-codegen` per `tools/jtd-codegen/README.md` before `cargo xtask codegen` is run.

20. **Linguistic vocabulary lock.** Only `flow`, `feat`, `stack`, `tool` for package kinds. Anti-vocabulary catalogued in `docs/glossary.md`.

21. **Live migration is non-routine** per CLAUDE.md Rule 4 — owner sign-off was given for the GitHub publish run on 2026-04-29.

22. **Legacy registries kept readable.** `git@gitverse.ru:anarchic/vibespecs.git` (HEAD `2203239`) for projects still on schema-v1 lockfiles.

---

## Recent commit chain (last ~20, most-recent first)

```
86dfae3 fix(vibe-registry): clone fallback and tag-aware update for GitHub
6e1bb3a fix(vibe-publish): redact credentials from git error messages
39a2152 feat(core,cli): rotate DEFAULT_REGISTRY_URL to GitHub vibespecs
ab0a3d4 feat(vibe-publish,cli): GitHub host adapter and per-host token loader
72dae08 docs(spec,guides,manual-tests): migrate registry org to GitHub
e874f97 docs(continue,wal): cold-resume checkpoint, GitVerse API findings
7573455 docs(claude,agents,gemini): session-end checkpoint command spec
36cbf08 feat(vibe-publish): correct GitVerse API surface from live probing
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
```

The top five commits are the entire migration slice — spec amendments, code, default rotation, two security/correctness fixes, and (this commit) the close-out.

---

## Quick-start commands

```bash
# Workspace health.
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Scaffold a project against the GitHub-hosted vibespecs registry.
cargo run -p vibe-cli -- init --path /tmp/demo

# Install a package (resolves from https://github.com/vibespecs/flow-wal).
cargo run -p vibe-cli -- install flow:wal --path /tmp/demo

# List installed packages.
cargo run -p vibe-cli -- list --path /tmp/demo

# Refresh per-package clones.
cargo run -p vibe-cli -- registry sync --path /tmp/demo

# Publish a package (maintainers; reads token from
# ~/.vibevm/github.publish.token without echoing the value).
cargo run -p vibe-cli -- registry publish fixtures/registry/flow/wal/v0.1.0 --dry-run
cargo run -p vibe-cli -- registry publish fixtures/registry/flow/wal/v0.1.0
```

---

## Standing rules / pointers

- **Read on session boot, in this order:** `CLAUDE.md`, every file in `spec/boot/` in filename order, `spec/WAL.md`, then any PROP/FEAT documents under `spec/common/` and `spec/modules/` for the task at hand. **THEN** start work.
- **Four non-negotiable rules** (CLAUDE.md, copied in PROP-000 §12): (1) human-only attribution, (2) Conventional Commits, (3) group commits by meaning, (4) autonomy on routine changes only.
- **Memory discipline:** project facts in repo, machine-local in user-memory.
- **Setup-docs obligation** (PROP-000 §19): toolchain / prereqs / env / paths changes → `DEV-GUIDE.md` or `RUNTIME-GUIDE.md` in same commit.
- **Vocabulary lock:** `flow` / `feat` / `stack` / `tool`; never `lifecycle` / `phase` / `goal` / `plugin`.
- **Token secrecy** (PROP-000 §20): never display, never persist, never commit.
- **Adapter scope:** RepoCreators refuse operations outside the configured org.
- **User-owned files** (`vibe install` / `uninstall` never modifies): `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any 00-09 or 90-99 boot file.
- **License hygiene:** permissive only (MIT / Apache-2.0 / BSD / Unlicense; MPL-2.0 case-by-case; GPL/AGPL/LGPL forbidden).
- **Manifest format:** TOML for human-edited; JTD+codegen for wire contracts.

---

## Beyond Phase A — what comes next

- **M1.2 — `vibe update`.** Lock-aware version bumping, respecting capability/conflict constraints.
- **M1.3 — `vibe check`.** Constraint validation against the current lockfile + manifest.
- **M1.4 — `vibe show`.** Inspector for installed packages.
- **M1.5 — `vibe build`.** Compose a flow set into a runnable target.
- **M1.6 — multi-registry polish (Phase B of decentralized-registry refactor).** Live mirror dispatch, `vibe vendor` generator, `vibe registry add/list/set-mirror`, GitHub/Gitea/Forgejo publish adapters on demand.
- **JTD struct migration sweep.** Hand-rolled `Serialize` structs in `vibe-cli` swap to `vibe-wire::generated::*` types.
- **Supply-chain attestation.** Out of M1 scope.

## Optional follow-ups (left for next session)

- Walk `manual-tests/M1.5-gate-v2-per-package-smoke.md` top-to-bottom against the live GitHub host and fill in the "Last known pass" line at the top of the file.
- Schedule a recurring background agent to verify the `vibespecs` org on GitHub stays reachable and the v0.1.0 tags don't drift (peeled SHAs as of 2026-04-29 recorded above and in WAL).
- Migrate hand-rolled `Serialize` structs to `vibe-wire::generated::*` once `cargo xtask codegen` is exercised against the JTD schemas in `schemas/`.

---

## Things to be careful about (not blockers, but easy to slip on)

- **Don't `git push --force` to `main`.** Rule 4. If you need to rewrite published history, ask the owner first.
- **Don't auto-attribute commits to AI.** Rule 1.
- **Don't edit `VIBEVM-SPEC.md`** without owner sign-off.
- **Don't commit `refs/`** content.
- **Don't put project-scoped facts in user-memory.**
- **Don't ever print, paste, or screenshot the publish token.** PROP-000 §20.
- **Don't construct a `RepoCreator` without `expected_org`** — the scope guard is the second line of defence; bypassing it would be a regression.
- **Don't widen the API surface of `RepoCreator`** without checking how `Token` flows through every method — the trait is intentionally narrow so review can audit token paths exhaustively.

---

## If something has changed since this checkpoint

This file is frozen at 2026-04-29 (Phase A close-out). Before acting on it:

- Re-read `spec/WAL.md` (it gets updated more often than this file does — and at session-end it gets updated at the same time).
- `git log origin/main..HEAD --oneline` to see what's local.
- `git status` to see the working tree.
- `cat TASKS.md` for the latest checklist state.

If the WAL and this file disagree, **trust the WAL** — it's the canonical living state.
