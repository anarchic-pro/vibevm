//! Git backend abstraction.
//!
//! Every operation the registry performs against git goes through
//! [`GitBackend`]. `M1` ships exactly one implementation — [`ShellGit`],
//! which spawns the system `git` binary. A future `libgit2`-based
//! implementation would add a second type alongside it without touching
//! [`GitBackend`] consumers.
//!
//! Spec: [`spec/modules/vibe-registry/PROP-001-git-backend.md`][prop].
//!
//! [prop]: ../../../../../spec/modules/vibe-registry/PROP-001-git-backend.md

use std::path::Path;

use thiserror::Error;

pub mod shell;

pub use shell::ShellGit;

/// Errors a [`GitBackend`] operation may surface.
///
/// Variants correspond to stderr patterns stable enough to key on.
/// Anything unclassified surfaces as [`GitError::CommandFailed`] with the
/// raw stderr attached.
#[derive(Debug, Error)]
pub enum GitError {
    #[error(
        "the `git` executable is not available on PATH; install git (https://git-scm.com/downloads) and retry"
    )]
    NotInstalled,

    #[error("remote repository `{url}` not found (does it exist? is access granted?)")]
    RepoNotFound { url: String },

    #[error("ssh authentication failed for `{url}` — check your ssh-agent / keys")]
    AuthFailed { url: String },

    #[error("unable to reach `{url}` (network or DNS error)")]
    NetworkUnreachable { url: String },

    #[error("branch / ref `{refname}` not found on `{url}`")]
    RefNotFound { url: String, refname: String },

    #[error("git `{cmd}` exited with status {status}:\n{stderr}")]
    CommandFailed {
        cmd: String,
        status: i32,
        stderr: String,
    },

    #[error("I/O error spawning git `{cmd}`: {source}")]
    Io {
        cmd: String,
        #[source]
        source: std::io::Error,
    },
}

/// Narrow abstraction over the two git operations the registry needs.
///
/// The trait deliberately carries no `ls_remote`, `fetch_ref`, or
/// `checkout` — version discovery is done by reading the working tree
/// after a clone or update. Widening the interface is a deliberate
/// design decision, not something that happens by accident.
///
/// **Method names.** `bootstrap` (not `clone`) avoids collision with
/// `std::clone::Clone::clone` when the backend is held behind
/// `Arc<dyn GitBackend>`, where `Arc::clone` would otherwise be
/// ambiguous at the call site.
pub trait GitBackend: Send + Sync {
    /// Clone `url` (checked out at `refname`) into `dest`.
    ///
    /// The caller guarantees `dest` is either empty or absent. On error,
    /// the backend makes no guarantee about the partial state of `dest`
    /// — the caller cleans up.
    fn bootstrap(&self, url: &str, refname: &str, dest: &Path) -> Result<(), GitError>;

    /// Fast-forward `dest` to `origin/<refname>`. Assumes `dest` is a git
    /// repository previously populated by `bootstrap`.
    fn update(&self, dest: &Path, refname: &str) -> Result<(), GitError>;
}
