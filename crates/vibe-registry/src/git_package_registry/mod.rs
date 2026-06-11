//! Per-package git registry — PROP-002.
//!
//! `GitPackageRegistry` resolves a [`PackageRef`] against an organization-root
//! URL by:
//!
//! 1. Composing the per-package repo URL via the registry's [`NamingConvention`]
//!    (`org.vibevm/wal` + `Fqdn` → `<org>/org.vibevm.wal.git`). The registry is
//!    group-native (PROP-008): identity is `(group, name)`, `kind` plays no part
//!    in URL composition or resolution.
//! 2. Listing tags on that repo via the cheap [`GitBackend::list_tags`]
//!    primitive — no clone.
//! 3. Filtering tags to `v<semver>` and picking the highest match for the
//!    requested [`VersionSpec`].
//! 4. For dep-graph walks: reading the candidate version's manifest via
//!    [`GitBackend::fetch_file_at_ref`] — still no clone.
//! 5. Only when the resolver commits to installing a specific version:
//!    [`GitBackend::bootstrap`] (or [`GitBackend::update`] if the clone
//!    already exists), copy the worktree into the per-project package
//!    cache (excluding `.git/`), parse manifest, compute `content_hash`.
//!
//! The cache layout follows PROP-002 §2.6:
//!
//! ```text
//! <cache_root>/<canonical_url_hash>/packages/<group>.<name>/clone/
//! ```
//!
//! `<canonical_url_hash>` is keyed off the **canonical organization URL** of
//! the registry (not the mirror URL), so a transparent mirror does not
//! invalidate the cache. The internal cache subpath uses `<group>.<name>`
//! always, decoupled from the registry's URL-shape `naming` — the cache is
//! organized by `(group, name)` identity, the URLs are just one routing
//! decision.
//!
//! Spec: [PROP-002 §2.5 / §2.6 / §2.12](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md).

specmark::scope!("spec://vibevm/modules/vibe-registry/PROP-002#registry-model");

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use specmark::cell;
use vibe_core::manifest::{Manifest, NamingConvention};
use vibe_core::{Group, PackageRef, VersionSpec};

use crate::git_backend::{GitBackend, GitError, ShellGit};
use crate::registry_cache::{
    DEFAULT_FRESHNESS_SECS, default_cache_root, normalize_url, strip_git_plus_prefix,
};
use crate::{CachedPackage, Registry, RegistryError, ResolvedPackage, compute_content_hash};

mod auth;
mod fetch;
mod urls;

pub use auth::inject_token;
pub(crate) use fetch::copy_dir_excluding_git;

/// Per-package git registry — one organization URL, many package repos under it.
#[cell(seam = "Registry", variant = "git-per-package")]
pub struct GitPackageRegistry {
    backend: Arc<dyn GitBackend>,
    name: String,
    org_url: String,
    org_ref: String,
    naming: NamingConvention,
    /// Authentication regime for this registry, per PROP-002 §2.2.1.
    /// Plumbed in by `MultiRegistryResolver::from_manifest` so the
    /// runtime knows whether to inject a token, whether a 401 is a
    /// fall-through signal, and whether to emit a "missing token"
    /// error before even spawning git.
    auth: vibe_core::manifest::AuthKind,
    /// Resolved bearer token for this registry — `Some` only when
    /// `auth == TokenEnv` and the env-var was set at construction
    /// time. Read once at open and held in memory; never logged,
    /// never written to disk. The token is injected into per-package
    /// URLs in [`Self::credentialed_url`] and stripped from any
    /// public-facing URL the lockfile or error messages might carry.
    /// (Modern git ≥ 2.31 also redacts passwords from its own
    /// stderr; we rely on that as the second line of defence.)
    effective_token: Option<String>,
    /// The env-var name the registry would consult under
    /// `auth = TokenEnv`. Held verbatim so a `MissingToken` error
    /// surfaces the exact name the operator typed in `vibe.toml` /
    /// passed via `vibe registry add --token-env`. `None` when the
    /// caller didn't supply an explicit name — falls back to the
    /// host-derived default at error time.
    token_env_name: Option<String>,
    cache_root: PathBuf,
    canonical_hash: String,
    /// Org-level mirror URLs in priority order (lower index = tried
    /// first). Mirrors share the registry's [`NamingConvention`], so
    /// each mirror URL is treated as an alternate org root from which
    /// per-package URLs are composed identically. Empty in M0/M1.1 and
    /// when `vibe.toml` carries no `[[mirror]]` for this registry.
    /// Phase B v0 wires this only for the read-only lookup paths
    /// (`list_versions`, `fetch_dep_manifest` archive path) — the
    /// fetch/clone path stays primary-only until cross-source
    /// `content_hash` verification lands.
    mirror_urls: Vec<String>,
    /// When `Some`, this registry holds **exactly one package** at the
    /// given URL — `package_repo_url(group, name)` returns this verbatim
    /// without applying `naming` to compose `<org>/<group>.<name>.git`.
    /// Used for git-source declarations from `[requires.packages]`
    /// table-form (PROP-002 §2.4.1). When `None`, this is a normal
    /// multi-package registry and naming applies as before.
    single_package_url: Option<String>,
    /// Optional upstream index — when set, `list_versions` queries
    /// it before falling back to `git ls-remote`. PROP-005 §2.10
    /// slice 10. The fetch path is unaffected: `content_hash` is
    /// still verified at fetch time per [PROP-002 §2.1] regardless
    /// of how versions were enumerated.
    index_client: Option<crate::index_client::IndexClient>,
    /// Implicit-update freshness TTL — reserved for the next commit, where
    /// per-package `meta.toml` files track `last_synced_at`. Stored now so
    /// callers parameterising it do not need to thread it through later.
    #[allow(dead_code)]
    freshness_secs: u64,
}

impl GitPackageRegistry {
    /// Open a registry against the default cache root and a fresh
    /// [`ShellGit`] backend.
    pub fn open(
        name: &str,
        org_url: &str,
        org_ref: &str,
        naming: NamingConvention,
    ) -> Result<Self, RegistryError> {
        let cache_root = default_cache_root()?;
        Self::open_with(
            name,
            org_url,
            org_ref,
            naming,
            &cache_root,
            Arc::new(ShellGit::new()),
            DEFAULT_FRESHNESS_SECS,
        )
    }

    /// Lower-level constructor for tests and callers that want to plug in a
    /// custom backend or cache root.
    pub fn open_with(
        name: &str,
        org_url: &str,
        org_ref: &str,
        naming: NamingConvention,
        cache_root: &Path,
        backend: Arc<dyn GitBackend>,
        freshness_secs: u64,
    ) -> Result<Self, RegistryError> {
        Self::open_with_mirrors(
            name,
            org_url,
            org_ref,
            naming,
            Vec::new(),
            cache_root,
            backend,
            freshness_secs,
        )
    }

    /// Like [`open_with`](Self::open_with), but accepts an org-level
    /// mirror chain in priority order. Used by the multi-registry
    /// resolver to thread `[[mirror]]` from `vibe.toml` into the
    /// registry instance. Empty `mirror_urls` is the same as
    /// [`open_with`].
    ///
    /// `auth` defaults to `AuthKind::None` (the legacy behaviour);
    /// callers wanting authenticated registries reach for
    /// [`Self::open_with_auth`].
    #[allow(clippy::too_many_arguments)]
    pub fn open_with_mirrors(
        name: &str,
        org_url: &str,
        org_ref: &str,
        naming: NamingConvention,
        mirror_urls: Vec<String>,
        cache_root: &Path,
        backend: Arc<dyn GitBackend>,
        freshness_secs: u64,
    ) -> Result<Self, RegistryError> {
        Self::open_with_auth(
            name,
            org_url,
            org_ref,
            naming,
            mirror_urls,
            cache_root,
            backend,
            freshness_secs,
            vibe_core::manifest::AuthKind::None,
            None,
        )
    }

    /// Test-only constructor that takes the resolved token directly
    /// instead of reading an env-var. Production code uses
    /// [`Self::open_with_auth`]. This method is useful in tests
    /// where `#![forbid(unsafe_code)]` prohibits `std::env::set_var`
    /// (Rust 2024+); construct the registry with the token already
    /// in hand and skip the env layer. The resulting registry
    /// behaves identically — same `auth_kind`, same
    /// `effective_token_value`, same downstream injection.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn open_with_explicit_token(
        name: &str,
        org_url: &str,
        org_ref: &str,
        naming: NamingConvention,
        mirror_urls: Vec<String>,
        cache_root: &Path,
        backend: Arc<dyn GitBackend>,
        freshness_secs: u64,
        auth: vibe_core::manifest::AuthKind,
        token_value: Option<String>,
    ) -> Result<Self, RegistryError> {
        let normalized = normalize_url(org_url);
        let canonical_hash = short_url_hash(&normalized);
        let cache_root_owned = cache_root.to_path_buf();
        let bucket = cache_root_owned.join(&canonical_hash);
        fs::create_dir_all(&bucket).map_err(|source| RegistryError::Io {
            path: bucket.clone(),
            source,
        })?;
        Ok(GitPackageRegistry {
            backend,
            name: name.to_string(),
            org_url: org_url.to_string(),
            org_ref: org_ref.to_string(),
            naming,
            auth,
            effective_token: token_value.filter(|s| !s.trim().is_empty()),
            token_env_name: None,
            cache_root: cache_root_owned,
            canonical_hash,
            mirror_urls,
            single_package_url: None,
            index_client: None,
            freshness_secs,
        })
    }

    /// Full constructor — same as [`open_with_mirrors`] plus the
    /// per-registry authentication knobs from PROP-002 §2.2.1.
    ///
    /// `auth` selects the regime; `token_env_name` is the explicit
    /// env-var override under `auth = AuthKind::TokenEnv` (the
    /// host-derived default applies when `None`). Token resolution
    /// happens once at construction time:
    ///
    /// - `AuthKind::TokenEnv` + env-var set → token loaded,
    ///   injected into per-package URLs in
    ///   [`Self::credentialed_url`].
    /// - `AuthKind::TokenEnv` + env-var absent → registry opens
    ///   anyway with no token; a `MissingToken` error surfaces at
    ///   the first credential-required git operation.
    ///   (Authentication is a runtime property of the fetch, not of
    ///   the constructor, so we don't pre-fail here — letting the
    ///   resolver walk a chain that has *some* authenticated
    ///   registries with missing tokens means the operator can fix
    ///   them one by one as install errors surface.)
    /// - Other regimes (`None`, `CredentialHelper`, `Ssh`) read no
    ///   token; their `effective_token` is `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_with_auth(
        name: &str,
        org_url: &str,
        org_ref: &str,
        naming: NamingConvention,
        mirror_urls: Vec<String>,
        cache_root: &Path,
        backend: Arc<dyn GitBackend>,
        freshness_secs: u64,
        auth: vibe_core::manifest::AuthKind,
        token_env_name: Option<&str>,
    ) -> Result<Self, RegistryError> {
        let normalized = normalize_url(org_url);
        let canonical_hash = short_url_hash(&normalized);
        let cache_root_owned = cache_root.to_path_buf();

        let bucket = cache_root_owned.join(&canonical_hash);
        fs::create_dir_all(&bucket).map_err(|source| RegistryError::Io {
            path: bucket.clone(),
            source,
        })?;

        let effective_token = if matches!(auth, vibe_core::manifest::AuthKind::TokenEnv) {
            token_env_name
                .map(|s| s.to_string())
                .and_then(|var| std::env::var(&var).ok())
                .and_then(|v| {
                    let trimmed = v.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                })
        } else {
            None
        };

        Ok(GitPackageRegistry {
            backend,
            name: name.to_string(),
            org_url: org_url.to_string(),
            org_ref: org_ref.to_string(),
            naming,
            auth,
            effective_token,
            token_env_name: token_env_name.map(|s| s.to_string()),
            cache_root: cache_root_owned,
            canonical_hash,
            mirror_urls,
            single_package_url: None,
            index_client: None,
            freshness_secs,
        })
    }

    /// Open a registry that holds **exactly one package** at `repo_url`.
    /// Used for git-source declarations from `[requires.packages]`
    /// table-form (PROP-002 §2.4.1) — the consumer's `vibe.toml`
    /// declares `"org.vibevm/internal" = { git = "...", tag = "..." }`
    /// and the resolver synthesises a `GitPackageRegistry` pointing
    /// directly at that URL, bypassing the org-level `naming`-driven
    /// URL composition.
    ///
    /// Differences from `open_with_auth`:
    /// - `repo_url` is the per-package URL (not an org URL).
    /// - `naming` is irrelevant — the synthetic registry stores `Fqdn`
    ///   as a placeholder; `package_repo_url(group, name)` returns
    ///   `repo_url` verbatim because `single_package_url` short-circuits
    ///   the naming step entirely.
    /// - `mirror_urls` are empty (mirrors are an org-level concept;
    ///   git-source has no mirror chain — see PROP-002 §2.4.1
    ///   "Out of scope").
    /// - `name` is a synthetic local label such as
    ///   `"git-source-org.vibevm-internal"` — not a registry org name.
    #[allow(clippy::too_many_arguments)]
    pub fn open_single_package(
        synthetic_name: &str,
        repo_url: &str,
        repo_ref: &str,
        cache_root: &Path,
        backend: Arc<dyn GitBackend>,
        freshness_secs: u64,
        auth: vibe_core::manifest::AuthKind,
        token_env_name: Option<&str>,
    ) -> Result<Self, RegistryError> {
        let mut reg = Self::open_with_auth(
            synthetic_name,
            repo_url,
            repo_ref,
            NamingConvention::Fqdn,
            Vec::new(),
            cache_root,
            backend,
            freshness_secs,
            auth,
            token_env_name,
        )?;
        reg.single_package_url = Some(repo_url.to_string());
        Ok(reg)
    }

    /// Attach an [`IndexClient`](crate::index_client::IndexClient) to
    /// this registry. When set, `list_versions` consults the index
    /// before falling back to `git ls-remote`. Returns the modified
    /// registry for chaining. Slice 10.
    pub fn with_index_client(mut self, client: crate::index_client::IndexClient) -> Self {
        self.index_client = Some(client);
        self
    }

    pub fn index_client(&self) -> Option<&crate::index_client::IndexClient> {
        self.index_client.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn org_url(&self) -> &str {
        &self.org_url
    }

    pub fn org_ref(&self) -> &str {
        &self.org_ref
    }

    pub fn naming(&self) -> NamingConvention {
        self.naming
    }

    /// Root of this registry's cache bucket — `<cache_root>/<hash>/`.
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_root.join(&self.canonical_hash)
    }

    /// True when this registry was constructed via `open_single_package`
    /// — used by callers (e.g. `MultiRegistryResolver`) to skip features
    /// that don't apply to single-package registries (mirror chains,
    /// org-level index lookups).
    pub fn is_single_package(&self) -> bool {
        self.single_package_url.is_some()
    }

    /// Where this package's clone lives on disk —
    /// `<cache_dir>/packages/<group>.<name>/clone/`. Note the internal
    /// subdirectory is always `<group>.<name>`, regardless of registry
    /// naming (which may have produced a different *URL*-side name) — the
    /// cache is organised by `(group, name)` identity (PROP-008).
    pub fn package_clone_dir(&self, group: &Group, name: &str) -> PathBuf {
        let internal = format!("{group}.{name}");
        self.cache_dir()
            .join("packages")
            .join(internal)
            .join("clone")
    }
}

impl Registry for GitPackageRegistry {
    fn list_versions(
        &self,
        group: &Group,
        name: &str,
    ) -> Result<Vec<semver::Version>, RegistryError> {
        GitPackageRegistry::list_versions(self, group, name)
    }
    fn resolve(&self, pkgref: &PackageRef) -> Result<ResolvedPackage, RegistryError> {
        GitPackageRegistry::resolve(self, pkgref)
    }
    fn fetch(
        &self,
        resolved: &ResolvedPackage,
        cache_root: &Path,
    ) -> Result<CachedPackage, RegistryError> {
        GitPackageRegistry::fetch(self, resolved, cache_root)
    }
}

/// Lowercase hex of the first 8 bytes (16 chars) of `sha256(s)`. Matches the
/// hashing rule pinned in PROP-001 §2.4 / PROP-002 §2.6 — same identity
/// shape as the monorepo `GitRegistry` uses for its registry-level cache
/// directories.
fn short_url_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let digest = h.finalize();
    digest.iter().take(8).fold(String::new(), |mut acc, b| {
        let _ = write!(&mut acc, "{b:02x}");
        acc
    })
}

/// Shared fixtures for this module's submodule tests — the canned
/// [`GitBackend`] fake plus registry constructors.
#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    use super::*;

    /// Test-only `GitBackend` that serves a pre-seeded set of tags and
    /// archive-fetched files per `(url, ref, path)`, and on `bootstrap`
    /// copies a fixture directory into the destination clone.
    #[derive(Default)]
    pub(crate) struct FakeBackend {
        pub(crate) tags: Mutex<HashMap<String, Vec<String>>>,
        pub(crate) files: Mutex<HashMap<(String, String, String), Vec<u8>>>,
        pub(crate) bootstrap_seeds: Mutex<HashMap<String, PathBuf>>,
        /// URLs that should make `update` fail with `RefNotFound`. Used
        /// to test the "primary's working clone is now stuck on a tag
        /// the remote no longer carries; fall through to mirror"
        /// scenario — the mirror walk must wipe the local clone and
        /// re-bootstrap from the next URL when `update` fails.
        pub(crate) update_fail_urls: Mutex<HashSet<String>>,
        pub(crate) bootstrap_calls: Mutex<u32>,
        pub(crate) update_calls: Mutex<u32>,
        /// Recorded bootstrap-call URLs, in order. Tests assert on
        /// this when verifying primary-then-mirror dispatch.
        pub(crate) bootstrap_urls: Mutex<Vec<String>>,
    }

    impl FakeBackend {
        pub(crate) fn seed_tags(&self, url: impl Into<String>, tags: Vec<String>) {
            self.tags.lock().unwrap().insert(url.into(), tags);
        }
        pub(crate) fn seed_file(
            &self,
            url: impl Into<String>,
            refname: impl Into<String>,
            path: impl Into<String>,
            bytes: Vec<u8>,
        ) {
            self.files
                .lock()
                .unwrap()
                .insert((url.into(), refname.into(), path.into()), bytes);
        }
        pub(crate) fn seed_bootstrap(&self, url: impl Into<String>, source_dir: PathBuf) {
            self.bootstrap_seeds
                .lock()
                .unwrap()
                .insert(url.into(), source_dir);
        }
        /// Wire `update(dest, refname)` to fail with `RefNotFound` when
        /// the clone at `dest` was last bootstrapped from `url`. Used
        /// to drive the "wipe and fall through" branch of
        /// `bootstrap_or_update_at`.
        #[allow(dead_code)]
        pub(crate) fn fail_update_for_url(&self, url: impl Into<String>) {
            self.update_fail_urls.lock().unwrap().insert(url.into());
        }
        pub(crate) fn bootstrap_count(&self) -> u32 {
            *self.bootstrap_calls.lock().unwrap()
        }
        pub(crate) fn update_count(&self) -> u32 {
            *self.update_calls.lock().unwrap()
        }
        pub(crate) fn bootstrap_urls(&self) -> Vec<String> {
            self.bootstrap_urls.lock().unwrap().clone()
        }
    }

    impl GitBackend for FakeBackend {
        fn bootstrap(&self, url: &str, _refname: &str, dest: &Path) -> Result<(), GitError> {
            *self.bootstrap_calls.lock().unwrap() += 1;
            self.bootstrap_urls.lock().unwrap().push(url.to_string());
            let seed = self
                .bootstrap_seeds
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| GitError::RepoNotFound {
                    url: url.to_string(),
                })?;
            fs::create_dir_all(dest).unwrap();
            for entry in walkdir::WalkDir::new(&seed)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let rel = entry.path().strip_prefix(&seed).unwrap();
                if rel.as_os_str().is_empty() {
                    continue;
                }
                let target = dest.join(rel);
                if entry.file_type().is_dir() {
                    fs::create_dir_all(&target).unwrap();
                } else if entry.file_type().is_file() {
                    fs::copy(entry.path(), &target).unwrap();
                }
            }
            // Mark dest as a real git repo for the `.git` presence check.
            // Stash the URL inside `.git/origin-url` so `update` can
            // recover which URL last sourced this clone — that lets
            // `fail_update_for_url` selectively fail updates per origin.
            fs::create_dir_all(dest.join(".git")).unwrap();
            fs::write(dest.join(".git/origin-url"), url).unwrap();
            Ok(())
        }
        fn update(&self, dest: &Path, refname: &str) -> Result<(), GitError> {
            *self.update_calls.lock().unwrap() += 1;
            let origin = fs::read_to_string(dest.join(".git/origin-url")).unwrap_or_default();
            if self.update_fail_urls.lock().unwrap().contains(&origin) {
                return Err(GitError::RefNotFound {
                    url: origin,
                    refname: refname.to_string(),
                });
            }
            Ok(())
        }
        fn list_tags(&self, url: &str) -> Result<Vec<String>, GitError> {
            self.tags
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| GitError::RepoNotFound {
                    url: url.to_string(),
                })
        }
        fn fetch_file_at_ref(
            &self,
            url: &str,
            refname: &str,
            path: &str,
        ) -> Result<Vec<u8>, GitError> {
            let key = (url.to_string(), refname.to_string(), path.to_string());
            self.files
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| GitError::FileNotFoundInRef {
                    url: url.to_string(),
                    refname: refname.to_string(),
                    path: path.to_string(),
                })
        }
    }

    pub(crate) fn manifest_text(name: &str, kind: &str, version: &str) -> String {
        format!(
            "[package]\ngroup = \"org.vibevm\"\nname = \"{name}\"\nkind = \"{kind}\"\nversion = \"{version}\"\n"
        )
    }

    /// The canonical group every fixture package in these tests belongs
    /// to. The registry is group-native (PROP-008): identity is
    /// `(group, name)`, `kind` plays no part in resolution.
    pub(crate) fn org() -> Group {
        Group::parse("org.vibevm").unwrap()
    }

    pub(crate) fn registry_with(
        cache: &Path,
        org_url: &str,
        naming: NamingConvention,
        backend: Arc<dyn GitBackend>,
    ) -> GitPackageRegistry {
        GitPackageRegistry::open_with(
            "vibespecs",
            org_url,
            "main",
            naming,
            cache,
            backend,
            DEFAULT_FRESHNESS_SECS,
        )
        .unwrap()
    }

    pub(crate) fn registry_with_mirrors(
        cache: &Path,
        org_url: &str,
        naming: NamingConvention,
        mirror_urls: Vec<String>,
        backend: Arc<dyn GitBackend>,
    ) -> GitPackageRegistry {
        GitPackageRegistry::open_with_mirrors(
            "vibespecs",
            org_url,
            "main",
            naming,
            mirror_urls,
            cache,
            backend,
            DEFAULT_FRESHNESS_SECS,
        )
        .unwrap()
    }
}
