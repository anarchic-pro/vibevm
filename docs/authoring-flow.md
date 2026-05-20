# Authoring a `flow` package

A **flow** is a discipline / process module — a set of conventions, protocols, and reminders an AI agent reads at session start so it follows the team's working agreements. Flows are the "how we work" half of vibevm; feats and stacks are the "what we build" half.

Examples shipped today:
- `flow:wal` — the Write-Ahead Log discipline.
- `flow:sync-from-code` — the protocol for reconciling spec drift from code.
- `flow:atomic-commits` — one-commit-per-idea + Conventional Commits format.

A flow is *content*. There is no executable component, no LLM call, no build artefact — at install time a flow's files land verbatim in the consumer project's `spec/flows/<name>/` tree, and a single boot snippet is added to `spec/boot/` so the AI session reads it first.

## Anatomy of a flow package

```
flow-<name>/                 # the per-package repo on the registry
├── vibe.toml                # required; manifest, carries a [package] table
├── README.md                # required; human-readable description
├── boot/
│   └── <prefix>-flow-<name>.md   # the boot-snippet content
└── spec/
    └── flows/
        └── <name>/
            ├── PROTOCOL.md         # canonical "what is this discipline"
            ├── <subprotocol-1>.md  # supporting docs, broken out by topic
            └── <subprotocol-2>.md
```

After `vibe install flow:<name>`, the consumer's tree has:

```
<consumer>/
├── spec/
│   ├── boot/<prefix>-flow-<name>.md     # mirror — same content
│   └── flows/<name>/                     # mirror — same content
│       ├── PROTOCOL.md
│       └── …
```

This is the **mirror layout** ([`VIBEVM-SPEC.md` §13.1](../VIBEVM-SPEC.md)): every entry under `[writes].files` in the manifest is simultaneously the path inside the package and the path it will appear at in the consumer project. No path mapping. The boot snippet is the single exception — it has an explicit `[boot_snippet]` section because its target is fixed to `spec/boot/<filename>` regardless of how it's organised inside the package.

## The boot-snippet prefix

Every flow's boot snippet is named `<NN>-flow-<name>.md`, where `<NN>` is a two-digit prefix that determines read order at session start. The numbering convention from [`VIBEVM-SPEC.md` §6.2](../VIBEVM-SPEC.md):

| Range | Reserved for |
| --- | --- |
| `00`-`09` | User-owned (`vibe install` never writes here). |
| `10`-`89` | Package-contributed snippets. Flows traditionally start at `10` and go up. |
| `90`-`99` | User-owned overrides (`vibe install` never writes here). |

Pick a prefix that doesn't clash with anything you intend to install alongside. Today's three demo flows use `10-` (`flow:wal`), `20-` (`flow:sync-from-code`), `30-` (`flow:atomic-commits`). A new flow that's meant to coexist with these picks `40-` or higher.

`vibe install` rejects with exit code `3` if two installs would land at the same `NN-` prefix.

## Manifest: `vibe.toml`

A publishable package carries a `vibe.toml` with a `[package]` table. Minimal:

```toml
[package]
name = "atomic-commits"
kind = "flow"
version = "0.1.0"
authors = ["You <you@example.com>"]
license = "EULA"
description = "One commit = one idea. Conventional Commits format."
keywords = ["commits", "discipline", "conventional-commits"]

[compatibility]
min_vibe_version = "0.1.0"

[writes]
files = [
    "spec/flows/atomic-commits/PROTOCOL.md",
    "spec/flows/atomic-commits/conventional-commits.md",
    "spec/flows/atomic-commits/splitting-large-changes.md",
]

[boot_snippet]
filename = "30-flow-atomic-commits.md"
source = "boot/30-flow-atomic-commits.md"
```

`[provides]`, `[requires]`, `[[requires_any]]`, `[obsoletes]`, `[conflicts]` are all optional. A typical flow has none of them — it's self-contained content. Use them when:

- Your flow advertises a capability another package may consume (`[provides].capabilities`).
- Your flow assumes the project also follows another flow it depends on (`[requires].packages`).
- Your flow supersedes an older one (`[obsoletes].packages`).

See [`VIBEVM-SPEC.md` §7.3](../VIBEVM-SPEC.md) for the full manifest schema.

## Writing the boot snippet

The boot snippet is the *only* file from your package that the AI agent is guaranteed to read every session — `spec/boot/` is loaded in filename order before anything else. Treat it as the front page.

Good boot snippet shape:

1. **One sentence describing the discipline** — what does this flow ask of the agent?
2. **A pointer to `PROTOCOL.md`** — `For the full protocol, see spec://<project>/flows/<name>/PROTOCOL`.
3. **The non-negotiable rules**, terse and numbered.
4. **Any links to subprotocols** worth pulling in for specific tasks.

Keep it under ~80 lines. Boot snippets compete for the agent's attention budget; brevity wins.

## Writing the protocol

`spec/flows/<name>/PROTOCOL.md` carries the full discipline. Sections worth thinking about:

- **What problem this flow solves** — why does the team adopt it?
- **The protocol** — concrete steps, with examples.
- **Anti-patterns** — what this flow forbids and why.
- **Edge cases** — recurring questions and the agreed answers.
- **References** — books, prior art, original sources.

Subprotocols (`spec/flows/<name>/<topic>.md`) hold detail for specific situations — pull them out when `PROTOCOL.md` would otherwise grow past comfortable reading length.

## Versioning

Versions are git tags on the per-package repo, prefixed `v` (e.g. `v0.1.0`). Bump rules follow [SemVer](https://semver.org):

- **Patch** (`0.1.0` → `0.1.1`): wording fixes, typo corrections, no semantic change to the protocol.
- **Minor** (`0.1.0` → `0.2.0`): additive changes — new optional subsections, new examples, expanded protocol that doesn't remove existing rules.
- **Major** (`0.1.0` → `1.0.0`): breaking change — rules removed or replaced, prefix renumber, `[requires]` added.

Pre-1.0 (`0.x`), even minor bumps may break consumers — that's the SemVer convention for unstable APIs.

## Publishing

Once your package directory has a manifest, README, boot snippet, and content files, publish through the maintainer command:

```bash
vibe registry publish ./path/to/your/flow-package
```

See [`vibe registry publish`](commands/registry-publish.md) for the full token / authentication / error model. The first publish creates the per-package repo under your registry's organization; subsequent versions reuse it and push new tags.

## Tips

- **Read existing flows first.** [`flow:wal`](https://gitverse.ru/anarchic/vibespecs) is short and well-formed; use it as a structural template.
- **Test with the local-fixture path before publishing.** `vibe install flow:<name> --registry ./path/to/your/dir` against a directory laid out in the M0 monorepo shape lets you iterate without going through `git push` round-trips.
- **Boot snippets are user-facing prose.** Write them like you're briefing a teammate. The AI agent is the immediate reader, but humans review the boot files; make both happy.
- **Don't put runtime logic in a flow.** A flow is content. If your idea needs a build step or a tool invocation, it's a `feat` or a `tool`, not a `flow`.

## Related

- [`vibe install`](commands/install.md) — installing a flow.
- [`VIBEVM-SPEC.md` §6](../VIBEVM-SPEC.md) — the boot directory model.
- [authoring-feat.md](authoring-feat.md) — feats are the runtime-y counterpart of flows.
