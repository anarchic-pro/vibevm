# vibevm — roadmap

> **Status snapshot (2026-04-17):** M0 walking skeleton is complete and
> published to `git@gitverse.ru:anarchic/vibevm.git`. 64 tests green, 0
> warnings, 0 clippy warnings. The `vibe init → install → list →
> uninstall` loop works end-to-end against a local-directory registry,
> with `flow:wal@0.1.0` as the canonical hand-written package.

This document is the long-form version of `VIBEVM-SPEC.md` §11 (staging
plan). It keeps the "why" and "nuance" that a compressed staging table
cannot carry. `VIBEVM-SPEC.md` remains authoritative on scope; if this
file and the spec disagree, the spec wins and this file is updated.

**Reading order.** Take it top-to-bottom. Each milestone is
self-contained — if work stops after M1, the tool is useful on its own
(a package manager). If work stops after M1.5, the tool is useful on
its own (a package manager + code generator). M2 makes it safe to ship
to other humans. M3 is speculation.

**Non-negotiable rule.** Build in staging order. M0 → M1 → M1.5 → M2.
Do not work on M1.5 before M1 is done; do not start M2 before M1.5 is
done. The temptation to skip to the "shiny" LLM milestone is
particularly strong and must be resisted — the walking-skeleton
discipline is what the whole project is about.

---

## M0 — Walking skeleton ✅ COMPLETE

**Landed.** A `vibe` CLI that scaffolds a project, installs / lists /
uninstalls packages from a local-directory registry, updates the
lockfile, and respects user-owned files. The package model works
end-to-end: hand-written `flow:wal@0.1.0` installs cleanly, uninstalls
cleanly, and user edits in `00-core.md` / `90-user.md` survive both
sides of the cycle.

**Shipped commands.**
- `vibe init [--path] [--name] [--stack]` — idempotent project
  scaffold.
- `vibe install <kind>:<name>[@version] … [--registry] [--assume-yes]`
  — plan → confirm → apply → lockfile update.
- `vibe list [--kind]` — lockfile display as table / `--json` /
  `--quiet` one-liner.
- `vibe uninstall <kind>:<name> [--assume-yes]` — reverse install,
  never touches user-owned files.

**Proven mechanics.**
- TOML manifest parsing with `deny_unknown_fields` everywhere.
- Semver-based package identity with `Latest | Req(VersionReq)`.
- Content-addressed cache under `.vibe/cache/<kind>/<name>/<version>/`
  with deterministic sha256 (forward-slashed relative paths for
  cross-OS stability).
- Boot-snippet conflict detection — both exact filename and numeric
  `NN-` prefix conflicts (matching `VIBEVM-SPEC.md` §6.2 intent).
- User-owned path guards enforced at plan time, not apply time.
- Exit codes per §9.4: 3 for conflict, 5 for declined confirmation.

**Not in M0.** No git registry, no LLM, no build, no sync, no check,
no update, no formal graph runner (workflows are procedural).

---

## M1 — The package manager

**Thesis.** Turn the walking skeleton into a real package manager:
fetch from a git registry, refresh the cache on demand, update
installed packages, lint the project's spec corpus, and give the user
introspection commands.

**Recommended entry point.** Git backend in `vibe-registry`. Without
it, nothing else in M1 has weight — `vibe update` is pointless against
a local dir, `vibe registry sync` is a no-op, and `vibe check`
works fine without a remote. Adding git first means every subsequent
M1 feature ships against a realistic remote from day one.

### M1.1 — Git-backed registry

**What lands.**

- `vibe-registry` gains a `GitRegistry` type that implements the same
  interface as `LocalRegistry` (resolve / list_versions / fetch). The
  existing `LocalRegistry` stays, because M0 tests and demo workflows
  still use it; `--registry <path>` keeps working against a local
  dir.
- First-use clone into `~/.vibe/registries/<hash>/` where `<hash>` is
  a sha256 of the `vibe.toml` `[registry].url`. Subsequent calls do a
  `git pull` unless the cache is fresh (cache-freshness window TBD,
  initially 1 hour).
- Sparse-checkout is *not* in scope for M1 — clone the whole
  registry. Optimisation lands in M2 if the registry gets large.
- The `[registry]` section in `vibe.toml` starts actually being read
  (M0 only had a `--registry` override).
- Source URI format in the lockfile switches from `file:///…` to
  `git+ssh://git@gitverse.ru/anarchic/vibespecs.git#<kind>/<name>/v<ver>`
  when the package came from a git registry.

**Decisions required during this slice.**

- `git2` crate vs shelling out to `git`? `git2` adds a hefty
  dependency (libgit2 bindings). Shelling out relies on `git` being on
  `PATH` but keeps the dep tree small. Default: try `git2` first; if
  build complexity or binary size gets ugly, swap to shell.
- Authentication: SSH-agent is the default. Token-based HTTPS auth is
  M2 scope (private registries). For M1, document that the user must
  have a working SSH identity that the git backend can discover.

### M1.2 — `vibe update`

- `vibe update <pkgref>` and `vibe update --all`: re-fetch the
  registry (if stale), re-resolve the package, if a newer version
  satisfies the original constraint show a diff (file list adds /
  removes / modifies), confirm, apply.
- File-modification case: if a file already exists in the project and
  its content is identical to the previous cached version, overwrite.
  If the user has edited it locally, refuse and show a 3-way diff
  guidance message.
- Lockfile updated per usual.

### M1.3 — `vibe check` (spec linter)

Implements the full `VIBEVM-SPEC.md` §12 check list:
1. Manifest validity (`vibe.toml`, `vibe.lock` parse and match schema).
2. Dead `spec://` references.
3. Orphan `{#anchor}`s.
4. Anchor uniqueness within a spec file.
5. WAL freshness (modification timestamp < 24h, warn if older).
6. WAL well-formedness (required sections present).
7. Boot directory consistency (NN-name.md pattern, no number clashes).
8. Lockfile consistency (no orphan files in `spec/flows/*` etc.).
9. REVIEW marker aging (default 14-day threshold).
10. Implementation coverage (files with `build` history carry
    `Implements: spec://…` markers). This last check becomes
    meaningful only after M1.5 ships — in M1 it can be a warning-only
    noop.

`vibe check --fix` is a narrow subset: remove dead anchor references
we can identify safely, nothing that loses information.

### M1.4 — `vibe show …`

Pure inspection, no mutation:
- `vibe show effective` — materialize the full spec corpus as one
  stream, with provenance (which package contributed what). The
  `EffectiveSpec` typed value from §5.3 finally gets a consumer.
- `vibe show graph [<workflow>]` — textual render of the task graph.
  Helps debug the install subgraph and, later, build.
- `vibe show node <name>` — details of a single node (inputs,
  outputs, cacheability).
- `vibe show config` — effective configuration with provenance (which
  flag / env var / vibe.toml value won).
- `vibe show plan <workflow> [args...]` — dry-run. Prints what would
  happen without executing.

### M1.5-gate — registry publish

Before cutting the M1 tag:
- `packages/flow/wal/v0.1.0/` gets pushed to
  `git@gitverse.ru:anarchic/vibespecs.git` as the first real entry.
- Two more demo packages land as stretch content, both hand-written:
  `flow:sync-from-code` (derived from book chapter 3) and
  `flow:atomic-commits` (derived from book chapter 2). They prove the
  registry holds multiple packages, and they exercise numeric-prefix
  collision detection (one flow picks `20-…`, the next `30-…`).
- Docs: `docs/commands/*.md` for every user-facing command;
  `docs/authoring-flow.md`, `docs/authoring-feat.md`,
  `docs/authoring-stack.md` for package authors.

### M1 acceptance (from §16 of the spec)

- [ ] `vibe install` resolves packages from git per `vibe.toml`.
- [ ] Registry cache lives at `~/.vibe/registries/<hash>/`.
- [ ] `vibe registry sync` refreshes.
- [ ] `vibe update <pkgref>` and `--all` work with diff display.
- [ ] `vibe check` runs every §12 check.
- [ ] `vibe check --fix` autofixes only safe issues.
- [ ] `vibe show effective` / `graph` / `config` all produce useful
      output.
- [ ] Public registry on GitVerse with ≥ 3 packages.
- [ ] Documentation in `docs/` covers every command plus authoring
      guide per kind.

**Estimated effort.** 2–4 weekends. The git backend is the biggest
lift; the rest is straightforward with `vibe-core` already in place.

---

## M1.5 — Generation

**Thesis.** vibevm earns its tagline — "the disciplined runtime for
spec-driven vibecoding" — only when it can actually produce working
code from a `feat × stack` pairing. This milestone is where the tool
makes the jump from "manages specs" to "produces software."

### M1.5.1 — LLM provider abstraction

- `vibe-llm` gets real. `LLMProvider` trait with methods `chat` and
  `chat_with_tools`. First implementation: Anthropic via the Messages
  API.
- `ProviderConfig` read from `vibe.toml` `[llm]` section: default
  provider, default model, `api_key_env`. Per-step overrides (`[llm.build]`,
  `[llm.sync]`) supported per spec §7.5.
- Streaming (`stream_chat`) is out of scope for M1.5; add in M2 when
  CLI output polish lands.
- OpenAI, OpenRouter, Ollama providers land in a second slice after
  Anthropic works — they all share the Messages-or-ChatCompletions
  shape plus a tool-use loop, so the incremental cost per provider is
  small.

### M1.5.2 — Tool-use loop

- The build loop (pseudocode in spec §10.4) runs against an explicit
  tool set: `read_file`, `write_file`, `list_dir`, `run_test`,
  `run_shell` (restricted to a short allowlist). Every tool
  invocation is sandboxed to project root — no `..` escape, no
  absolute-path reads.
- Tool-use traces are recorded for debugging and cost reporting.

### M1.5.3 — `vibe build`

- `vibe build <feat-pkgref> [--stack <name>]`. Loads the effective
  spec (all active flows + active stack + the named feat + WAL),
  invokes the LLM to produce a `BuildPlan`, asks for confirmation,
  then runs the tool-use loop to generate code files.
- Generated code carries `// Implements: spec://…` markers so `vibe
  check`'s implementation-coverage check can verify traceability.
- `vibe build --with-install` composes install + build for the
  fast-prototyping path.

### M1.5.4 — `vibe sync` (Sync-from-Code)

- Per book chapter 3's Sync-from-Code protocol: detect `git diff
  HEAD` changes to code, ask the LLM to summarise intent, propose
  corresponding spec updates, show the user, apply on approval.
- Pure reconciliation — never rewrites code to match stale spec; that
  direction is `vibe build` territory.

### M1.5.5 — Working example

- `stack:rust-cli@0.1.0` (hand-written) published to the registry.
- `feat:welcome-page@0.1.0` (hand-written).
- `vibe init → install stack:rust-cli → install feat:welcome-page →
  build feat:welcome-page --stack rust-cli` produces a running Rust
  CLI that prints a welcome page. This is the M1.5 demo.

### M1.5 acceptance (from §16)

- [ ] LLM provider abstraction supports Anthropic + OpenAI +
      OpenRouter + Ollama.
- [ ] `vibe build` produces working code from `feat:welcome-page ×
      stack:rust-cli`.
- [ ] Generated code has `Implements: spec://…` markers.
- [ ] Build subgraph respects `user-confirm` before mutation.
- [ ] `vibe sync` produces a clean spec-delta proposal from a code
      change.
- [ ] Tool-use loops are sandboxed to project root.
- [ ] LLM API errors surfaced clearly.
- [ ] LLM costs reported in the build's structured output.

**Estimated effort.** 3–6 weekends. Tool-use loops need real-world
hardening — the first working version is not the shippable version.

---

## M2 — Production-readiness

**Thesis.** Everything needed for someone other than the author to
use vibevm safely. Up through M1.5, the author is the only user and
"it works on my machine" is acceptable. M2 closes that gap.

### M2.1 — LLM-based install review

- `install:review` stops being a no-op. Before applying writes, the
  LLM reviews the fetched package contents and emits a safety
  analysis: does this look benign? does it try to exfiltrate
  anything? is it doing something inconsistent with what the
  manifest claims?
- The user sees both the mechanical plan and the semantic review
  before confirming. If the review flags a concern, confirmation
  requires an explicit `--accept-review` flag (never silent).

### M2.2 — Plugin contribution model v2

- Packages gain the ability to contribute actual graph nodes, not
  just files. A `flow:wal` package gets to register a
  `wal:checkpoint` node that runs automatically after
  `build:compile`. This is the point where `vibe-graph` earns the
  runner sophistication §5.2 hints at.
- Tooling to author and test contributed nodes.
- Type-checking at graph-build time gets teeth — type mismatches
  reject the graph with an `EXIT 4` before any mutation runs.

### M2.3 — Private registries

- Token-based authentication for `[registry]` URLs. `api_key_env`
  pattern extended to `token_env`.
- Per-registry cache keys so tokens don't leak across registries.

### M2.4 — Cross-platform CI

- GitHub Actions (or equivalent on GitVerse if available) matrix:
  macOS / Ubuntu / Windows, stable Rust.
- Pre-built binaries per platform on tag. Homebrew formula.
  Scoop manifest for Windows.

### M2.5 — Error-message polish

- Every user-facing error carries: what went wrong, where (file +
  line if applicable), and what to do about it.
- `vibe doctor` — inspects a project and reports common issues: WAL
  staleness, orphan anchors, missing implements-markers, registry
  cache older than N days.
- Colour/glyph output refined with a `--no-color` escape hatch.

### M2.6 — Structured telemetry (optional)

- Opt-in (`[telemetry] enabled = false` by default). Reports crash
  frequencies and common error paths. Gives the author signal on
  what to harden next.

**No M2 acceptance list in the spec** — §11.4 says "open-ended;
depends on adoption signals." Treat M2 as a rolling quality bar.

---

## M3+ — Speculative directions

None of these are funded. They are listed so the M0 / M1 / M1.5 /
M2 decisions keep these futures open rather than foreclosing them.

- **Interpret mode.** `vibe run <feat-pkgref>` executes the spec
  directly via an LLM runtime — no code generation. Useful for
  one-shot scripts and for exploring a feat before committing it to
  a stack.
- **Multi-stack composition.** One feat compiled for multiple stacks
  simultaneously (e.g. a UI feat for web + mobile). Requires the
  stack abstraction to be richer than the current §4.1.
- **Skill layer.** Distributable Claude Code / Codex / OpenCode skills
  that wrap the CLI for native slash-command access, so users don't
  have to leave their editor.
- **Hosted registry.** Replace git-as-registry with a proper package
  registry server: metadata index, search, signed publishes, a web
  UI. Only worth building if the community shape signals it.

---

## Side quests (independent of milestones)

These are small-to-medium polish items that are not on the critical
path of any milestone. Take them when a session has 30–60 free minutes
and you want to close a loop that's bugging you.

- **`.gitattributes`** with `* text=auto eol=lf`. The M0 commits
  produced 70+ "LF will be replaced by CRLF" warnings because the
  repo doesn't pin a line-ending policy. Left unchecked this
  eventually causes content-hash drift on Windows. Fix once, forget.
- **`git config gc.auto 0`** on the repo. The book (chapter 4) warns
  that Git's automatic garbage collector can fire mid-session and
  corrupt worktree indexes. Disable auto-gc and document a manual
  `git gc --prune=now` after each big commit burst.
- **Workspace README.md.** A top-level README explaining what vibevm
  is, how to build, where to start reading, how to contribute. Right
  now the project has `VIBEVM-SPEC.md` (spec) and `ROADMAP.md` (this
  file) but nothing for a first-time visitor landing on the repo
  page.
- **CHANGELOG.md.** Conventional Commits make this trivially
  generable. Nice for M1 onward when external users start tracking
  versions.
- **Clippy lint set promotion.** Upgrade `clippy::all` to `-D` (deny)
  and pick a tighter lint set (`clippy::pedantic` selectively) for
  the library crates. Warnings-as-errors in CI.
- **`cargo deny` in CI.** Licence-check automated: fail the build if a
  dep with a non-permissive licence sneaks in. Matches PROP-000 §3's
  "permissive only" rule.
- **Docs site.** Eventually `https://gitverse.ru/anarchic/vibevm` is
  enough — but once user-facing docs exist under `docs/`, render them
  through mdBook or Zola so URIs are clickable.

## Known outstanding review items

Nothing active. Historical:

- `vibe-install/src/lib.rs` carried a REVIEW about mirror package
  layout — resolved 2026-04-17 by pinning the convention in
  `VIBEVM-SPEC.md` §13.1 and in `PROP-000` §13.

---

## Cadence and review

- **Per milestone:** walk the acceptance checklist in §16 of the
  spec. If any item fails, fix before claiming completion. Tag the
  release (`v0.1.0-m0`, `v0.1.0-m1`, etc.) and update `spec/WAL.md`
  to reflect the new "Current phase."
- **Per session:** read `CLAUDE.md`, then `spec/WAL.md`, then the
  relevant PROP/FEAT for the task at hand. Update the WAL at session
  end. Commit in grouped units per `CLAUDE.md` Rule 3.
- **Per week:** re-read the spec sections relevant to the active
  milestone. Catch drift before it hardens.

---

*End of roadmap.*
