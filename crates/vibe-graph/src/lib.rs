//! Task-graph builder and runner.
//!
//! Workflows are graph queries: each user-facing command (`init`, `install`,
//! `list`, …) maps to a target node, and the runner walks the transitive
//! dependency closure of that node.
//!
//! Spec: `VIBEVM-SPEC.md` §5.

#![forbid(unsafe_code)]
