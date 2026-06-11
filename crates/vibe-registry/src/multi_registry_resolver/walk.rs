//! The priority-ordered registry walk — override / path-source /
//! git-source short-circuits, `UnknownPackage` fall-through, the
//! auth-aware 401 classification (PROP-002 §2.3.1), and the
//! fetch-side dispatch over the resolved source kind.

specmark::scope!("spec://vibevm/modules/vibe-registry/PROP-002#registry-model");

use std::fmt::Write as _;

use super::redirect_follow::try_fetch_redirect;
use super::*;

/// One row in the aggregated "tried these registries" report
/// surfaced via [`RegistryError::PackageNotFoundEverywhere`].
/// Captured per-registry during the walk in
/// [`MultiRegistryResolver::resolve`]; carried through the
/// `DepProvider` error chain into `vibe-cli`'s install-error
/// JSON envelope so machine-readable consumers can branch on the
/// per-registry status without parsing prose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegistryWalkAttempt {
    pub name: String,
    pub url: String,
    pub auth: vibe_core::manifest::AuthKind,
    pub status: WalkAttemptStatus,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WalkAttemptStatus {
    /// Registry's `resolve` returned `UnknownPackage` — the
    /// registry was reachable, manifest parsed, just no version
    /// matching the pkgref.
    NotFound,
    /// Registry returned 401 / 403 (`AuthFailed`) but was declared
    /// `auth = "none"` and `strict_auth` was off, so the resolver
    /// reclassified the failure as "no public answer here" and
    /// walked past. The line below tells the operator the host
    /// would need credentials if they want to access this
    /// registry as authenticated.
    Public401,
}

impl WalkAttemptStatus {
    pub fn as_label(&self) -> &'static str {
        match self {
            WalkAttemptStatus::NotFound => "not found",
            WalkAttemptStatus::Public401 => "access denied (401, walked past — auth=none)",
        }
    }
}

fn format_walk_attempts(attempts: &[RegistryWalkAttempt]) -> String {
    // Compute the column width for the registry-name column so
    // the rendered table stays aligned regardless of label
    // length. URLs are too varied to align — they wrap to the
    // right of the arrow.
    let name_width = attempts.iter().map(|a| a.name.len()).max().unwrap_or(0);
    let url_width = attempts.iter().map(|a| a.url.len()).max().unwrap_or(0);
    let mut out = String::new();
    for a in attempts {
        // Indent each line with two spaces so the report nests
        // visually under the parent error's "Tried:" label.
        let _ = writeln!(
            out,
            "  - {:<name_width$}  ({:<url_width$})  → {} (auth={})",
            a.name,
            a.url,
            a.status.as_label(),
            a.auth.as_str(),
            name_width = name_width,
            url_width = url_width,
        );
    }
    // Hint at the bottom — the most common operator next step
    // when nothing was found anywhere.
    if attempts
        .iter()
        .any(|a| matches!(a.status, WalkAttemptStatus::Public401))
    {
        out.push_str(
            "\nHint: at least one registry returned 401 / 403 and was walked past as `auth=none`.\n\
             If that registry is actually private, set `auth = \"token-env\"` and provide the\n\
             token via `VIBEVM_REGISTRY_TOKEN_<HOST>`; see docs/registry-auth.md.",
        );
    }
    out
}

impl MultiRegistryResolver {
    /// Resolve a pkgref through the override-then-registries decision tree.
    pub fn resolve(&self, pkgref: &PackageRef) -> Result<MultiResolution, RegistryError> {
        // Step 1: override short-circuit.
        if let Some(ovr) = self.overrides.get(&pkgref.qualified_name()) {
            return self.resolve_override(pkgref, ovr);
        }

        // Step 1.25: path-source short-circuit (PROP-007 §2.5).
        // `[requires.packages]` table-form may declare a dep as
        // `{ path = "..." }`; the package lives in a local directory
        // (typically a sibling workspace member). Path-source sits one
        // notch above git-source — a pkgref present in both sets
        // resolves via path-source. No registry walk, no git clone.
        if let Some(dep) = self.path_packages.get(&pkgref.qualified_name()) {
            return self.resolve_path_source(pkgref, dep);
        }

        // Step 1.5: git-source short-circuit (PROP-002 §2.4.1).
        // `[requires.packages]` table-form may declare a dep as
        // `{ git = "...", tag/branch/rev = "..." }`; the resolver
        // bypasses the `[[registry]]` walk for that pkgref entirely
        // and fetches directly from the declared URL.
        if let Some(dep) = self.git_packages.get(&pkgref.qualified_name()) {
            return self.resolve_git_source(pkgref, dep);
        }

        // Step 2: priority-ordered registry walk. PROP-002 §2.3.1
        // failure-mode discriminator:
        //
        // - `UnknownPackage` → fall through to next registry.
        // - `Git(AuthFailed)` on an `auth = "none"` registry →
        //   reclassify as `UnknownPackage` and fall through. (For
        //   public registries 401 / 403 means "no public answer
        //   here", e.g. GitVerse's policy on missing repos.)
        // - `Git(AuthFailed)` on an authenticated registry
        //   (`token-env`, `credential-helper`) → halt with the
        //   error — the operator declared this registry expects
        //   credentials, the credentials presented were rejected,
        //   the operator must see that.
        // - `MissingToken` on any registry → halt — the manifest
        //   declared `auth = "token-env"` but the env-var is unset;
        //   the operator must fix the env. (Walking past this would
        //   silently downgrade a private registry to "not present"
        //   which would mask configuration errors.)
        // - any other error → halt as before (network, malformed
        //   manifest, server error, ...).
        // A registry resolves by `(group, name)` identity (PROP-008 §2.2);
        // a pkgref reaching the registry walk without a `group` is an
        // `UnqualifiedPkgref` — short names are qualified at the CLI
        // boundary, never here.
        let group = pkgref
            .group
            .as_ref()
            .ok_or_else(|| RegistryError::UnqualifiedPkgref(pkgref.to_string()))?;
        let mut attempts: Vec<RegistryWalkAttempt> = Vec::new();
        for reg in &self.registries {
            match reg.resolve(pkgref) {
                Ok(resolved) => {
                    let stub_tag = format!("v{}", resolved.version);
                    // Step 2a: redirect probe (PROP-002 §2.4.2). The
                    // registry served a tag; check whether the repo
                    // at that tag is a stub pointing elsewhere. The
                    // probe is one extra `git archive` call, only
                    // when the registry-walk leg succeeded; cheap.
                    if let Some(redirect) =
                        try_fetch_redirect(&self.backend, reg, &resolved, &stub_tag)?
                    {
                        return self.follow_redirect(pkgref, &resolved, reg, &redirect, &stub_tag);
                    }
                    let url = reg.package_repo_url(&resolved.group, &resolved.name)?;
                    return Ok(MultiResolution {
                        resolved,
                        registry_name: Some(reg.name().to_string()),
                        source_url: url,
                        source_ref: Some(stub_tag),
                        overridden: false,
                        is_git_source: false,
                        is_path_source: false,
                        via_redirect: None,
                        redirect_target_auth: vibe_core::manifest::AuthKind::None,
                        redirect_target_token_env: None,
                    });
                }
                Err(RegistryError::UnknownPackage { .. }) => {
                    attempts.push(RegistryWalkAttempt {
                        name: reg.name().to_string(),
                        url: reg.org_url().to_string(),
                        auth: reg.auth_kind(),
                        status: WalkAttemptStatus::NotFound,
                    });
                    continue;
                }
                Err(RegistryError::Git(crate::git_backend::GitError::AuthFailed { .. }))
                    if matches!(reg.auth_kind(), vibe_core::manifest::AuthKind::None)
                        && !self.strict_auth =>
                {
                    tracing::debug!(
                        target: "vibe_registry::resolve",
                        registry = %reg.name(),
                        "auth_failed on auth=none registry treated as unknown-package; walking"
                    );
                    attempts.push(RegistryWalkAttempt {
                        name: reg.name().to_string(),
                        url: reg.org_url().to_string(),
                        auth: reg.auth_kind(),
                        status: WalkAttemptStatus::Public401,
                    });
                    continue;
                }
                Err(other) => return Err(other),
            }
        }

        // No registry had a satisfying answer. Two shapes:
        //
        // - If we walked at least one registry, surface the
        //   aggregate per-registry status so the operator sees
        //   exactly what happened where (PackageNotFoundEverywhere).
        // - Otherwise (no `[[registry]]` configured) fall back to
        //   the simpler UnknownPackage for back-compat with
        //   downstream consumers that match on it specifically.
        if attempts.is_empty() {
            return Err(RegistryError::UnknownPackage {
                group: group.clone(),
                name: pkgref.name.clone(),
            });
        }
        let summary = format_walk_attempts(&attempts);
        Err(RegistryError::PackageNotFoundEverywhere {
            group: group.clone(),
            name: pkgref.name.clone(),
            summary,
            attempts,
        })
    }

    fn resolve_override(
        &self,
        pkgref: &PackageRef,
        ovr: &OverrideSection,
    ) -> Result<MultiResolution, RegistryError> {
        let group = pkgref
            .group
            .as_ref()
            .ok_or_else(|| RegistryError::UnqualifiedPkgref(pkgref.to_string()))?;
        let refname = ovr
            .r#ref
            .clone()
            .unwrap_or_else(|| DEFAULT_OVERRIDE_REF.to_string());
        let manifest = self.read_override_manifest(&ovr.source_url, &refname)?;
        let meta = manifest
            .require_package()
            .map_err(|e| RegistryError::MalformedMeta {
                path: PathBuf::from(format!("{}@{}:vibe.toml", ovr.source_url, refname)),
                reason: e.to_string(),
            })?;
        // Sanity: the override is supposed to point at *this* package. If
        // the manifest at the pinned ref names a different `(group, name)`
        // identity, installing it would silently misroute on disk. Refuse
        // loudly. `kind` is metadata (PROP-008 §2.3) — not compared here.
        if &meta.group != group || meta.name != pkgref.name {
            return Err(RegistryError::MalformedMeta {
                path: PathBuf::from(format!("{}@{}:vibe.toml", ovr.source_url, refname)),
                reason: format!(
                    "override for `{}/{}` points at a manifest declaring `{}/{}` — refusing to install",
                    group, pkgref.name, meta.group, meta.name
                ),
            });
        }
        let resolved = ResolvedPackage {
            group: group.clone(),
            name: pkgref.name.clone(),
            version: meta.version.clone(),
            source_dir: self.override_clone_dir(group, &pkgref.name),
        };
        Ok(MultiResolution {
            resolved,
            registry_name: None,
            source_url: ovr.source_url.clone(),
            source_ref: Some(refname),
            overridden: true,
            is_git_source: false,
            is_path_source: false,
            via_redirect: None,
            redirect_target_auth: vibe_core::manifest::AuthKind::None,
            redirect_target_token_env: None,
        })
    }

    /// Read `vibe.toml` for a resolved `(group, name, version)`,
    /// transparently following any registry-redirect stub (PROP-002
    /// §2.4.2) or git-source declaration (§2.4.1). The depsolver's
    /// [`DepProvider::fetch_manifest`] adapter uses this so a
    /// stub-served pkgref returns the **target's** manifest (the stub
    /// itself carries only `vibe-redirect.toml`) and a git-source
    /// pkgref returns the manifest at the declared `tag`/`branch`/`rev`.
    ///
    /// The implementation re-runs [`Self::resolve`] with the version
    /// constraint pinned to `=<version>` so it converges on the same
    /// `MultiResolution` the install pipeline already saw, then reads
    /// the manifest from whichever URL the resolution recorded —
    /// stub's target for redirects, declared URL for git-source,
    /// the registry's own URL otherwise. Walking registries directly
    /// (the pre-M1.16 shape) cannot serve a stub-only repo.
    ///
    /// Keyed by `(group, name)` identity (PROP-008) — `kind` is metadata,
    /// read off the resolved manifest.
    pub fn fetch_manifest(
        &self,
        group: &Group,
        name: &str,
        version: &semver::Version,
    ) -> Result<Manifest, RegistryError> {
        // Build a pinned pkgref so `resolve` converges on the exact
        // slot the install pipeline committed to (the depsolver pinned
        // the version via `resolve_version` first). For pass-through
        // redirects (and direct registry installs) the stub's tag list
        // contains `v<version>` and the pinned resolve hits it
        // immediately. For pinned-policy redirects the stub may have
        // unrelated tags (the pinned semantic — every consumer goes
        // to the target's pinned ref, so the stub tag is irrelevant);
        // we fall back to a constraint-free resolve and verify the
        // resolved version still matches.
        let pinned_pkgref =
            PackageRef::parse(&format!("{group}/{name}@={version}")).map_err(|e| {
                RegistryError::MalformedMeta {
                    path: PathBuf::from("<synthetic-pkgref>"),
                    reason: format!("constructing pinned pkgref: {e}"),
                }
            })?;
        let resolution = match self.resolve(&pinned_pkgref) {
            Ok(r) => r,
            Err(RegistryError::NoMatchingVersion { .. })
            | Err(RegistryError::PackageNotFoundEverywhere { .. })
            | Err(RegistryError::UnknownPackage { .. }) => {
                // The stub's tag list does not contain `=version` —
                // happens with pinned-policy redirects where the
                // stub-side tag and the target version are decoupled.
                // Re-resolve without a constraint and accept the
                // result as long as the version it produces matches
                // what the depsolver pinned.
                let fallback_pkgref =
                    PackageRef::parse(&format!("{group}/{name}")).map_err(|e| {
                        RegistryError::MalformedMeta {
                            path: PathBuf::from("<synthetic-pkgref>"),
                            reason: format!("constructing latest pkgref: {e}"),
                        }
                    })?;
                let r = self.resolve(&fallback_pkgref)?;
                if &r.resolved.version != version {
                    return Err(RegistryError::NoMatchingVersion {
                        group: group.clone(),
                        name: name.to_string(),
                        req: format!("={version}"),
                    });
                }
                r
            }
            Err(other) => return Err(other),
        };

        if resolution.is_path_source {
            // Path-source: the package lives in a local directory.
            // `path_packages` carries the resolver-side `package_dir`
            // (already canonicalised by the workspace layer); read
            // `vibe.toml` straight off disk so transitive dependencies
            // of a path-source package resolve.
            let dep = self
                .path_packages
                .get(&pinned_pkgref.qualified_name())
                .ok_or_else(|| RegistryError::UnknownPackage {
                    group: group.clone(),
                    name: name.to_string(),
                })?;
            let manifest_path = dep.package_dir.join(Manifest::FILENAME);
            return Manifest::read(&manifest_path).map_err(RegistryError::from);
        }

        if resolution.via_redirect.is_some() {
            // Redirect-resolved: target_url is in source_url, target_ref
            // is in source_ref. Open a synthetic single-package
            // registry on the target and read the manifest at the
            // recorded ref. Auth carries the redirect's declared
            // policy so private targets keep working.
            let target_url = resolution.source_url.clone();
            let target_ref = resolution.source_ref.clone().unwrap_or_default();
            let synthetic_name = format!("redirect-target-{group}-{name}");
            let target_reg = GitPackageRegistry::open_single_package(
                &synthetic_name,
                &target_url,
                &target_ref,
                &self.cache_root,
                Arc::clone(&self.backend),
                DEFAULT_FRESHNESS_SECS,
                resolution.redirect_target_auth,
                resolution.redirect_target_token_env.as_deref(),
            )?;
            return target_reg.fetch_manifest_at_ref(group, name, &target_ref);
        }

        if resolution.is_git_source {
            // Git-source: source_url + source_ref carry the operator-
            // declared `tag`/`branch`/`rev`. Construct the same
            // synthetic registry the resolver used and re-read the
            // manifest at that ref.
            //
            // Note: `git_packages` lookup gives us the original
            // `auth` / `token_env`; the resolver did the lookup at
            // `resolve` time and stored the values there too.
            let dep = self
                .git_packages
                .get(&pinned_pkgref.qualified_name())
                .ok_or_else(|| RegistryError::UnknownPackage {
                    group: group.clone(),
                    name: name.to_string(),
                })?;
            let source_ref = resolution
                .source_ref
                .clone()
                .unwrap_or_else(|| dep.ref_kind.as_str().to_string());
            let synthetic_name = format!("git-source-{group}-{name}");
            let reg = GitPackageRegistry::open_single_package(
                &synthetic_name,
                &dep.url,
                &source_ref,
                &self.cache_root,
                Arc::clone(&self.backend),
                DEFAULT_FRESHNESS_SECS,
                dep.auth,
                dep.token_env.as_deref(),
            )?;
            return reg.fetch_manifest_at_ref(group, name, &source_ref);
        }

        // Override or registry: walk in priority order, preferring the
        // registry the resolver picked. Override-served packages have
        // `registry_name = None`, so we just walk and the first match
        // wins (overrides are not consulted by `fetch_dep_manifest` —
        // those are handled by the install pipeline directly).
        if let Some(name_filter) = &resolution.registry_name
            && let Some(reg) = self
                .registries
                .iter()
                .find(|r| r.name() == name_filter.as_str())
        {
            return reg.fetch_dep_manifest(group, name, version);
        }
        let mut last_err: Option<RegistryError> = None;
        for reg in &self.registries {
            match reg.fetch_dep_manifest(group, name, version) {
                Ok(m) => return Ok(m),
                Err(err)
                    if matches!(
                        err,
                        RegistryError::Git(GitError::FileNotFoundInRef { .. })
                            | RegistryError::Git(GitError::ArchiveUnsupported { .. })
                            | RegistryError::Io { .. }
                            | RegistryError::MalformedMeta { .. }
                            | RegistryError::UnknownPackage { .. }
                            | RegistryError::NoMatchingVersion { .. }
                    ) =>
                {
                    last_err = Some(err);
                    continue;
                }
                Err(other) => return Err(other),
            }
        }
        Err(last_err.unwrap_or(RegistryError::UnknownPackage {
            group: group.clone(),
            name: name.to_string(),
        }))
    }

    fn read_override_manifest(&self, url: &str, refname: &str) -> Result<Manifest, RegistryError> {
        let bytes = self.backend.fetch_file_at_ref(
            strip_git_plus_prefix(url),
            refname,
            Manifest::FILENAME,
        )?;
        let text = String::from_utf8(bytes).map_err(|e| RegistryError::MalformedMeta {
            path: PathBuf::from(format!("{url}@{refname}:{}", Manifest::FILENAME)),
            reason: format!("invalid UTF-8: {e}"),
        })?;
        Manifest::parse_str(&text).map_err(|e| RegistryError::MalformedMeta {
            path: PathBuf::from(format!("{url}@{refname}:{}", Manifest::FILENAME)),
            reason: e.to_string(),
        })
    }

    /// Materialise a previously-resolved package into the per-project cache.
    /// The returned [`CachedPackage`] carries lockfile-v2 provenance
    /// (`registry_name` / `source_ref` / `overridden`) populated by the
    /// `GitPackageRegistry` impl or by the override path.
    pub fn fetch(
        &self,
        resolution: &MultiResolution,
        project_cache: &Path,
    ) -> Result<CachedPackage, RegistryError> {
        self.fetch_with_expected_hash(resolution, project_cache, None)
    }

    /// Mirror-aware fetch with an optional cross-source content_hash gate.
    ///
    /// `expected_hash`, when supplied (typically the lockfile pin for
    /// this `(kind, name, version)`), is enforced source-by-source:
    /// each URL in the registry's primary-then-mirror chain is tried,
    /// and the first whose served content matches the pin wins. A
    /// disagreeing source is logged at `tracing::warn!` and skipped.
    /// If every source disagrees, the last one's [`CachedPackage`] is
    /// returned — its `content_hash` differs from `expected_hash`, so the
    /// caller can compare the two to detect drift against the lockfile pin.
    ///
    /// Override-resolved entries skip mirror dispatch entirely —
    /// `[[override]]` is a surgical pin to one specific URL/ref by
    /// design, so the same URL is the only legitimate source.
    pub fn fetch_with_expected_hash(
        &self,
        resolution: &MultiResolution,
        project_cache: &Path,
        expected_hash: Option<&str>,
    ) -> Result<CachedPackage, RegistryError> {
        if resolution.overridden {
            return self.fetch_override(resolution, project_cache);
        }
        if resolution.is_path_source {
            return self.fetch_path_source(resolution, project_cache);
        }
        if resolution.is_git_source {
            return self.fetch_git_source(resolution, project_cache, expected_hash);
        }
        if resolution.via_redirect.is_some() {
            return self.fetch_via_redirect(resolution, project_cache, expected_hash);
        }
        let registry_name =
            resolution
                .registry_name
                .as_deref()
                .ok_or_else(|| RegistryError::UnknownPackage {
                    group: resolution.resolved.group.clone(),
                    name: resolution.resolved.name.clone(),
                })?;
        let reg = self
            .registries
            .iter()
            .find(|r| r.name() == registry_name)
            .ok_or_else(|| RegistryError::UnknownPackage {
                group: resolution.resolved.group.clone(),
                name: resolution.resolved.name.clone(),
            })?;
        // `GitPackageRegistry::fetch_with_expected_hash` already populates
        // `registry_name` / `source_ref` / `overridden = false` correctly;
        // nothing to wrap.
        reg.fetch_with_expected_hash(&resolution.resolved, project_cache, expected_hash)
    }

    fn fetch_override(
        &self,
        resolution: &MultiResolution,
        project_cache: &Path,
    ) -> Result<CachedPackage, RegistryError> {
        let url = &resolution.source_url;
        let refname = resolution
            .source_ref
            .clone()
            .unwrap_or_else(|| DEFAULT_OVERRIDE_REF.to_string());
        let group = &resolution.resolved.group;
        let name = resolution.resolved.name.as_str();

        let clone_dir = self.override_clone_dir(group, name);
        ensure_clone_at(self.backend.as_ref(), url, &refname, &clone_dir)?;

        let dest = project_cache
            .join(group.as_str())
            .join(name)
            .join(format!("v{}", resolution.resolved.version));
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|source| RegistryError::Io {
                path: dest.clone(),
                source,
            })?;
        }
        copy_dir_excluding_git(&clone_dir, &dest)?;

        let manifest_path = dest.join(Manifest::FILENAME);
        let manifest = Manifest::read(&manifest_path)?;
        if manifest.package.is_none() {
            return Err(RegistryError::MalformedMeta {
                path: manifest_path.clone(),
                reason: "registry package manifest must carry a [package] table".to_string(),
            });
        }
        let content_hash = compute_content_hash(&dest)?;

        Ok(CachedPackage {
            resolved: ResolvedPackage {
                group: group.clone(),
                name: name.to_string(),
                version: resolution.resolved.version.clone(),
                source_dir: clone_dir,
            },
            cache_dir: dest,
            manifest,
            content_hash,
            source_uri: url.clone(),
            registry_name: None,
            source_ref: Some(refname),
            resolved_commit: None,
            overridden: true,
            is_git_source: false,
            is_path_source: false,
            via_redirect: None,
        })
    }

    /// Where override clones live —
    /// `<cache_root>/__overrides__/<group>.<name>/clone/`. Distinct
    /// directory tree from registry-served clones so a package that flips
    /// between override and registry origins on different days does not
    /// share state across modes. Keyed by `(group, name)` identity
    /// (PROP-008).
    pub(super) fn override_clone_dir(&self, group: &Group, name: &str) -> PathBuf {
        self.cache_root
            .join("__overrides__")
            .join(format!("{group}.{name}"))
            .join("clone")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use vibe_core::manifest::NamingConvention;

    use crate::multi_registry_resolver::test_support::*;

    #[test]
    fn resolve_picks_first_registry_with_match() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Both registries have the package; first wins.
        fake.seed_tags(
            "git@host:org-a/org.vibevm.wal.git",
            vec!["v0.1.0".into(), "v0.2.0".into()],
        );
        fake.seed_tags("git@host:org-b/org.vibevm.wal.git", vec!["v0.5.0".into()]);

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("a", "git@host:org-a"),
                registry_section("b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let m = r.resolve(&p).unwrap();
        assert_eq!(m.registry_name.as_deref(), Some("a"));
        assert_eq!(m.resolved.version.to_string(), "0.2.0");
        assert!(!m.overridden);
        assert_eq!(m.source_url, "git@host:org-a/org.vibevm.wal.git");
        assert_eq!(m.source_ref.as_deref(), Some("v0.2.0"));
    }

    #[test]
    fn resolve_falls_through_to_next_registry_on_unknown_package() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // First registry: no seed for this URL → RepoNotFound → fall through.
        fake.seed_tags("git@host:org-b/org.vibevm.wal.git", vec!["v0.5.0".into()]);

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("a", "git@host:org-a"),
                registry_section("b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let m = r.resolve(&p).unwrap();
        assert_eq!(m.registry_name.as_deref(), Some("b"));
        assert_eq!(m.resolved.version.to_string(), "0.5.0");
    }

    #[test]
    fn resolve_aggregates_walk_attempts_when_no_registry_has_it() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // No seed for any URL — both registries return UnknownPackage
        // for `flow:ghost`. The resolver collects both into the
        // aggregate `PackageNotFoundEverywhere` report so the
        // operator sees per-registry status.

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("a", "git@host:org-a"),
                registry_section("b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/ghost").unwrap();
        let err = r.resolve(&p).unwrap_err();
        match err {
            RegistryError::PackageNotFoundEverywhere {
                group,
                name,
                summary,
                attempts,
            } => {
                assert_eq!(attempts.len(), 2, "expected 2 walk attempts: {attempts:?}");
                assert_eq!(group, org());
                assert_eq!(name, "ghost");
                assert!(
                    summary.contains("a") && summary.contains("b"),
                    "summary must list both walked registries: {summary}"
                );
                assert!(
                    summary.contains("not found"),
                    "expected `not found` status label: {summary}"
                );
            }
            other => panic!("expected PackageNotFoundEverywhere with attempts, got: {other:?}"),
        }
    }

    #[test]
    fn resolve_unknown_when_no_registries_and_no_override() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake);
        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        assert!(matches!(err, RegistryError::UnknownPackage { .. }));
    }

    /// PROP-002 §2.3.1 strict-auth corollary: when
    /// `with_strict_auth(true)` is set, a 401 on an `auth = "none"`
    /// public registry halts instead of walking past. Useful for
    /// CI / cron where the operator wants to gate "must come from
    /// the private registry; if its 401 leaks to a public fallback,
    /// fail loudly". Default behaviour (without strict_auth) is
    /// covered by `resolve_walks_past_auth_failed_when_registry_is_public`
    /// below.
    #[test]
    fn resolve_strict_auth_halts_on_public_401_instead_of_walking() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Primary public registry returns AuthFailed; secondary has
        // the package. With strict_auth on, the resolver must NOT
        // walk to the secondary.
        fake.seed_auth_failure("git@host:org-a/org.vibevm.wal.git");
        fake.seed_tags("git@host:org-b/org.vibevm.wal.git", vec!["v0.5.0".into()]);

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("public-a", "git@host:org-a"),
                registry_section("public-b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake,
        )
        .with_strict_auth(true);
        assert!(r.strict_auth());

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        match err {
            RegistryError::Git(GitError::AuthFailed { url }) => {
                assert!(
                    url.contains("org-a"),
                    "halt error must surface the failing registry's URL: {url}"
                );
            }
            other => panic!(
                "strict-auth: expected halt with AuthFailed on first registry, got: {other:?}"
            ),
        }
    }

    /// PROP-002 §2.3.1: 401 / 403 on an `auth = "none"` registry is
    /// reclassified as "no public answer here", and the resolver
    /// walks to the next registry. Closes the original opencode
    /// regression where GitVerse's 401 (its policy on missing
    /// public repos) halted resolution before GitHub got a chance.
    #[test]
    fn resolve_walks_past_auth_failed_when_registry_is_public() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // First registry: returns AuthFailed (think GitVerse-style 401
        // for a missing public repo). Second registry: serves the
        // package.
        fake.seed_auth_failure("git@host:org-a/org.vibevm.wal.git");
        fake.seed_tags("git@host:org-b/org.vibevm.wal.git", vec!["v0.5.0".into()]);

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("public-a", "git@host:org-a"),
                registry_section("public-b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let m = r
            .resolve(&p)
            .expect("public-a's AuthFailed must walk to public-b, not halt");
        assert_eq!(m.registry_name.as_deref(), Some("public-b"));
        assert_eq!(m.resolved.version.to_string(), "0.5.0");
    }

    /// PROP-002 §2.3.1: 401 / 403 on an authenticated registry
    /// (`auth = "token-env"` in this test) is a real `AuthFailed`
    /// halt — the operator declared this registry expects creds and
    /// the creds presented were rejected (or absent / expired).
    /// Walking past would mask the configuration error.
    ///
    /// We use `open_with_explicit_token` indirectly through the
    /// resolver's `from_manifest` path by pre-loading the env-var.
    /// Skipping the env layer in this test would require a
    /// resolver-level test-only constructor; instead we set the
    /// env via a helper that doesn't need `unsafe` (read-only,
    /// because the value is already there from the caller).
    ///
    /// In this test we don't actually need a token *value* — the
    /// walk-vs-halt decision is gated on `auth_kind`, not on
    /// whether the token resolved. We mark the registry
    /// `auth = "token-env"` with no env-var set; the resolver's
    /// `MissingToken` precheck does NOT fire because the
    /// `MissingToken` path only triggers when a git invocation is
    /// attempted, and AuthFailed is already on the wire from
    /// `list_tags`. So this test exercises the AuthFailed-on-
    /// authenticated-registry branch directly.
    #[test]
    fn resolve_halts_on_auth_failed_against_authenticated_registry() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // The authenticated registry returns AuthFailed.
        fake.seed_auth_failure("https://internal.example.com/vibespecs/org.vibevm.wal.git");
        // A second registry has the package — but the resolver must
        // NOT walk to it (the operator declared the first registry
        // as authenticated; AuthFailed is information they need).
        fake.seed_tags(
            "git@host:org-public/org.vibevm.wal.git",
            vec!["v0.5.0".into()],
        );

        // Stash the token in an env-var so `from_manifest` can find
        // one. We can't `set_var` from this test (`forbid(unsafe_code)`),
        // so we use a name that's already in the test process env or
        // leverage a side door. Simplest: declare `auth = token-env`
        // with NO `token_env` field — `resolve_token_env_name` will
        // derive a name from the host that almost certainly isn't
        // set, so the registry opens with `effective_token = None`.
        // The MissingToken precheck would normally fire, but our
        // FakeBackend's `list_tags` returns AuthFailed first, before
        // any token-aware code path runs. (The AuthFailed comes from
        // the seeded backend, simulating a real 401 from the host;
        // we bypass the precheck by virtue of how the fake works.)
        //
        // Actually simpler still: just set `auth = "credential-helper"`,
        // which never triggers MissingToken (the precheck only fires
        // for `TokenEnv`). The walk-vs-halt rule applies the same:
        // any `auth != None` halts on AuthFailed.
        let auth_section = RegistrySection {
            name: "internal".to_string(),
            url: "https://internal.example.com/vibespecs".to_string(),
            r#ref: "main".to_string(),
            naming: NamingConvention::Fqdn,
            auth: vibe_core::manifest::AuthKind::CredentialHelper,
            token_env: None,
        };
        let r = build_resolver(
            cache.path(),
            vec![
                auth_section,
                registry_section("public-fallback", "git@host:org-public"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        match err {
            RegistryError::Git(GitError::AuthFailed { url }) => {
                assert!(
                    url.contains("internal.example.com"),
                    "halt error must surface the authenticated registry's URL, got: {url}"
                );
            }
            other => panic!(
                "expected halt with AuthFailed against authenticated registry, got: {other:?}"
            ),
        }
    }

    /// PROP-002 §2.2.1 + §2.3.1 corollary: when a registry is
    /// declared `auth = "token-env"` but the env-var is absent, the
    /// resolver must surface `MissingToken` immediately on that
    /// registry — it must NOT silently walk past, because doing so
    /// would mask the operator's configuration error (the
    /// authenticated registry was supposed to answer; a missing
    /// token is a setup mistake, not a "package not here" signal).
    #[test]
    fn resolve_halts_on_missing_token_for_authenticated_registry() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Public fallback also has the package — must NOT be walked
        // past the missing-token registry.
        fake.seed_tags(
            "git@host:org-public/org.vibevm.wal.git",
            vec!["v0.5.0".into()],
        );

        // `auth = token-env` with an env-var that resolves to nothing
        // (deliberately exotic name unlikely to be set anywhere).
        let env_name = "VIBEVM_REGISTRY_TOKEN_DEFINITELY_NOT_SET_ABCXYZ";
        let r = build_resolver(
            cache.path(),
            vec![
                registry_section_token_env(
                    "internal",
                    "https://internal.example/vibespecs",
                    env_name,
                ),
                registry_section("public-fallback", "git@host:org-public"),
            ],
            vec![],
            vec![],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        match err {
            RegistryError::MissingToken { registry, env_var } => {
                assert_eq!(registry, "internal");
                assert_eq!(env_var, env_name);
            }
            other => panic!(
                "expected MissingToken halt, got: {other:?}; resolver must NOT walk past missing-token registries"
            ),
        }
    }

    #[test]
    fn override_short_circuits_registry_resolution() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Registry has flow:wal at 0.2.0, but override pins to a fork.
        fake.seed_tags("git@host:org-a/org.vibevm.wal.git", vec!["v0.2.0".into()]);
        // Override URL: serve a manifest pinned at "my-fix" branch.
        fake.seed_file(
            "git@my-fork:vibevm/wal-fork.git",
            "my-fix",
            "vibe.toml",
            manifest_text("wal", "flow", "0.2.0").into_bytes(),
        );

        let ovr = OverrideSection {
            pkgref: "org.vibevm/wal".to_string(),
            source_url: "git@my-fork:vibevm/wal-fork.git".to_string(),
            r#ref: Some("my-fix".to_string()),
            reason: Some("waiting on upstream PR".to_string()),
        };

        let r = build_resolver(
            cache.path(),
            vec![registry_section("a", "git@host:org-a")],
            vec![],
            vec![ovr],
            fake,
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let m = r.resolve(&p).unwrap();
        assert!(m.overridden);
        assert!(m.registry_name.is_none());
        assert_eq!(m.source_url, "git@my-fork:vibevm/wal-fork.git");
        assert_eq!(m.source_ref.as_deref(), Some("my-fix"));
        assert_eq!(m.resolved.version.to_string(), "0.2.0");
    }

    #[test]
    fn override_uses_default_ref_when_unspecified() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        fake.seed_file(
            "git@my-fork:vibevm/wal-fork.git",
            DEFAULT_OVERRIDE_REF,
            "vibe.toml",
            manifest_text("wal", "flow", "1.0.0").into_bytes(),
        );

        let ovr = OverrideSection {
            pkgref: "org.vibevm/wal".to_string(),
            source_url: "git@my-fork:vibevm/wal-fork.git".to_string(),
            r#ref: None,
            reason: None,
        };

        let r = build_resolver(cache.path(), vec![], vec![], vec![ovr], fake);
        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let m = r.resolve(&p).unwrap();
        assert_eq!(m.source_ref.as_deref(), Some(DEFAULT_OVERRIDE_REF));
        assert_eq!(m.resolved.version.to_string(), "1.0.0");
    }

    #[test]
    fn override_refuses_when_manifest_identity_mismatches() {
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // The manifest at the pinned ref claims to be `flow:atomic-commits`,
        // but the override is for `flow:wal`. Refuse loudly — silently
        // installing as `flow:wal` would corrupt the lockfile.
        fake.seed_file(
            "git@my-fork:vibevm/wal-fork.git",
            "main",
            "vibe.toml",
            manifest_text("atomic-commits", "flow", "0.1.0").into_bytes(),
        );

        let ovr = OverrideSection {
            pkgref: "org.vibevm/wal".to_string(),
            source_url: "git@my-fork:vibevm/wal-fork.git".to_string(),
            r#ref: None,
            reason: None,
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![ovr], fake);
        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        match err {
            RegistryError::MalformedMeta { reason, .. } => {
                assert!(reason.contains("refusing to install"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn fetch_dispatches_to_registry_that_resolved() {
        let cache = tempdir().unwrap();
        let pkg_cache = tempdir().unwrap();
        let upstream = tempdir().unwrap();

        // Build an upstream tree at the second registry's URL.
        let pkg_root = upstream.path().join("pkg");
        fs::create_dir_all(&pkg_root).unwrap();
        fs::write(
            pkg_root.join("vibe.toml"),
            manifest_text("wal", "flow", "0.5.0"),
        )
        .unwrap();

        let fake = Arc::new(FakeBackend::default());
        fake.seed_tags("git@host:org-b/org.vibevm.wal.git", vec!["v0.5.0".into()]);
        fake.seed_bootstrap("git@host:org-b/org.vibevm.wal.git", pkg_root.clone());

        let r = build_resolver(
            cache.path(),
            vec![
                registry_section("a", "git@host:org-a"), // empty (no seed)
                registry_section("b", "git@host:org-b"),
            ],
            vec![],
            vec![],
            fake.clone(),
        );

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let resolution = r.resolve(&p).unwrap();
        let cached = r.fetch(&resolution, pkg_cache.path()).unwrap();

        assert_eq!(cached.registry_name.as_deref(), Some("b"));
        assert!(!cached.overridden);
        assert_eq!(cached.source_uri, "git@host:org-b/org.vibevm.wal.git");
        assert_eq!(cached.source_ref.as_deref(), Some("v0.5.0"));
        assert_eq!(cached.package_meta().version.to_string(), "0.5.0");
        assert!(cached.cache_dir.join("vibe.toml").exists());
        assert!(!cached.cache_dir.join(".git").exists());
        // Bootstrap exactly once — only against registry "b".
        assert_eq!(fake.bootstrap_count(), 1);
    }

    #[test]
    fn fetch_override_clones_into_overrides_subtree_and_marks_overridden() {
        let cache = tempdir().unwrap();
        let pkg_cache = tempdir().unwrap();
        let upstream = tempdir().unwrap();

        let pkg_root = upstream.path().join("pkg");
        fs::create_dir_all(&pkg_root).unwrap();
        fs::write(
            pkg_root.join("vibe.toml"),
            manifest_text("wal", "flow", "0.9.0"),
        )
        .unwrap();

        let fake = Arc::new(FakeBackend::default());
        // For override: backend serves manifest via `fetch_file_at_ref`
        // (resolve), then clones via `bootstrap` (fetch).
        fake.seed_file(
            "git@my-fork:vibevm/wal-fork.git",
            "my-fix",
            "vibe.toml",
            manifest_text("wal", "flow", "0.9.0").into_bytes(),
        );
        fake.seed_bootstrap("git@my-fork:vibevm/wal-fork.git", pkg_root.clone());

        let ovr = OverrideSection {
            pkgref: "org.vibevm/wal".to_string(),
            source_url: "git@my-fork:vibevm/wal-fork.git".to_string(),
            r#ref: Some("my-fix".to_string()),
            reason: Some("PR pending".to_string()),
        };

        let r = build_resolver(cache.path(), vec![], vec![], vec![ovr], fake.clone());

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let resolution = r.resolve(&p).unwrap();
        let cached = r.fetch(&resolution, pkg_cache.path()).unwrap();

        assert!(cached.overridden);
        assert!(cached.registry_name.is_none());
        assert_eq!(cached.source_uri, "git@my-fork:vibevm/wal-fork.git");
        assert_eq!(cached.source_ref.as_deref(), Some("my-fix"));
        assert_eq!(cached.package_meta().version.to_string(), "0.9.0");
        // Override clone lives under
        // `cache_root/__overrides__/<group>.<name>/clone/` — keyed by
        // `(group, name)` identity (PROP-008).
        let overrides_root = cache
            .path()
            .join("__overrides__")
            .join("org.vibevm.wal")
            .join("clone");
        assert!(overrides_root.join(".git").exists());
        // Materialised cache holds payload only.
        assert!(cached.cache_dir.join("vibe.toml").exists());
        assert!(!cached.cache_dir.join(".git").exists());
    }
}
