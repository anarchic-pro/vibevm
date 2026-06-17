//! Argument structs for `vibe man …` — the VibeVM Version Manager
//! (PROP-019 §2.2). Carries `install` plus the read-only introspection
//! verbs; the switch / remove / gc / doctor verbs land in later slices.

specmark::scope!("spec://vibevm/common/PROP-019#surface");

use clap::Subcommand;

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    #[command(subcommand)]
    pub command: ManSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ManSubcommand {
    /// Build and install a version of vibevm from source.
    Install(ManInstallArgs),

    /// List installed versions, marking the active one (`*`).
    #[command(visible_alias = "list")]
    Ls,

    /// Print the active version's canonical id (`<kind>:<id>`).
    Current,

    /// Print the absolute path of the active `vibe` binary.
    Which,
}

#[derive(Debug, clap::Args)]
pub struct ManInstallArgs {
    /// Version selector: latest | stable | <X.Y.Z> | <commit> | <branch>.
    /// Defaults to `latest` (in-tree: the current checkout).
    #[arg(default_value = "latest")]
    pub selector: String,

    /// Interpret the selector as a git tag.
    #[arg(long, conflicts_with_all = ["branch", "commit"])]
    pub tag: bool,

    /// Interpret the selector as a git branch.
    #[arg(long, conflicts_with_all = ["tag", "commit"])]
    pub branch: bool,

    /// Interpret the selector as a git commit.
    #[arg(long, conflicts_with_all = ["tag", "branch"])]
    pub commit: bool,

    /// Build profile (`debug` | `release`). Defaults to `debug`.
    #[arg(long, value_name = "PROFILE")]
    pub profile: Option<String>,

    /// Shorthand for `--profile release`.
    #[arg(long, conflicts_with = "profile")]
    pub release: bool,

    /// Rebuild even if this version is already installed.
    #[arg(long)]
    pub force: bool,
}
