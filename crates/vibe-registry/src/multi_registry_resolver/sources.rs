//! Path-source and git-source dispatch — the `[requires.packages]`
//! table-form declarations that bypass the registry walk: git-source
//! resolution / fetch (PROP-002 §2.4.1) and path-source resolution /
//! fetch (PROP-007 §2.5).

specmark::scope!("spec://vibevm/modules/vibe-registry/PROP-002#git-source");

use super::*;

impl MultiRegistryResolver {
    /// Resolve a `[requires.packages]` git-source declaration
    /// (PROP-002 §2.4.1). Synthesises a single-package
    /// `GitPackageRegistry` pointing at `dep.url`, fetches
    /// `vibe.toml` at the declared `tag`/`branch`/`rev`,
    /// verifies the `(group, name)` identity matches and the optional
    /// `version` constraint is satisfied, returns a `MultiResolution`
    /// with `is_git_source = true`.
    pub(super) fn resolve_git_source(
        &self,
        pkgref: &PackageRef,
        dep: &GitPackageDep,
    ) -> Result<MultiResolution, RegistryError> {
        let synthetic_name = format!("git-source-{}-{}", dep.group, dep.name);
        let refname = dep.ref_kind.as_str().to_string();
        let reg = GitPackageRegistry::open_single_package(
            &synthetic_name,
            &dep.url,
            &refname,
            &self.cache_root,
            Arc::clone(&self.backend),
            DEFAULT_FRESHNESS_SECS,
            dep.auth,
            dep.token_env.as_deref(),
        )?;
        let manifest = reg.fetch_manifest_at_ref(&dep.group, &dep.name, &refname)?;
        let meta = manifest
            .require_package()
            .map_err(|e| RegistryError::MalformedMeta {
                path: PathBuf::from(format!("{}@{}:{}", dep.url, refname, Manifest::FILENAME)),
                reason: e.to_string(),
            })?;
        // Sanity: the declaration says `(group, name)` but the repo's
        // manifest declares some other identity. Refuse to install —
        // pulling code under a misnamed slot would silently misroute
        // on disk and confuse downstream commands. `kind` is metadata
        // (PROP-008 §2.3) — not compared here.
        if meta.group != dep.group || meta.name != pkgref.name {
            return Err(RegistryError::MalformedMeta {
                path: PathBuf::from(format!("{}@{}:{}", dep.url, refname, Manifest::FILENAME)),
                reason: format!(
                    "git-source `{}/{}` points at a manifest declaring `{}/{}` — refusing to install",
                    dep.group, pkgref.name, meta.group, meta.name
                ),
            });
        }
        // Verify the optional version constraint, if the operator declared one.
        if let Some(spec) = &dep.version
            && !spec.matches(&meta.version)
        {
            return Err(RegistryError::MalformedMeta {
                path: PathBuf::from(format!("{}@{}:{}", dep.url, refname, Manifest::FILENAME)),
                reason: format!(
                    "git-source `{}/{}@{}` declares version `{}`, which does not satisfy the constraint `{}`",
                    dep.group, pkgref.name, refname, meta.version, spec
                ),
            });
        }
        let resolved = ResolvedPackage {
            group: dep.group.clone(),
            name: pkgref.name.clone(),
            version: meta.version.clone(),
            source_dir: self.git_source_clone_dir(&dep.group, &pkgref.name),
        };
        Ok(MultiResolution {
            resolved,
            registry_name: None,
            source_url: dep.url.clone(),
            source_ref: Some(refname),
            overridden: false,
            is_git_source: true,
            is_path_source: false,
            via_redirect: None,
            redirect_target_auth: vibe_core::manifest::AuthKind::None,
            redirect_target_token_env: None,
        })
    }

    /// Resolve a `[requires.packages]` path-source declaration
    /// (PROP-007 §2.5). The package lives in a local directory
    /// (`dep.package_dir`, already canonicalised by the workspace
    /// layer); there is no registry walk and no git clone. Reads the
    /// package's `vibe.toml`, verifies `(kind, name)` matches and the
    /// optional `version` constraint is satisfied, returns a
    /// `MultiResolution` with `is_path_source = true` and the source
    /// recorded as the workspace-relative path (`dep.workspace_rel`).
    pub(super) fn resolve_path_source(
        &self,
        pkgref: &PackageRef,
        dep: &ResolvedPathDep,
    ) -> Result<MultiResolution, RegistryError> {
        let manifest_path = dep.package_dir.join(Manifest::FILENAME);
        let manifest = Manifest::read(&manifest_path)?;
        let meta = manifest
            .require_package()
            .map_err(|e| RegistryError::MalformedMeta {
                path: manifest_path.clone(),
                reason: e.to_string(),
            })?;
        // Sanity: the declaration says `(group, name)` but the package's
        // own manifest declares some other identity. Refuse to install —
        // pulling code under a misnamed slot would silently misroute
        // on disk and confuse downstream commands. `kind` is metadata
        // (PROP-008 §2.3) — not compared here.
        if meta.group != dep.group || meta.name != pkgref.name {
            return Err(RegistryError::MalformedMeta {
                path: manifest_path.clone(),
                reason: format!(
                    "path-source `{}/{}` points at a manifest declaring `{}/{}` — refusing to install",
                    dep.group, pkgref.name, meta.group, meta.name
                ),
            });
        }
        // Verify the optional version constraint, if the path-dep
        // carried the dual-form `{ path, version }`. The resolved
        // version is the package's own `[package].version`.
        if let Some(spec) = &dep.version
            && !spec.matches(&meta.version)
        {
            return Err(RegistryError::MalformedMeta {
                path: manifest_path.clone(),
                reason: format!(
                    "path-source `{}/{}` at `{}` declares version `{}`, which does not satisfy the constraint `{}`",
                    dep.group, pkgref.name, dep.workspace_rel, meta.version, spec
                ),
            });
        }
        let resolved = ResolvedPackage {
            group: dep.group.clone(),
            name: pkgref.name.clone(),
            version: meta.version.clone(),
            source_dir: dep.package_dir.clone(),
        };
        Ok(MultiResolution {
            resolved,
            registry_name: None,
            // `source_url` records the workspace-relative path, never an
            // absolute path and never a URL — PROP-007 §2.5.
            source_url: dep.workspace_rel.clone(),
            source_ref: None,
            overridden: false,
            is_git_source: false,
            is_path_source: true,
            via_redirect: None,
            redirect_target_auth: vibe_core::manifest::AuthKind::None,
            redirect_target_token_env: None,
        })
    }

    /// Where git-source clones live —
    /// `<cache_root>/__git_sources__/<group>.<name>/clone/`. Distinct
    /// from registry-served clones and from override clones so a
    /// package that flips between resolution modes does not share
    /// state across modes. Keyed by `(group, name)` identity (PROP-008).
    fn git_source_clone_dir(&self, group: &Group, name: &str) -> PathBuf {
        self.cache_root
            .join("__git_sources__")
            .join(format!("{group}.{name}"))
            .join("clone")
    }

    /// Fetch a git-source-resolved package into the per-project cache.
    /// Same shape as `fetch_override` but threads `dep.auth` /
    /// `dep.token_env` through so private targets get token injection
    /// and the M1.14 scrub-from-`.git/config` discipline applies.
    pub(super) fn fetch_git_source(
        &self,
        resolution: &MultiResolution,
        project_cache: &Path,
        _expected_hash: Option<&str>,
    ) -> Result<CachedPackage, RegistryError> {
        let group = &resolution.resolved.group;
        let name = resolution.resolved.name.as_str();
        let qualified = format!("{group}/{name}");
        let dep =
            self.git_packages
                .get(&qualified)
                .ok_or_else(|| RegistryError::UnknownPackage {
                    group: group.clone(),
                    name: name.to_string(),
                })?;
        let refname = resolution
            .source_ref
            .clone()
            .unwrap_or_else(|| dep.ref_kind.as_str().to_string());

        // Synthesise a single-package registry just to leverage its
        // `package_repo_url` / `credentialed_url` plumbing for token
        // injection + scrub. The synthetic registry's clone path is
        // not used here — we clone into our own `__git_sources__`
        // sub-tree so the cache stays organised by resolution mode.
        let synthetic_name = format!("git-source-{group}-{name}");
        let reg = GitPackageRegistry::open_single_package(
            &synthetic_name,
            &dep.url,
            &refname,
            &self.cache_root,
            Arc::clone(&self.backend),
            DEFAULT_FRESHNESS_SECS,
            dep.auth,
            dep.token_env.as_deref(),
        )?;
        let plain_url = reg.package_repo_url(group, name)?;
        let credentialed = reg.credentialed_url(&plain_url);

        let clone_dir = self.git_source_clone_dir(group, name);
        ensure_clone_at(self.backend.as_ref(), &credentialed, &refname, &clone_dir)?;
        // Token-discipline (M1.14): scrub any credentialed URL from
        // the freshly-bootstrapped `.git/config` so the token does not
        // persist on disk. Best-effort — if the backend has no
        // `set_remote_url` impl, the default is a no-op (the
        // credentialed URL was only ever in-memory anyway for
        // backends that don't write a `.git/config`).
        if credentialed != plain_url {
            self.backend
                .set_remote_url(&clone_dir, "origin", &plain_url)
                .ok();
        }

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
            source_uri: plain_url,
            registry_name: None,
            source_ref: Some(refname),
            resolved_commit: None,
            overridden: false,
            is_git_source: true,
            is_path_source: false,
            via_redirect: None,
        })
    }

    /// Fetch a path-source-resolved package into the per-project cache.
    /// Unlike git-source there is NO git clone — a path-source package
    /// is a local directory. `resolution.resolved.source_dir` carries
    /// the resolver-supplied absolute `package_dir`; we copy its content
    /// (excluding any `.git/`) straight into the per-project package
    /// cache and hash the copied tree. PROP-007 §2.5.
    pub(super) fn fetch_path_source(
        &self,
        resolution: &MultiResolution,
        project_cache: &Path,
    ) -> Result<CachedPackage, RegistryError> {
        let group = &resolution.resolved.group;
        let name = resolution.resolved.name.as_str();
        // The resolver stored the canonicalised package directory on
        // `resolved.source_dir`; `workspace_rel` is in `source_url`.
        let package_dir = resolution.resolved.source_dir.clone();
        let workspace_rel = resolution.source_url.clone();

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
        // Copy the local directory's content into the cache, excluding
        // any `.git/` — same exclusion the registry / override / git-
        // source paths apply. A path-source package directory is
        // ordinarily not a git checkout of its own, but a workspace
        // member can be, so the exclusion is load-bearing.
        copy_dir_excluding_git(&package_dir, &dest)?;
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
                source_dir: package_dir,
            },
            cache_dir: dest,
            manifest,
            content_hash,
            // `source_uri` records the workspace-relative path — the
            // lockfile `source_url` for a path entry. Never a URL,
            // never absolute.
            source_uri: workspace_rel,
            registry_name: None,
            source_ref: None,
            resolved_commit: None,
            overridden: false,
            is_git_source: false,
            is_path_source: true,
            via_redirect: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    use crate::multi_registry_resolver::test_support::*;

    #[test]
    fn resolve_dispatches_to_git_source_short_circuiting_registries() {
        // M1.15: a `[requires.packages]` git-source declaration bypasses
        // the registry walk for that pkgref. The resolver synthesises a
        // single-package registry pointing at `dep.url`, fetches the
        // manifest at the declared ref, returns
        // `MultiResolution { is_git_source: true, ... }`.
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Registry has nothing — would fail without git-source dispatch.
        // git-source URL has the manifest at v0.3.0 tag.
        let url = "git@host:owner/flow-internal.git";
        fake.seed_file(
            url,
            "v0.3.0",
            "vibe.toml",
            manifest_text("internal", "flow", "0.3.0").into_bytes(),
        );

        let dep = vibe_core::manifest::GitPackageDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "internal".to_string(),
            url: url.to_string(),
            ref_kind: vibe_core::manifest::GitRefKind::Tag("v0.3.0".to_string()),
            version: None,
            auth: vibe_core::manifest::AuthKind::None,
            token_env: None,
        };
        let r =
            build_resolver(cache.path(), vec![], vec![], vec![], fake).with_git_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/internal").unwrap();
        let m = r.resolve(&p).expect("git-source resolution must succeed");
        assert!(m.is_git_source);
        assert!(!m.overridden);
        assert_eq!(m.registry_name, None);
        assert_eq!(m.source_url, url);
        assert_eq!(m.source_ref.as_deref(), Some("v0.3.0"));
        assert_eq!(m.resolved.version.to_string(), "0.3.0");
    }

    #[test]
    fn resolve_git_source_rejects_name_mismatch() {
        // The repo's `vibe.toml` declares `org.vibevm/something-else`,
        // but the consumer's `[requires.packages]` declared
        // `org.vibevm/internal` pointing at this URL. Refuse — pulling
        // code under the wrong pkgref slot would silently misroute on
        // disk. `kind` is metadata (PROP-008 §2.3); the `name` mismatch
        // is what catches this.
        let cache = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        let url = "git@host:owner/wrong-pkg.git";
        fake.seed_file(
            url,
            "v0.1.0",
            "vibe.toml",
            manifest_text("something-else", "feat", "0.1.0").into_bytes(),
        );
        let dep = vibe_core::manifest::GitPackageDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "internal".to_string(),
            url: url.to_string(),
            ref_kind: vibe_core::manifest::GitRefKind::Tag("v0.1.0".to_string()),
            version: None,
            auth: vibe_core::manifest::AuthKind::None,
            token_env: None,
        };
        let r =
            build_resolver(cache.path(), vec![], vec![], vec![], fake).with_git_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/internal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("refusing to install"),
            "expected identity-mismatch refusal, got: {msg}"
        );
    }

    // ----- path-source (PROP-007 §2.5) ------------------------------

    /// Lay down a path-source package directory under `parent`:
    /// `<parent>/<dirname>/vibe.toml` carrying a `[package]` table.
    /// Returns the package directory.
    fn seed_path_package(
        parent: &Path,
        dirname: &str,
        name: &str,
        kind: &str,
        version: &str,
    ) -> PathBuf {
        let dir = parent.join(dirname);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("vibe.toml"), manifest_text(name, kind, version)).unwrap();
        dir
    }

    #[test]
    fn resolve_dispatches_to_path_source_short_circuiting_registries() {
        // PROP-007 §2.5: a `[requires.packages]` path-source declaration
        // bypasses the registry walk for that pkgref. The resolver reads
        // the package's `vibe.toml` straight off the local directory and
        // returns `MultiResolution { is_path_source: true, ... }`.
        let cache = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // Registry has nothing — would fail without path-source dispatch.
        let pkg_dir = seed_path_package(ws.path(), "flow-internal", "internal", "flow", "0.3.0");

        let dep = ResolvedPathDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "internal".to_string(),
            version: None,
            package_dir: pkg_dir.clone(),
            workspace_rel: "flow-internal".to_string(),
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake)
            .with_path_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/internal").unwrap();
        let m = r.resolve(&p).expect("path-source resolution must succeed");
        assert!(m.is_path_source);
        assert!(!m.is_git_source);
        assert!(!m.overridden);
        assert_eq!(m.registry_name, None);
        // source_url carries the workspace-relative path, never an
        // absolute path and never a URL.
        assert_eq!(m.source_url, "flow-internal");
        assert_eq!(m.source_ref, None);
        assert_eq!(m.resolved.version.to_string(), "0.3.0");
    }

    #[test]
    fn resolve_path_source_rejects_name_mismatch() {
        // The package's `vibe.toml` declares `org.vibevm/something-else`,
        // but the consumer's `[requires.packages]` declared
        // `org.vibevm/internal` pointing at this directory. Refuse —
        // installing code under a misnamed slot would silently misroute
        // on disk. `kind` is metadata (PROP-008 §2.3); the `name`
        // mismatch is what catches this.
        let cache = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        let pkg_dir = seed_path_package(ws.path(), "wrong-pkg", "something-else", "feat", "0.1.0");

        let dep = ResolvedPathDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "internal".to_string(),
            version: None,
            package_dir: pkg_dir,
            workspace_rel: "wrong-pkg".to_string(),
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake)
            .with_path_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/internal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("refusing to install"),
            "expected identity-mismatch refusal, got: {msg}"
        );
    }

    #[test]
    fn resolve_path_source_rejects_version_constraint_mismatch() {
        // The path-dep carried a dual-form `{ path, version }` constraint
        // that the package's own `[package].version` does not satisfy.
        // Refuse — same shape as the git-source version check.
        let cache = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        let pkg_dir = seed_path_package(ws.path(), "flow-wal", "wal", "flow", "0.1.0");

        let dep = ResolvedPathDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "wal".to_string(),
            // Package is 0.1.0; constraint demands ^0.3 — mismatch.
            version: Some(VersionSpec::parse("^0.3").unwrap()),
            package_dir: pkg_dir,
            workspace_rel: "flow-wal".to_string(),
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake)
            .with_path_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/wal").unwrap();
        let err = r.resolve(&p).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("does not satisfy the constraint"),
            "expected version-constraint refusal, got: {msg}"
        );
    }

    #[test]
    fn resolve_path_source_wins_over_same_pkgref_git_source() {
        // PROP-007 §2.5 priority: a pkgref declared as BOTH path-source
        // and git-source resolves via path-source — path-source sits one
        // notch above git-source in the resolution order.
        let cache = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());
        // path-source package: version 0.5.0.
        let pkg_dir = seed_path_package(ws.path(), "flow-dual", "dual", "flow", "0.5.0");
        // git-source for the SAME pkgref: a different version on a URL.
        let git_url = "git@host:owner/flow-dual.git";
        fake.seed_file(
            git_url,
            "v9.9.9",
            "vibe.toml",
            manifest_text("dual", "flow", "9.9.9").into_bytes(),
        );

        let path_dep = ResolvedPathDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "dual".to_string(),
            version: None,
            package_dir: pkg_dir,
            workspace_rel: "flow-dual".to_string(),
        };
        let git_dep = vibe_core::manifest::GitPackageDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "dual".to_string(),
            url: git_url.to_string(),
            ref_kind: vibe_core::manifest::GitRefKind::Tag("v9.9.9".to_string()),
            version: None,
            auth: vibe_core::manifest::AuthKind::None,
            token_env: None,
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake)
            .with_git_packages(vec![git_dep])
            .with_path_packages(vec![path_dep]);

        let p = PackageRef::parse("org.vibevm/dual").unwrap();
        let m = r.resolve(&p).expect("path-source must win and resolve");
        assert!(m.is_path_source, "path-source must win over git-source");
        assert!(!m.is_git_source);
        // The path-source version (0.5.0), not the git-source (9.9.9).
        assert_eq!(m.resolved.version.to_string(), "0.5.0");
        assert_eq!(m.source_url, "flow-dual");
    }

    #[test]
    fn fetch_path_source_copies_local_dir_and_computes_hash() {
        // PROP-007 §2.5: fetching a path-source package copies the local
        // directory's content into the per-project package cache,
        // excludes any `.git/`, and computes a content_hash over the
        // copied tree. No git clone happens.
        let cache = tempdir().unwrap();
        let pkg_cache = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let fake = Arc::new(FakeBackend::default());

        // Path-source package with a regular file AND a `.git/` subtree
        // that must NOT make it into the cache.
        let pkg_dir = seed_path_package(ws.path(), "flow-local", "local", "flow", "0.2.0");
        fs::write(pkg_dir.join("README.md"), "# local package\n").unwrap();
        let git_dir = pkg_dir.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();

        let dep = ResolvedPathDep {
            kind: Some(vibe_core::PackageKind::Flow),
            group: org(),
            name: "local".to_string(),
            version: None,
            package_dir: pkg_dir,
            workspace_rel: "flow-local".to_string(),
        };
        let r = build_resolver(cache.path(), vec![], vec![], vec![], fake.clone())
            .with_path_packages(vec![dep]);

        let p = PackageRef::parse("org.vibevm/local").unwrap();
        let resolution = r.resolve(&p).unwrap();
        let cached = r.fetch(&resolution, pkg_cache.path()).unwrap();

        assert!(cached.is_path_source);
        assert!(!cached.is_git_source);
        assert!(!cached.overridden);
        assert_eq!(cached.registry_name, None);
        assert_eq!(cached.source_ref, None);
        // source_uri is the workspace-relative path, recorded verbatim
        // as the lockfile `source_url` for a path entry.
        assert_eq!(cached.source_uri, "flow-local");
        assert_eq!(cached.package_meta().version.to_string(), "0.2.0");
        // Cache is populated with the package payload.
        assert!(cached.cache_dir.join("vibe.toml").exists());
        assert!(cached.cache_dir.join("README.md").exists());
        // `.git/` was excluded.
        assert!(!cached.cache_dir.join(".git").exists());
        // content_hash computed over the copied tree.
        assert!(cached.content_hash.starts_with("sha256:"));
        // No git clone — `bootstrap` was never invoked.
        assert_eq!(fake.bootstrap_count(), 0);
    }
}
