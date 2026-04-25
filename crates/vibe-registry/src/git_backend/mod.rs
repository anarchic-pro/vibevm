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

    #[error("file `{path}` not found in `{url}` at ref `{refname}`")]
    FileNotFoundInRef {
        url: String,
        refname: String,
        path: String,
    },

    #[error(
        "remote `{url}` does not support `git archive` for fetching individual files \
         (uploadarch service refused). Caller should fall back to a clone."
    )]
    ArchiveUnsupported { url: String },

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

/// Narrow abstraction over the git operations the registry needs.
///
/// The trait deliberately stays small — every new method is a deliberate
/// widening, not an accident. Today it carries:
///
/// - `bootstrap` / `update` — full clone and refresh of a working tree.
/// - `list_tags` / `fetch_file_at_ref` — *shallow* primitives the depsolver
///   uses to enumerate versions and read manifests *without* a clone (see
///   PROP-002 §2.12 — performance strategy). A resolver pass that touches
///   N candidate versions of a package must not clone all N; it walks
///   `list_tags` then reads `vibe-package.toml` per candidate via
///   `fetch_file_at_ref`, and only `bootstrap`s the version it commits to.
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

    /// List the tag names available on `url` without cloning. Implemented
    /// via `git ls-remote --tags`. Tags annotated with the
    /// `^{}` peeled-form suffix are stripped so the caller sees clean
    /// tag names; duplicates (peeled + annotated) are deduplicated.
    ///
    /// Returns tag names verbatim — semver coercion (e.g. stripping the
    /// `v` prefix) is the caller's job.
    fn list_tags(&self, url: &str) -> Result<Vec<String>, GitError>;

    /// Fetch the contents of a single file at the given ref from `url`,
    /// without populating a working tree. Implemented via `git archive
    /// --remote=<url> --format=tar <refname> <path>` piped through
    /// in-process tar extraction.
    ///
    /// `path` is the path inside the repo; both forward-slash and
    /// platform-native separators are accepted and normalised to forward
    /// slash (the form `git archive` expects).
    ///
    /// Returns the file's bytes. Errors:
    /// - [`GitError::RefNotFound`] if `refname` does not exist on `url`.
    /// - [`GitError::FileNotFoundInRef`] if `path` is missing in that ref.
    ///
    /// Note that `git archive` over `git://`-style protocols requires
    /// server support (`uploadarch.allowAnySHA1InWant` etc). Hosted git
    /// providers (GitHub, GitLab, Gitea, GitVerse) typically support
    /// this; a private bare server may not. The `GitBackend` returns
    /// [`GitError::ArchiveUnsupported`] in that case so the caller can
    /// fall back to a shallow clone.
    fn fetch_file_at_ref(
        &self,
        url: &str,
        refname: &str,
        path: &str,
    ) -> Result<Vec<u8>, GitError>;
}
