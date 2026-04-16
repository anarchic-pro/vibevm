//! The `install` / `uninstall` / `update` workflows: plan → user-confirm
//! → apply → update-lockfile → report. Mutating nodes run only after an
//! `Approval`. The implementation lands in a follow-up commit.
//!
//! Spec: `VIBEVM-SPEC.md` §5.6, §6, §11.1.

#![forbid(unsafe_code)]
