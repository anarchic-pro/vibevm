# vibevm — read this first

Every session in this repository begins by reading this file, then every file in `spec/boot/` in filename order, then `spec/WAL.md`, then any relevant PROP/FEAT documents under `spec/common/` and `spec/modules/` for the task at hand. Only after that, start work.

The four rules below are non-negotiable and apply to every session. Authoritative reference: [spec://vibevm/common/PROP-000#commits](spec/common/PROP-000.md#commits).

## Rule 1 — Attribution: keep this repository human-authored

Never attribute authorship of any part of this repository to an artificial-intelligence (AI) or neural-network–based system of any kind — no commit messages, no Git trailers (`Co-Authored-By`, `Signed-off-by: <model-name>`, etc.), no branch names, no worktree branch names, no code comments, no README lines, no release notes. This applies to every such system regardless of provider, model family, or whether it is local or cloud-hosted.

**Why:** Some jurisdictions regulate or criminalize machine authorship of software. We are not currently subject to such regulation and are not violating any present law; the owner chooses this policy proactively so that any future regulation finds no hook on this project. The surface this repository presents is: a human wrote this code, full stop.

**This paragraph (and its copy in [PROP-000 §12.1](spec/common/PROP-000.md#commits)) is the single place in the entire project where AI tooling is discussed in the attribution sense.** Everywhere else — commits, code, docs, branches, CI, signing — assume human authorship only. `VIBEVM-SPEC.md` discusses AI integration as a feature of the vibevm product (what the tool does for its users); that is product scope, not attribution, and is not covered by this rule.

## Rule 2 — Conventional Commits

Every commit follows the [Conventional Commits](https://www.conventionalcommits.org/) specification.

```
type(scope): short imperative subject line

Longer body — a sentence, a paragraph, or a mini-article depending
on how much reasoning the change carries. Explain WHY this change
was made and what follows from it. The diff already shows what
changed; the value of the commit message is the reasoning and the
downstream consequences that a future reader cannot reconstruct
from the diff alone.

Cite `spec://…` URIs where relevant.
```

- Keep the subject short (target ≤ 60 characters, hard limit 72) so Git web UIs render it without truncation.
- Body is free-form; prefer paragraphs over bullet lists when reasoning is continuous.
- `type` is one of `feat`, `fix`, `chore`, `docs`, `build`, `test`, `refactor`, `perf`, `style`, `ci`, `revert`.
- `scope` names the most affected crate, package, or subsystem (e.g. `core`, `install`, `wal`, `registry`, `spec`).

## Rule 3 — Group commits by meaning

When the working tree carries changes spanning multiple concerns, split them into separate commits grouped by topic — never by file name or time of edit. Each commit is one logical unit. A working set containing "fix typo in README" + "refactor the planner" + "update the manifest schema" is **three** commits, not one.

## Rule 4 — Autonomy on routine changes only

Routine large changes — implementing a planned milestone, finishing a feature slice, touching many files for one coherent reason — may be committed and pushed without first asking the user, using rules 1–3.

Stop and ask the user first for anything **non-routine**:

- rewriting published history (rebase of pushed commits, `git commit --amend` of pushed work),
- `git push --force` or `--force-with-lease`,
- bringing in large binary blobs,
- changing CI, signing, or secrets configuration,
- any operation whose reversal would cost work.

When uncertain, ask.

## Memory discipline: project facts stay in the project

Facts about *this project* — its design, conventions, decisions, milestones, open questions, owner preferences that govern technology choices — live **inside this repository**. The canonical homes are:

- `CLAUDE.md` / `AGENTS.md` / `GEMINI.md` (kept identical; the four rules and the few directives that must hit every harness on session boot).
- `MEMORY.md` at repo root (currently a pointer to [`spec/boot/90-user.md`](spec/boot/90-user.md), the user-owned boot snippet).
- `TASKS.md` at repo root, if one is warranted (not present today).
- Authoritatively, the `spec/**/*.md` tree — PROP / FEAT documents, `spec/WAL.md`, `spec/boot/*`.

Project facts do **not** belong in the running harness's global per-user auto-memory (whatever tool-specific path that happens to be). A teammate who clones the repo will never see global user-memory, and anything they need to know about the project must live in the repo.

Global user-memory is reserved for facts about *this developer's machine* — shell quirks, SSH-agent setup on this box, installed-tool specifics that persist across sessions but are not universal.

**Default:** when uncertain whether a fact is project-scoped or machine-scoped, treat it as project-scoped and write it into the repo. Moving a fact from the project into user-memory later is cheap; the reverse has already silently cost a teammate context.
