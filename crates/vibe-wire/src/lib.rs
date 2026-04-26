//! `vibe-wire` — generated Rust types for vibevm wire contracts.
//!
//! Every type under [`generated`] is **machine-generated** from a JTD
//! schema in [`schemas/`](../../../schemas/) at the repo root. Source of truth lives
//! there; this crate carries the codegen output verbatim. `cargo
//! xtask codegen` regenerates; `cargo xtask check-codegen` asserts no
//! drift (CI runs the latter). Per PROP-000 §16, JTD + codegen is the
//! standing pattern for wire contracts in this project.
//!
//! See [`tools/jtd-codegen/README.md`](../../../tools/jtd-codegen/README.md) for the
//! generator install procedure and pinned version.
//!
//! Today the crate is **scaffold-only** — `cargo xtask codegen` is
//! wired up, schemas exist under `schemas/`, but the generator binary
//! is not committed. The first time a developer runs `cargo xtask
//! codegen` on a populated `tools/jtd-codegen/`, files appear under
//! [`generated`]. Until then, this module is empty and the rest of
//! the workspace is untouched. Migration of existing hand-written
//! `Serialize` structs to JTD-derived types lands incrementally.

#![forbid(unsafe_code)]

/// Generated wire types. Populated by `cargo xtask codegen`.
pub mod generated {
    // Generated files appear here after running the codegen task.
    // Each entry is its own submodule; all submodules are re-exported
    // here unchanged. Drift between `schemas/` and this directory is
    // surfaced by `cargo xtask check-codegen` in CI.
}
