# CONTINUE — cold-resume checkpoint

_Written: 2026-05-08 session-end. Owner-readable, self-contained. Pick this up with zero prior context._

---

## TL;DR (executive summary)

**The 2026-05-08 push closed three full milestones — M1.12, M1.13, and M1.14 (with three half-step closers: M1.14.1 / .2 / .3) — across 25 commits.** Together they make `vibe`'s package-management surface ready for v0.1.0: cargo-shape version constraints, `[requires]` declaration in `vibe.toml`, full registry-auth runtime (public + private), `--unattended` posture, comment-preserving manifest writes, MCP confirm-prompts wired correctly, and the `--auth-required` / `--exact` flags reaching every command they should.

Workspace state at HEAD `8ab5c9c`:
- vibe-cli e2e: **89 hermetic + 3 ignored**.
- vibe-core: **115 hermetic** (+5 toml_edit merge tests, +8 auth schema, +2 schema round-trip from earlier).
- vibe-registry: **94 hermetic** (+12 from M1.14 — classifier patterns, inject_token edges, MissingToken precheck, bootstrap-with-scrub end-to-end, resolver walk-vs-halt rules, strict-auth halt, aggregated walk attempts).
- vibe-cli bin: **93 hermetic** (+9 from `--unattended` resolver tests, schema lock-tests for env-vars).
- `cargo test --workspace` all green; `cargo clippy --workspace --all-targets -- -D warnings` clean; `vibe check --path . --quiet` 0/0/0.

Working tree is clean. `origin/main` is at `8ab5c9c`. No active blockers. Three deferred enhancements (aggregated JSON-mode error report, inline-comment preservation inside `[[registry]]` blocks, `vibe registry test` diagnostic) are all listed at the bottom of `spec/WAL.md` — none load-bearing.

---

## Where we are right now

- **Branch:** `main`. Working tree clean.
- **Latest commits this session (newest-first; full session was 25 commits):**

  ```
  8ab5c9c docs(roadmap,changelog): catch up on M1.12 / M1.13 / M1.14 milestones
  a915b12 docs(commands,wal): surface-consistency closing slice
  1f58e71 feat(vibe-cli): surface consistency — MCP --yes wired, --auth-required + --exact reach
  5c2b504 docs(commands,wal): closing-slice landings — strict-auth, aggregated report, comment preservation
  cac03fe feat(vibe-core): toml_edit-based comment-preserving writes for vibe.toml
  d7bf8bb feat(vibe-registry,vibe-cli): --auth-required + aggregated per-registry error report
  bf4111d docs(registry-auth,wal): user-facing reference + production-ready checkpoint
  1210268 feat(vibe-registry): per-auth walk-vs-halt + auth plumbing in resolver
  8942ee7 feat(vibe-registry): token injection + bootstrap-with-scrub for auth=token-env
  6dc8747 feat(vibe-registry): classifier + GitBackend::set_remote_url
  41efc0c feat(vibe-registry): TTY-aware credential helper silencing
  e65c73e feat(vibe-cli): --auth and --token-env on `vibe registry add`
  97753f7 feat(vibe-core): AuthKind enum + RegistrySection.auth/token_env
  5f296d9 docs(spec): per-registry auth axis (PROP-002 §2.2.1) + 401 classifier rules
  c9c18d7 docs(commands): document --unattended flag for scripted runs
  8420df5 feat(vibe-cli): --unattended global flag + VIBE_UNATTENDED env-var
  1572a11 docs(commands/mcp-install): provisioning recipe + best-effort `--scope both`
  b4cdcd7 feat(vibe-cli/mcp): --scope both is best-effort on the project leg
  01c5531 docs(versions): user-facing version-syntax reference
  8e84b6b docs(spec,commands,roadmap,wal): cargo-shape version syntax + --exact
  7992bca feat(vibe-cli/install): caret default constraint + --exact flag
  a158475 refactor(vibe-core,vibe-resolver): bare semver follows Cargo (caret) not exact
  d719457 feat(vibe-cli/search): hint that install bypasses index when search is empty
  e41a478 docs(vibe-cli/mcp): SKILL.md happy-path + --assume-yes + search/registry guards
  1697f5a docs(commands,roadmap,wal): refresh install/uninstall + checkpoint
  ```

- **Active blocker:** none.

---

## What to do first in the next session

Pick whichever matches the owner's interest:

### Option 1 — walk a real-world install against a private registry

The headline feature of this push is end-to-end private-registry support. The natural validation walk:

1. Stand up a small test registry on GitLab / Gitea / forgejo with one package repo + a tag.
2. `vibe registry add internal "https://<host>/<org>" --auth token-env` (let it derive the env-var name).
3. `export VIBEVM_REGISTRY_TOKEN_<HOST>=<PAT>` with read scope.
4. `vibe install <kind>:<name> --assume-yes` — should resolve, fetch via credentialed URL, and `git remote -v` inside the cloned cache should show the **plain** URL (token scrubbed).
5. `grep -r x-access-token ~/.vibe/registries/` — must come up empty (the hard token-discipline invariant).
6. `vibe install` again — should be `unchanged` everywhere.

Reference: `docs/registry-auth.md` covers every regime. If anything goes off-script, the symptom narrows the slice (classifier? token injection? walk-vs-halt? scrub?).

### Option 2 — `vibe registry test` diagnostic command

Read-only `git ls-remote` against each `[[registry]]`, prints per-registry status (reachable / auth-ok / auth-required / unreachable). Hour or so of work; very useful when first wiring a private registry. Code anchor: a new subcommand under `crates/vibe-cli/src/commands/registry.rs` next to `list` / `sync` / `vendor`.

### Option 3 — JSON-mode aggregated error report

Currently `RegistryError::PackageNotFoundEverywhere` carries a pre-formatted `summary: String` that flows through `Display` for text mode. JSON envelope still serialises through generic `Other(String)`. To structure it: extend `DepProviderError::UnknownPackage` with `attempts: Vec<RegistryAttempt>` (or a new variant), thread through resolver → install error → `emit_report`. Cross-crate API change in vibe-resolver; medium effort, narrow scope. WAL deferred-list calls this out explicitly.

### Option 4 — Inline-comment preservation inside `[[registry]]` blocks

Current `toml_edit`-merge in `vibe-core::manifest::write_toml` preserves three layers (header / per-table prefix / document trailing). What it doesn't preserve: comments **inside** a table (between `name = ...` and `url = ...`). Operators rarely write those, but if a case surfaces, the fix is per-key decor copy across the schema-paired keys. Same pattern as the existing per-table prefix copy.

### Option 5 — M1.5 (LLM generation)

The next major milestone per ROADMAP. Non-routine — needs explicit owner sign-off before starting. M1.5 is what makes vibevm "produce software" rather than "manage specs."

### Option 6 — CHANGELOG / ROADMAP further refinement

CHANGELOG `[Unreleased]` block now covers M1.12 / M1.13 / M1.14. Whenever v0.1.0 actually tags, that block migrates to a numbered `[0.1.0]` section. Same for ROADMAP — when a future release closes the v0.1 surface, M1.x entries can be lifted into a "Released" group.

---

## Non-obvious findings from this session

These cost time / hit edge cases — write them down so a future session does not re-derive.

### Rust 2024 edition forbids `unsafe` env-var mutation; tests need a workaround

`#![forbid(unsafe_code)]` at the crate level (which `vibe-registry` and `vibe-cli` both carry) blocks `std::env::set_var` because Rust 2024 marks it `unsafe`. Tests that mutate process env to exercise env-var-driven code paths cannot use `set_var` directly. Two approaches in this codebase:

1. **`Mutex<()>`-serialised env tests** in `vibe-cli::output::tests` — `INVOKED_BY_LOCK` and `UNATTENDED_LOCK` static `Mutex<()>` ensure parallel tests don't observe each other's transient env writes; inside the lock, tests use `EnvGuard` / `UnattendedGuard` RAII patterns wrapping `unsafe { ... set_var ... }` blocks. The crate must allow `unsafe_code` at the test scope for this to work.
2. **Test-only doc-hidden constructors** — `GitPackageRegistry::open_with_explicit_token` takes the resolved token directly (`Option<String>`) instead of reading an env-var. Production code calls `open_with_auth` (env-driven); tests call the explicit-token sibling. This avoids env mutation entirely. Cleaner; preferred when feasible.

If a future test needs to drive env-var resolution and neither approach fits, opening a test-only API like `open_with_explicit_token` is the path of least resistance.

### `apply_common_env` order matters: env BEFORE args

`ShellGit::run` prepends `-c credential.helper= -c core.askPass=` flags via `apply_common_env`. Those are global git options that **must** come before the subcommand name (`git -c k=v ls-remote ...`, not `git ls-remote -c k=v`). Every callsite of `Command::new("git")` followed by `apply_common_env(&mut cmd)` followed by `cmd.args(args)` needs that exact order. The ShellGit private methods (`run`, `run_raw`, `preflight`) and the test helper `run_or_panic` were all reordered in commit `41efc0c`; if a new git-spawning callsite is added, it must follow the same order or git will refuse the args as sub-command parameters.

### `GIT_ASKPASS=""` (empty value) confuses git's startup probe on some platforms

Setting `GIT_ASKPASS` to an empty string on Windows can produce a `cmd /C ""` invocation that git interprets as "askpass available, exec it" → fails the startup. Solution: don't set `GIT_ASKPASS` at all when silencing — `core.askPass=` (empty value via `-c`) plus `GIT_TERMINAL_PROMPT=0` plus `credential.helper=` is sufficient. The original silencing block tried `cmd.env("GIT_ASKPASS", "")`, hit this exact failure on every git invocation in the test suite, and was reverted to "leave GIT_ASKPASS alone."

### Token must NOT persist in `.git/config` after bootstrap

The token-discipline invariant is "token never on disk via vibevm-controlled paths." A naive `bootstrap(credentialed_url)` saves the credentialed URL into `<dest>/.git/config` as `remote.origin.url = https://x-access-token:<TOKEN>@...`, which violates the invariant. The fix in `update_clone_at_ref` is to immediately call `backend.set_remote_url(clone_dir, "origin", plain_url)` after a successful bootstrap — git `remote set-url` is a config write, not a network operation, and overwrites the credentialed URL with the plain one. Subsequent `update` calls hit the plain origin; if it returns 401 (still-private host), `ensure_clone_against_sources` wipes and re-bootstraps. Slight perf cost on stale-cache-against-private-host paths; acceptable trade.

### bare semver in Cargo crate IS caret, not exact

This was the reason for the M1.13 parser collapse. `semver::VersionReq::parse("0.3.0")` returns a caret-comparator, not an exact one. The pre-this-session vibevm parser explicitly converted bare semver to `=0.3.0` via `format!("={version}")` — that wrapper made vibevm diverge from cargo / npm / Poetry. Removing the wrapper (commit `a158475`) brought us in line. Pre-1.0 caret semantics are tighter than post-1.0 (`^0.3.0` matches only `0.3.x`, not `0.4.0`); since every vibevm package today is `0.x.y`, this is the intended behaviour.

### `vibe.toml` is mutated by `vibe install` / `uninstall` / `registry add` (M1.12+)

Pre-M1.12 `vibe.toml` was append-once: `vibe init` wrote it, the operator hand-edited it, no command rewrote it. M1.12 introduced `[requires].packages` writes from `vibe install` and `vibe uninstall`. M1.14.1 added `vibe registry add --auth ...` write. M1.14.2 layered `toml_edit` on top to preserve comments. Future commands that need to mutate `vibe.toml` should use the same `ProjectManifest::write` path — it goes through the comment-preserving merge automatically.

### Public-401 walk-past + GitVerse policy

GitVerse returns 401 (not 404) for missing public repos. Without the M1.14 walk-past rule, vibevm would halt the first time it hit a non-existent package against a project that has `vibespecs-gitverse` (the default GitVerse registry). PROP-002 §2.3.1 reclassifies "401 against `auth = "none"`" as `UnknownPackage` so the resolver walks to the next registry. `--auth-required` flips this back to halt-on-public-401 for CI / cron use cases where a public substitute would be wrong.

### MCP commands are TTY-confirm, NOT non-TTY-confirm

The M1.14.3 closer wires MCP `--yes` to a real apply-confirm prompt — but ONLY on a TTY. Non-TTY callers (CI / opencode) get the pre-existing zero-confirm behaviour preserved. Operators on a TTY without an explicit skip-flag now see `[y/N]` before any MCP-config / SKILL.md write. The TTY-gate condition is:

```rust
if args.yes || ctx.is_unattended() || args.auto || ctx.is_json()
   || !console::user_attended() {
    // approved, no prompt
}
```

The `!console::user_attended()` short-circuit at the bottom is what preserves CI-script compat. If a future change wants to make MCP commands strictly confirm in non-TTY too, that condition is the one place to touch.

---

## Repository map

```
vibevm/
├── CLAUDE.md / AGENTS.md / GEMINI.md   # Three identical copies of the four rules.
├── CONTINUE.md                          # This file. Cold-resume snapshot.
├── ROADMAP.md                           # Milestone-oriented plan; M1.14 closed via this push.
├── CHANGELOG.md                         # Milestone chronicle; [Unreleased] holds M1.12/M1.13/M1.14.
├── VIBEVM-SPEC.md                       # Owner-frozen spec; do not edit without explicit instruction.
├── DEV-GUIDE.md / RUNTIME-GUIDE.md      # Per-machine setup docs.
├── crates/
│   ├── vibe-cli/                        # `vibe` binary entry point. clap dispatch + per-subcommand modules.
│   │   └── src/commands/
│   │       ├── install.rs               # Resolve+plan+apply pipeline. M1.12 [requires] writes,
│   │       │                            # M1.13 caret-default + --exact, M1.14 --auth-required.
│   │       ├── update.rs                # Re-resolve+diff+apply. M1.14.3 --exact + --auth-required reach.
│   │       ├── uninstall.rs             # Symmetric to install. M1.12 [requires] cleanup.
│   │       ├── outdated.rs              # Read-only upstream-newer probe. M1.14.3 --auth-required reach.
│   │       ├── mcp.rs                   # Five-agent matrix; install/upgrade/uninstall/status/serve.
│   │       │                            # M1.14.3: walk_install/upgrade/uninstall extracted +
│   │       │                            # TTY-gated confirm prompt + --assume-yes alias.
│   │       ├── registry.rs              # add (with --auth/--token-env) / remove / list / set-mirror /
│   │       │                            # sync / vendor / publish.
│   │       ├── search.rs                # PROP-005 index-aware discovery; auth-naive (read-only).
│   │       └── skill_template.md        # Vendored two-state SKILL.md (Section A bootstrap + B + Common).
│   ├── vibe-core/                       # Manifests, lockfile schema v3, AuthKind, version_spec.
│   │   └── src/manifest/
│   │       ├── project.rs               # ProjectManifest + RegistrySection.auth/token_env (M1.14.1).
│   │       ├── package.rs               # PackageManifest (vibe-package.toml) + Requires/Provides.
│   │       ├── lockfile.rs              # vibe.lock schema v3 with full provenance.
│   │       └── mod.rs                   # write_toml() — comment-preserving via toml_edit (M1.14.2).
│   ├── vibe-graph/                      # In-memory dep graph helpers.
│   ├── vibe-registry/                   # The big crate — git_backend, GitPackageRegistry,
│   │   │                                # MultiRegistryResolver. Auth runtime lives here.
│   │   └── src/
│   │       ├── git_backend/
│   │       │   ├── mod.rs               # GitBackend trait + set_remote_url method (M1.14).
│   │       │   └── shell.rs             # ShellGit + apply_common_env (TTY-aware silencing).
│   │       ├── git_package_registry.rs  # Per-registry instance with auth + effective_token +
│   │       │                            # token_env_name; bootstrap-with-scrub flow.
│   │       ├── multi_registry_resolver.rs # Walk + per-auth walk-vs-halt + strict_auth gate.
│   │       └── lib.rs                   # RegistryError including MissingToken +
│   │                                    # PackageNotFoundEverywhere variants.
│   ├── vibe-resolver/                   # Feature expansion + activation evaluation (PROP-003).
│   ├── vibe-install/                    # Install pipeline: plan_install → apply → register.
│   ├── vibe-llm/                        # LLM provider abstraction. Skeleton — real impls land in M1.5.
│   ├── vibe-mcp/                        # JSON-RPC MCP server. 3 tools today.
│   ├── vibe-check/                      # Spec-consistency linter.
│   ├── vibe-publish/                    # GitHubCreator / GitVerseCreator / DirectGitCreator publishers.
│   └── vibe-wire/                       # JTD-codegen'd wire types.
├── services/
│   └── vibe-index/                      # Standalone PROP-005 utility: per-org package index. Own workspace.
├── spec/
│   ├── boot/{00-core,90-user}.md        # Read at every session start.
│   ├── WAL.md                           # Living checkpoint of project state. Authoritative if it
│   │                                    # diverges from this file.
│   ├── common/PROP-000…PROP-006         # Foundation policy + operating modes.
│   ├── modules/                         # Per-crate PROPs.
│   │   └── vibe-registry/PROP-002       # §2.2.1 (auth axis), §2.3.1 (failure classifier).
│   └── research/PROP-004                # Tessl comparative research.
├── docs/
│   ├── README.md                        # User-doc index; gained "Version syntax" + "Registry auth".
│   ├── architecture.md / lockfile-format.md / glossary.md / troubleshooting.md
│   ├── version-syntax.md                # NEW (M1.13) — operator reference for semver constraints.
│   ├── registry-auth.md                 # NEW (M1.14) — operator reference for the four auth regimes.
│   ├── commands/                        # Per-subcommand reference. install / update / mcp-* / registry-*
│   │                                    # all updated with --auth-required / --exact / --unattended notes.
│   ├── guides/                          # Long-form walkthroughs.
│   └── authoring-{flow,feat,stack}.md
├── manual-tests/                        # Runnable smoke protocols.
├── fixtures/registry/                   # Hermetic per-package registry fixtures.
├── tools/                               # self-check.sh + jtd-codegen install README.
└── xtask/                               # `cargo xtask codegen` / `check-codegen`.
```

---

## Architectural / policy decisions still in force

In rough order of how often they bite a fresh contributor:

1. **Four non-negotiable rules** ([PROP-000 §12](spec/common/PROP-000.md#commits)):
   1. **No AI / machine-author attribution** anywhere.
   2. **Conventional Commits.** Subject ≤ 60 chars (hard limit 72), body explains WHY.
   3. **Group commits by meaning**, never by file or by time.
   4. **Autonomy on routine changes.** Non-routine red lines (history rewrite, `--force` push, large blobs, CI / signing / secrets, irreversible ops) STILL require explicit owner sign-off.

2. **Memory discipline.** Project facts live in the repo. Per-machine facts only live in tool-specific user-memory.

3. **Vocabulary lock.** Only `flow`, `feat`, `stack`, `tool`. Never `lifecycle` / `phase` / `goal` / `plugin`.

4. **Language: Rust.** Permissive licenses only. `dependency weight is not a decision factor` per PROP-000 §15.

5. **Manifest format: TOML** for human-edited; **JTD+codegen** for wire contracts.

6. **Identity: `(kind, name, version, content_hash)`.** URL is informational.

7. **Token secrecy** (PROP-000 §20). Never printed in any vibevm-produced output. Modern git (≥ 2.31) auto-redacts; vibevm relies on that as the second line of defence.

8. **Repository hosts.** vibevm source = GitVerse. Package registry = GitHub `vibespecs` (primary) + GitVerse `vibespecs` (secondary).

9. **User-owned files** (vibevm install/uninstall NEVER touches): `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`.

10. **PROP-006 codewords.** `«move fast and break things»` is the first; never overrides the four rules.

11. **Cargo-shape version syntax** (M1.13). Bare semver `0.3.0` = caret `^0.3.0`. Use `=0.3.0` for strict equal.

12. **`[requires]` is the source of truth for declared deps** (M1.12). `vibe.toml` carries the human's input list (constraints); `vibe.lock` carries the resolved materialisation (exact pins + content hashes). `meta.root_dependencies` in the lockfile is a mirror, not authoritative.

13. **Per-registry `auth` axis** (M1.14, PROP-002 §2.2.1). Four regimes: `none` (default, public read-only) / `token-env` (PAT from env-var) / `credential-helper` (system git helpers, opt-in) / `ssh` (delegated to ssh-agent).

14. **Auth-aware 401 classifier** (PROP-002 §2.3.1). 401 on `auth = "none"` walks past as `UnknownPackage`; 401 on authenticated registries halts. `--auth-required` flips public-401 to halt for strict CI gating.

15. **Token never on disk via vibevm-controlled paths** (M1.14). Tokens loaded once at registry-open from env-var, held in memory only, scrubbed from `.git/config` immediately after `bootstrap` via `set_remote_url(.., "origin", plain_url)`.

16. **TTY-aware credential silencing** (M1.14). `apply_common_env` silences GCM / `credential.helper` / `core.askPass` in non-TTY / `--unattended` runs. Interactive TTY without `--unattended` leaves them alone — operator might genuinely want a one-off password prompt.

17. **`--unattended` global flag** + `VIBE_UNATTENDED` env-var (truthy: `1`, `true`, `yes`, `on`). Implies skip-confirm everywhere; refuses to open wizards in MCP install; stamps `unattended: true` on every JSON envelope.

18. **MCP command confirm-prompt is TTY-gated** (M1.14.3). Non-TTY callers see the pre-this-version zero-confirm behaviour preserved; TTY callers see a real `[y/N]` summary unless they pass `--yes` / `--unattended` / `--auto` / `--json`.

19. **comment-preserving `vibe.toml` writes** (M1.14.2). Three layers via `toml_edit`: header comments, per-table prefix, document trailing. Inline-comments inside tables not yet preserved (deferred enhancement).

---

## Recent commit chain (last 25, newest first)

```
8ab5c9c docs(roadmap,changelog): catch up on M1.12 / M1.13 / M1.14 milestones
a915b12 docs(commands,wal): surface-consistency closing slice
1f58e71 feat(vibe-cli): surface consistency — MCP --yes wired, --auth-required + --exact reach
5c2b504 docs(commands,wal): closing-slice landings — strict-auth, aggregated report, comment preservation
cac03fe feat(vibe-core): toml_edit-based comment-preserving writes for vibe.toml
d7bf8bb feat(vibe-registry,vibe-cli): --auth-required + aggregated per-registry error report
bf4111d docs(registry-auth,wal): user-facing reference + production-ready checkpoint
1210268 feat(vibe-registry): per-auth walk-vs-halt + auth plumbing in resolver
8942ee7 feat(vibe-registry): token injection + bootstrap-with-scrub for auth=token-env
6dc8747 feat(vibe-registry): classifier + GitBackend::set_remote_url
41efc0c feat(vibe-registry): TTY-aware credential helper silencing
e65c73e feat(vibe-cli): --auth and --token-env on `vibe registry add`
97753f7 feat(vibe-core): AuthKind enum + RegistrySection.auth/token_env
5f296d9 docs(spec): per-registry auth axis (PROP-002 §2.2.1) + 401 classifier rules
c9c18d7 docs(commands): document --unattended flag for scripted runs
8420df5 feat(vibe-cli): --unattended global flag + VIBE_UNATTENDED env-var
1572a11 docs(commands/mcp-install): provisioning recipe + best-effort `--scope both`
b4cdcd7 feat(vibe-cli/mcp): --scope both is best-effort on the project leg
01c5531 docs(versions): user-facing version-syntax reference
8e84b6b docs(spec,commands,roadmap,wal): cargo-shape version syntax + --exact
7992bca feat(vibe-cli/install): caret default constraint + --exact flag
a158475 refactor(vibe-core,vibe-resolver): bare semver follows Cargo (caret) not exact
d719457 feat(vibe-cli/search): hint that install bypasses index when search is empty
e41a478 docs(vibe-cli/mcp): SKILL.md happy-path + --assume-yes + search/registry guards
1697f5a docs(commands,roadmap,wal): refresh install/uninstall + checkpoint
```

---

## Quick-start commands

```powershell
# Build everything.
cargo build --workspace

# Full test gate (matches CI).
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p vibe-cli -- check --path . --quiet

# Or one-shot via the bundled script.
bash tools/self-check.sh

# Install vibe into ~/.cargo/bin/.
cargo install --path crates/vibe-cli --locked

# Three signature recipes from this push:

# Public install, scripted, no prompts ever:
vibe --unattended install flow:wal

# Private registry on a fresh user account, one-time setup:
vibe registry add internal "https://gitlab.example/vibespecs" --auth token-env
export VIBEVM_REGISTRY_TOKEN_GITLAB_EXAMPLE=ghp_...
vibe --unattended install flow:internal-helper

# MCP provisioning (no project yet — Section A in SKILL.md):
vibe --unattended mcp install --agent opencode --scope both --what both
```

---

## Pointer

`spec/WAL.md` is the canonical **living** checkpoint. If anything in this `CONTINUE.md` disagrees with the top of `spec/WAL.md`, trust the WAL — it gets bumped every session.
