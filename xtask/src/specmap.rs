//! `cargo xtask specmap` — regenerate (or `--check`) the canonical
//! `specmap.json` traceability index (PROP-014 §2.5), plus the Phase 2
//! orphan ratchet gate that rides every run.

use anyhow::{Result, bail};

use crate::repo_root;

pub(crate) fn run_specmap(check: bool) -> Result<()> {
    let root = repo_root()?;
    if check {
        match specmap_core::index::check(&root)? {
            Ok(summary) => {
                eprintln!("xtask specmap --check: clean ({summary}).");
            }
            Err(msg) => bail!("{msg}"),
        }
    } else {
        let (path, summary) = specmap_core::index::write(&root)?;
        eprintln!("xtask specmap: wrote {} ({summary}).", path.display());
    }
    run_ratchet_gate(&root, check)
}

/// The Phase 2 ratchet: the orphan gate over non-exempt crates
/// (PLAYBOOK #phase2 "flip the ratchet"). Reported in both modes;
/// blocking only under `--check`. Absent ratchet file = gate off.
fn run_ratchet_gate(root: &std::path::Path, blocking: bool) -> Result<()> {
    let Some(ratchet) = specmap_core::ratchet::load(root)? else {
        return Ok(());
    };
    let map = specmap_core::index::build(root);
    let orphans = specmap_core::ratchet::orphans(root, &map, &ratchet);
    let mut blockers = 0usize;
    for o in &orphans {
        match &o.disposition {
            Some(debt) => eprintln!(
                "  ratchet: orphan dispositioned ({debt}): `{}` ({}) at {}:{}",
                o.symbol, o.item_kind, o.file, o.line
            ),
            None => {
                blockers += 1;
                eprintln!(
                    "  ratchet: ORPHAN `{}` ({}) at {}:{} — tag it, scope! its module, \
                     or disposition it in specmap-ratchet.json with a debt id",
                    o.symbol, o.item_kind, o.file, o.line
                );
            }
        }
    }
    eprintln!(
        "xtask specmap: ratchet gate — {} gated orphan(s), {} dispositioned ({} crate(s) exempt).",
        blockers,
        orphans.len() - blockers,
        ratchet.exempt.len()
    );
    if blocking && blockers > 0 {
        bail!(
            "specmap ratchet: {blockers} orphan(s) in gated crates — \
             see the list above (PLAYBOOK #phase2 acceptance: empty or dispositioned)"
        );
    }
    Ok(())
}
