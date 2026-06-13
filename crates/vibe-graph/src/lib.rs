//! Task-graph builder and runner.
//!
//! Workflows are graph queries: each user-facing command (`init`, `install`,
//! `list`, …) maps to a target node, and the runner walks the transitive
//! dependency closure of that node.
//!
//! **STATUS: M0 stub.** The runtime task-graph builder/runner specified in
//! `VIBEVM-SPEC.md` §5 is not built yet — today each command runs its logic
//! directly rather than as a graph query. This crate is a deliberate
//! placeholder for that milestone, not forgotten work.
//!
//! Spec: `VIBEVM-SPEC.md` §5.

#![forbid(unsafe_code)]
