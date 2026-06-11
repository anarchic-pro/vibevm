# spec/neworder — shim

`spec/neworder/` used to hold the discipline package drops. As of the
v0.3 adoption (Phase 0) the Discipline is **installed as vibevm
packages** and this directory is a pointer, nothing more.

## Where everything lives now

| What | Where |
|---|---|
| The Discipline product (manifesto, card format, scaffold catalog, raid playbook, cards, appendix) | package `flow:org.vibevm/discipline-core` — source `packages/org.vibevm/discipline-core/v0.2.0/`, installed slot `vibedeps/flow-discipline-core/v0.2.0/` |
| The Rust projection (AI-Native Rust guide, vibe-tcg tool spec) | package `stack:org.vibevm/rust-ai-native` — source `packages/org.vibevm/rust-ai-native/v0.2.0/`, installed slot `vibedeps/stack-rust-ai-native/v0.2.0/` |
| Retained mechanisms (PROP-014 specmap, BROWNFIELD protocol, ENGINE-CONFORM, LEDGER-INTENT) | [`spec/discipline/`](../discipline/) — vibevm-hosted mechanism specs, code-anchored via `spec://vibevm/discipline/…` |
| The vibevm-specific adoption plan | [`spec/terraforms/TERRAFORM-PLAN-v0.3.md`](../terraforms/TERRAFORM-PLAN-v0.3.md) |
| Adoption working state (raid log, prediction ledger) | `terraform/adopt-v0.3/` |
| Discipline version pin | `vibevm.discipline.lock` (repo root) |

## Reinstall recipe

The packages resolve from the in-repo local registry:

```sh
vibe install flow:org.vibevm/discipline-core stack:org.vibevm/rust-ai-native --registry ./packages --assume-yes
```

Publishing them to the public `vibespecs` registry is an owner-gated
step (token, outward-facing) and has not been performed.

## Carried-over notes from the v0.2-beta drop README

- Legacy per-language projections (`GUIDE-{TYPESCRIPT,PYTHON,GO,JAVA*,CPP*,KOTLIN}-v0.1`)
  travel inside `discipline-core` under `legacy-projections/`. They are
  v0.1-era material: Rust is the pilot language; other languages are
  re-projected after the vibevm pilot validates the v0.2 shape.
  Their known beta gaps (C++ profile-composition semantics, Java
  trunk+overlay computed rule sets, the TS guide's missing
  boundary-validation paragraph) remain open in those files.
- Pending PROP-014 amendment: external read-only normative namespaces
  (`misra://cpp2008/<rule>`) — code may `deviates` such units, never
  `implements`. Still pending; tracked in `spec/WAL.md`.
