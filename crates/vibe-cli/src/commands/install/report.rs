//! Presentation for `vibe install` — the plan listing and the outcome
//! / fresh-path envelopes. Pure rendering over the orchestrator's
//! types; nothing here mutates state.

specmark::scope!("spec://vibevm/VIBEVM-SPEC#install-workflow-in-detail");

use anyhow::Result;
use serde::Serialize;
use vibe_workspace::install::{InstallOutcome, ResolvedDep};

use crate::output;

pub(super) fn present_resolution(ctx: &output::Context, resolution: &[ResolvedDep]) {
    if ctx.is_json() {
        #[derive(Serialize)]
        struct PlanEntry {
            package: String,
            version: String,
        }
        let payload: Vec<PlanEntry> = resolution
            .iter()
            .map(|d| PlanEntry {
                package: format!("{}/{}", d.group, d.name),
                version: d.version.to_string(),
            })
            .collect();
        let _ = ctx.emit_json(&serde_json::json!({
            "command": "install:plan",
            "packages": payload,
        }));
        return;
    }
    if ctx.is_quiet() {
        return;
    }
    ctx.heading(&format!(
        "\nMaterialising {} package{} into vibedeps/:",
        resolution.len(),
        if resolution.len() == 1 { "" } else { "s" },
    ));
    for d in resolution {
        println!("  {}/{}@{}", d.group, d.name, d.version);
    }
    println!();
}

pub(super) fn emit_report(ctx: &output::Context, outcome: &InstallOutcome) -> Result<()> {
    if ctx.is_json() {
        ctx.emit_json(&serde_json::json!({
            "ok": true,
            "command": "install",
            "materialised": outcome.materialised,
            "skipped": outcome.skipped,
            "pruned": outcome.pruned,
            "nodes_regenerated": outcome.nodes_regenerated,
        }))?;
        return Ok(());
    }
    if ctx.is_quiet() {
        ctx.summary(&format!(
            "vibe install: {} package{} materialised",
            outcome.materialised.len(),
            if outcome.materialised.len() == 1 {
                ""
            } else {
                "s"
            },
        ));
        return Ok(());
    }
    ctx.summary(&format!(
        "\nMaterialised {} package{} into vibedeps/; regenerated boot artifacts for {} node{}.",
        outcome.materialised.len(),
        if outcome.materialised.len() == 1 {
            ""
        } else {
            "s"
        },
        outcome.nodes_regenerated.len(),
        if outcome.nodes_regenerated.len() == 1 {
            ""
        } else {
            "s"
        },
    ));
    if !outcome.skipped.is_empty() {
        ctx.step(&format!(
            "{} slot{} already present — re-copy skipped (PROP-011 §2.3)",
            outcome.skipped.len(),
            if outcome.skipped.len() == 1 { "" } else { "s" },
        ));
    }
    if !outcome.pruned.is_empty() {
        ctx.step(&format!(
            "pruned {} stale vibedeps/ slot{}",
            outcome.pruned.len(),
            if outcome.pruned.len() == 1 { "" } else { "s" },
        ));
    }
    Ok(())
}

/// Report the PROP-011 §2.2 fast path — `vibe.lock` was fresh, so no
/// resolution ran. Kept distinct from [`emit_report`] so the operator can
/// tell a no-op `vibe install` from one that materialised packages.
pub(super) fn emit_fresh_report(ctx: &output::Context, nodes_regenerated: &[String]) -> Result<()> {
    if ctx.is_json() {
        ctx.emit_json(&serde_json::json!({
            "ok": true,
            "command": "install",
            "unchanged": true,
            "nodes_regenerated": nodes_regenerated,
        }))?;
        return Ok(());
    }
    ctx.summary(&format!(
        "vibe install: vibe.lock unchanged — nothing to re-resolve ({} node{} up to date)",
        nodes_regenerated.len(),
        if nodes_regenerated.len() == 1 {
            ""
        } else {
            "s"
        },
    ));
    Ok(())
}
