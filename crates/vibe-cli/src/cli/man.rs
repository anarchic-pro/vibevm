//! Argument structs for `vibe man …` — the VibeVM Version Manager
//! (PROP-019 §2.2). This slice carries the read-only introspection verbs;
//! the build / switch / remove verbs land in later slices.

specmark::scope!("spec://vibevm/common/PROP-019#surface");

use clap::Subcommand;

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    #[command(subcommand)]
    pub command: ManSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ManSubcommand {
    /// List installed versions, marking the active one (`*`).
    #[command(visible_alias = "list")]
    Ls,

    /// Print the active version's canonical id (`<kind>:<id>`).
    Current,

    /// Print the absolute path of the active `vibe` binary.
    Which,
}
