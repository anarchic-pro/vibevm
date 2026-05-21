//! `vibe update [<pkgref>...] [--all]` — re-resolve and re-materialise.
//!
//! In the PROP-009 loading model, `vibe update` re-resolves the project's
//! `[requires]` afresh — the depsolver picks the newest version within
//! each declared constraint — and re-materialises the result into
//! `vibedeps/`, regenerating the boot artifacts. That is exactly the
//! `vibe install` path run from the manifest, so `vibe update` delegates
//! to it.
//!
//! v1 limitation: named pkgrefs and `--all` do not yet scope the update —
//! the whole declared graph is re-resolved either way. Per-package-scoped
//! update (bump one, hold the rest at their lockfile pins) is a follow-up.
//!
//! Spec: spec://vibevm/modules/vibe-workspace/PROP-009-loading-model.

use anyhow::Result;

use crate::cli::{InstallArgs, UpdateArgs};
use crate::output;

pub fn run(ctx: &output::Context, args: UpdateArgs) -> Result<()> {
    // Re-resolve every declared dependency: an empty `packages` list puts
    // `vibe install` into from-manifest mode, which re-runs the depsolver
    // over `[requires]` and re-materialises the result.
    let install_args = InstallArgs {
        packages: Vec::new(),
        path: args.path,
        registry: None,
        assume_yes: args.assume_yes,
        language: None,
        features: Vec::new(),
        no_default_features: false,
        all_features: false,
        exact: args.exact,
        auth_required: args.auth_required,
        git: None,
        tag: None,
        branch: None,
        rev: None,
        git_auth: None,
        git_token_env: None,
    };
    super::install::run(ctx, install_args)
}
