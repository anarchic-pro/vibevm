# Design-rationale specs

This directory holds **design rationale** documents: the *why* and the *lore* behind vibevm's own architectural decisions — the path of a design discussion, the forks weighed and rejected, the precedents studied, the owner's mental model, and the ideas parked for later.

These documents are **non-normative**. The contract — *what* the system does — lives in the PROP / FEAT documents under [`spec/modules/`](../modules/) and [`spec/common/`](../common/). A `spec/design/` document explains *why a PROP is shaped the way it is*. When a design document and its PROP disagree, **the PROP wins** and the design document is corrected.

## Why this genre exists

A PROP is a contract for an implementer: it must be precise, minimal, and readable as a contract. Pouring a full discussion log — every fork, every analogy, every rejected branch — into a PROP makes it unreadable as a contract. But that reasoning is valuable: it is what lets a future session (or a fresh contributor) understand the *intent* without re-deriving it, and avoid re-litigating settled questions.

Industry calls this split RFC-vs-RFC-discussion, or spec-vs-ADR, or code-vs-design-doc. vibevm keeps the **load-bearing** rationale inside each PROP (the `Decision` / `Rejected alternatives` / `Open questions` sections) and moves the **narrative** rationale — the lore — here.

This split mirrors the project's existing reading layers ([`spec/boot/00-core.md`](../boot/00-core.md)): the spec is stable normative memory; this directory is its explanatory companion.

## How `spec/design/` differs from the other `spec/` genres

| Directory | Holds | Normative? |
|---|---|---|
| [`boot/`](../boot/) | Session-boot instructions read at the start of every session | yes |
| [`common/`](../common/) | Foundation decisions crossing every crate (PROP-000, PROP-006) | yes |
| [`modules/`](../modules/) | Per-crate PROP / FEAT — the implementation contract | yes |
| [`research/`](../research/) | Backgrounders on **external** systems (Tessl, threat models, prior-art surveys) | no |
| `design/` (this directory) | Rationale for vibevm's **own** decisions — the why and the lore behind our PROPs | no |
| [`WAL.md`](../WAL.md) | Volatile current-state checkpoint, rewritten each session | n/a |

`research/` and `design/` are both non-normative, but they look in opposite directions: `research/` studies what *other* projects did; `design/` records why *we* chose what we chose.

## When to write a document here

When a design discussion produces more reasoning than a PROP can absorb without losing its contract readability — a multi-fork design session, a large refactor weighed against several alternatives, a decision whose context would otherwise live only in one conversation and be lost at the next session boundary.

## Linking rule

Every `spec/design/` document names the PROP(s) it explains. Every PROP it explains links back to it from its `Related` header — so a session that reads a PROP during the boot sequence finds the rationale without being told it exists. The link is the mechanism that makes the lore survive a cold start.

## Index

- [Workspace & qualified naming](workspace-and-qualified-naming.md) — rationale for [PROP-007](../modules/vibe-workspace/PROP-007-workspace.md) (workspace) and [PROP-008](../modules/vibe-registry/PROP-008-qualified-naming.md) (qualified naming): the owner's Maven-submodules + cargo mental model, the four-axis decomposition, the fork-by-fork decision record, the Cargo-vs-Maven precedent lore, the physical-publication model, and ideas parked for later. Captured 2026-05-20.
- [Loading & boot composition model](loading-and-boot-model.md) — rationale for PROP-009 (loading model): why the flat boot model fails under a workspace, the static/dynamic linking spine, the two-trees + computed-index design, the three inclusion types (`inline` / `static` / `dynamic`) and the `INLINE.md` priority lane, and the fork-by-fork record. Captured 2026-05-21.
