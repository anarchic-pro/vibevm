# WAL — Project Continuation State
_Updated: 2026-04-17_

## Current phase

**Milestone M0 — walking skeleton: COMPLETE and published.** Spec reconciled with shipped code; roadmap documented; ready to start M1.

The initial repository history is on `git@gitverse.ru:anarchic/vibevm.git`. All M0 acceptance checklist items (`VIBEVM-SPEC.md` §16) pass — the `vibe` binary performs the full `init → install → list → uninstall` loop against a local-directory registry, the canonical `flow:wal@0.1.0` package is hand-written under `packages/flow/wal/v0.1.0/`, and 64 tests cover the cycle. The four non-negotiable project rules (attribution, Conventional Commits, grouping by meaning, autonomy on routine changes) live in [`CLAUDE.md`](../CLAUDE.md) / `AGENTS.md` / `GEMINI.md` (verbatim) and in [spec://vibevm/common/PROP-000#commits](common/PROP-000.md#commits) (authoritative).

`VIBEVM-SPEC.md` §13.1 now pins the **mirror package layout** (every path in `writes.files` is both the source inside the package and the target inside the installed project). §6.3 documents both exact-filename and numeric-prefix boot-snippet conflict detection. §5.6 notes the M0 procedural implementation of the install subgraph. §9.1 and §11.1 reflect the actual shipped CLI flags.

[`ROADMAP.md`](../ROADMAP.md) is the long-form milestone plan — M1 (package manager with git registry), M1.5 (generation), M2 (production-readiness), M3+ (speculation). Side-quests (`.gitattributes`, `gc.auto`, workspace README, CI matrix) are listed there too.

**Next:** M1 — start with the git backend in `vibe-registry`.

## Constraints (do not violate without discussion)

- **Language:** Rust only for the CLI. See [spec://vibevm/common/PROP-000#language](common/PROP-000.md#language).
- **License:** proprietary EULA placeholder (see [`LICENSE.md`](../LICENSE.md)); eventual target is UPL 1.0 — owner's decision, not final. See [spec://vibevm/common/PROP-000#license](common/PROP-000.md#license).
- **Manifest format:** TOML only.
- **Vocabulary lock:** only `flow`, `feat`, `stack`, `tool`. Never `lifecycle`, `phase`, `goal`, `plugin` (except as passing synonym for `package`).
- **User-owned files (never modified by `vibe install`/`uninstall`):** `spec/boot/00-core.md`, `spec/boot/90-user.md`, `spec/WAL.md`, `VIBEVM-SPEC.md`, `refs/book/**`, any 00-09 or 90-99 boot file.
- **Four project rules (authoritative in [spec://vibevm/common/PROP-000#commits](common/PROP-000.md#commits), copied into `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`):**
  1. **Attribution** — this repository is human-authored; never mark any artefact as machine-authored. The rule itself is the only place AI tooling is mentioned in attribution context.
  2. **Conventional Commits** — short subject, long explanatory body about *why*.
  3. **Group by meaning** — one logical unit per commit; split mixed working trees.
  4. **Autonomy on routine changes only** — commit and push routine work; stop for history rewrites, force-push, large blobs, CI/signing changes, anything whose reversal costs work.
- **M0 registry is local-directory only.** Git registry lands in M1.
- **Work in staging order.** M0 first and complete (done), then M1, then M1.5. No jumping ahead.
- **REVIEW marker discipline:** when the spec is silent, pick the conservative interpretation, mark with `<!-- REVIEW: … -->`, surface in the session report.
- **`refs/` is not committed.** Contents are upstream reference material (book + cloned study repos); kept out of the vibevm distribution both to respect upstream copyright and to keep the repo lean.

## Remotes

- **vibevm source (this repo):** `git@gitverse.ru:anarchic/vibevm.git` (SSH) / `https://gitverse.ru/anarchic/vibevm` (web).
- **Package registry (future, M1+):** `git@gitverse.ru:anarchic/vibespecs.git`.

## Done

- [x] `VIBEVM-SPEC.md` received (v1.0) and read end-to-end.
- [x] Book in `refs/book/` (4 chapters) read end-to-end.
- [x] Reference sources cloned under `refs/src/`: spec-kit, uv, cargo, maven, bazel, tessl-mcp.
- [x] Gitverse SSH access verified.
- [x] Project rules (attribution, commits, push discipline) written verbatim into `CLAUDE.md`, `AGENTS.md`, `GEMINI.md` and authoritatively into [spec://vibevm/common/PROP-000#commits](common/PROP-000.md#commits).
- [x] `VIBEVM-SPEC.md` §7.5 now carries the correct `git@gitverse.ru:anarchic/vibespecs.git` registry URL (was a `github.com/anarchic-org/...` placeholder).
- [x] `git init`, `.gitignore` (including `/refs/`), `LICENSE.md` (proprietary EULA placeholder).
- [x] `spec/boot/00-core.md`, `90-user.md`, `spec/common/PROP-000.md`, `spec/WAL.md` written (dogfood).
- [x] Cargo workspace with 7 crates: `vibe-core`, `vibe-graph`, `vibe-registry`, `vibe-install`, `vibe-cli`, `vibe-llm` (stub), `vibe-check` (stub).
- [x] `vibe-core` (§7, §5.3): `PackageRef` / `PackageKind` / `VersionSpec` with parse/Display/validate. `ProjectManifest`, `PackageManifest`, `Lockfile` schemas with roundtrip tests. `ValueTag` enum for graph edges.
- [x] `vibe-registry`: `LocalRegistry` with resolve/fetch, deterministic `sha256:` content hash, cached-package copy into `.vibe/cache/<kind>/<name>/<version>/`.
- [x] `vibe-install`: plan / apply / register for install, plan / apply / unregister for uninstall. Rejects writes to user-owned paths, escaping paths, and detects both exact and numeric-prefix boot-snippet conflicts.
- [x] `vibe-cli` (`vibe` binary): `vibe init` idempotent scaffold; `vibe install …` with plan → confirm → apply and lockfile update; `vibe list [--kind]`; `vibe uninstall`; `--json` / `--quiet` modes; exit codes per §9.4.
- [x] Hand-written `flow:wal` package at `packages/flow/wal/v0.1.0/`.
- [x] **64 tests green, 0 warnings, 0 clippy warnings** (`cargo test --workspace`, `cargo clippy --workspace --all-targets`).
- [x] M0 acceptance checklist (§16) walked, all 15 items tick.

## In progress

Nothing active. Next session picks up at the start of M1.

## Next

Start of M1:
1. Push `packages/flow/wal/v0.1.0` content to `git@gitverse.ru:anarchic/vibespecs.git` and record the exact push command in `spec/boot/90-user.md`.
2. `vibe-registry` gains git support: clone registry to `~/.vibe/registries/<hash>/`, `git pull` on sync, version directory layout unchanged (`<kind>/<name>/v<semver>/`).
3. New workflows: `vibe update`, `vibe registry sync`, `vibe check`, `vibe show effective`, `vibe show graph`, `vibe show config`.
4. Publish at least two more demo packages to the registry (spec suggests `flow:sync-from-code`, `flow:atomic-commits`).

## Known issues

- **Spec §5.6 `install:update-lockfile` ordering.** If apply partially fails mid-write, the current M0 rolls back already-written files best-effort and does NOT touch the lockfile. Documented behavior; matches "one commit = one logical unit."
- **tessl-mcp** clone was effectively empty. Not blocking; Tessl ideas are covered by its public docs and the book.
- **M0 boot snippet validator** rejects `NN` prefixes outside `10..90` as "reserved range". Matches §6.2, but the error message is terse — could be friendlier in M2 when general error-message polish happens.
- **Path display on Windows** strips `\\?\` UNC prefixes for human-readable output; lockfile stores forward-slash relative paths, so lockfiles are portable across OSes.
- **Line-ending warnings** on every commit — `.gitattributes` with `* text=auto eol=lf` would shut them up. Side-quest in ROADMAP.

## Session context

- **Entry point for next session:** read `CLAUDE.md`, then this WAL, then [spec://vibevm/common/PROP-000](common/PROP-000.md), then pick the first M1 item above.
- **Do NOT touch:** `VIBEVM-SPEC.md` (owner-frozen, only the owner amends), `refs/book/**`, `spec/boot/00-core.md`, `spec/boot/90-user.md`, or the `packages/flow/wal/v0.1.0/` fixture (canonical test payload — changes must be a new version).
- **Key commands to know:**
  - `cargo test --workspace` — all green.
  - `cargo clippy --workspace --all-targets` — clean.
  - `cargo run -p vibe-cli -- init --path <dir>` — hand-run the scaffold.
  - `cargo run -p vibe-cli -- install flow:wal --registry $(pwd)/packages --assume-yes --path <project>` — local install.
  - `git push origin main` — routine push to gitverse (rules 1–4 apply; force-push and history-rewrite need user approval).
