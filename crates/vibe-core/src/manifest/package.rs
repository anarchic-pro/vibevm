//! `vibe-package.toml` — the package manifest.
//!
//! Schema: `VIBEVM-SPEC.md` §7.3. The capability-based dependency vocabulary
//! (`[provides]` / `[requires]` / `[[requires_any]]` / `[obsoletes]` /
//! `[conflicts]`) is defined in [PROP-002 §2.9](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#capability).
//!
//! Legacy M0 / M1.1 compact form — `[dependencies] required = [...] conflicts =
//! [...]` — is still accepted on parse: values migrate transparently into
//! `requires.packages` / `conflicts.packages` via [`PackageManifest::normalize_legacy_deps`],
//! which is called from [`PackageManifest::read`]. On the next write the
//! manifest round-trips in modern form; the `[dependencies]` section
//! disappears.
//!
//! Rationale for the migration: empty-deps packages (every live v0.1.0 flow
//! today) round-trip unchanged because `PackageDependencies::is_empty()` is
//! true for them, and the modern serializer skips empty sections too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::capability_ref::CapabilityRef;
use crate::error::{Error, Result};
use crate::package_ref::{PackageKind, PackageRef, VersionSpec};

use super::i18n::I18nDecl;
use super::project::AuthKind;
use super::purl::Purl;
use super::{read_toml, write_toml};

/// The package manifest — `vibe-package.toml` inside a package directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub package: PackageMeta,

    #[serde(default)]
    pub compatibility: Compatibility,

    #[serde(default)]
    pub writes: WritesSection,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_snippet: Option<BootSnippet>,

    /// Capabilities this package advertises. Consumers reference them via
    /// `[requires].capabilities` and `[[requires_any]]`.
    #[serde(default, skip_serializing_if = "Provides::is_empty")]
    pub provides: Provides,

    /// Packages and capabilities this package requires. Resolved transitively
    /// by the depsolver at install time.
    #[serde(default, skip_serializing_if = "Requires::is_empty")]
    pub requires: Requires,

    /// Disjunctive requirements — any `one_of` list must be satisfied by at
    /// least one of its entries. Each `[[requires_any]]` is an independent
    /// disjunction.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_any: Vec<RequiresAny>,

    /// Packages this package supersedes. On upgrade, the solver treats an
    /// installed `obsoletes` target as evidence to remove it.
    #[serde(default, skip_serializing_if = "Obsoletes::is_empty")]
    pub obsoletes: Obsoletes,

    /// Direct exclusion — these cannot coexist with this package.
    #[serde(default, skip_serializing_if = "ConflictsList::is_empty")]
    pub conflicts: ConflictsList,

    /// Legacy v1 compact form — accepted for back-compat; migrated into
    /// `requires.packages` / `conflicts.packages` by
    /// [`PackageManifest::normalize_legacy_deps`]. After normalization this
    /// field is empty; serialization skips it.
    #[serde(default, skip_serializing_if = "PackageDependencies::is_empty")]
    pub dependencies: PackageDependencies,

    /// `[features]` — optional, conditionally-activated components per
    /// PROP-003 §2.4. Empty by default; absent on round-trip.
    #[serde(default, skip_serializing_if = "FeaturesTable::is_empty")]
    pub features: FeaturesTable,

    /// `[i18n]` — language preferences declared by this package.
    /// Per PROP-003 §2.7. Empty default = English-only.
    #[serde(default, skip_serializing_if = "I18nDecl::is_default")]
    pub i18n: I18nDecl,

    /// Conditional dependencies — `[target."context(<probe>)".dependencies]`
    /// per PROP-003 §2.6.1. Each key is a context predicate string; the
    /// value carries `[dependencies]` in `[requires]`-shape that get
    /// added to the dep graph when the predicate matches the resolved
    /// project state. Predicate language: `context(<key>)` where
    /// `<key>` is a capability/pkgref/interface tag that probes the
    /// `present` / `provides` channels of the activation context.
    /// Richer predicates (`if_files`, boolean composition) reserved for
    /// follow-up slices.
    #[serde(default, rename = "target", skip_serializing_if = "BTreeMap::is_empty")]
    pub conditional_deps: BTreeMap<String, ConditionalTarget>,
}

/// `[target."<predicate>"]` body — currently just `[dependencies]`,
/// shaped like `[requires]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionalTarget {
    #[serde(default, skip_serializing_if = "Requires::is_empty")]
    pub dependencies: Requires,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMeta {
    pub name: String,
    pub kind: PackageKind,
    pub version: semver::Version,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// PURL of the upstream library this package documents
    /// (PROP-003 §2.5.6). Optional; when set, ties the package's
    /// version to a specific upstream artefact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describes: Option<Purl>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_vibe_version: Option<String>,

    #[serde(default)]
    pub requires_kinds: Vec<PackageKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritesSection {
    #[serde(default)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootSnippet {
    /// Target filename inside `spec/boot/`, e.g. `10-flow-wal.md`.
    pub filename: String,
    /// Path to the source file inside the package directory, e.g.
    /// `boot/10-flow-wal.md`.
    pub source: PathBuf,
}

/// `[provides]` — capabilities this package advertises.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CapabilityRef>,
}

impl Provides {
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }
}

/// `[requires]` — concrete package pkgrefs plus capability requirements.
///
/// Wire form (what lands in TOML on disk) accepts two shapes for the
/// `packages` field:
///
/// 1. **Legacy array of pkgref strings** — `packages = ["flow:wal@^0.3"]`.
///    The pre-M1.15 shape; still parses for back-compat.
/// 2. **Modern table** — `[requires.packages]` with each key a bare pkgref
///    (`<kind>:<name>` without `@version`) and the value either:
///    - a constraint string (`"^0.3"`, `"=1.0"`, `"*"`) — registry-resolved,
///    - or an inline-table — registry-resolved with options
///      (`{ version = "..." }`) **or** git-source dependency declaration
///      (`{ git = "...", tag = "..." }` etc., per PROP-002 §2.4.1).
///
/// Round-trip writes the modern table form. Both shapes parse forever; the
/// array form is just never produced on write. Manifests that mix git-source
/// and registry-resolved declarations require the modern table form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "RequiresWire", try_from = "RequiresWire")]
pub struct Requires {
    /// Registry-resolved package dependencies (legacy or modern shape).
    pub packages: Vec<PackageRef>,
    /// Abstract capability requirements (RPM-family `Requires:` semantics).
    pub capabilities: Vec<CapabilityRef>,
    /// Git-source package dependencies — one git repository = one package
    /// (PROP-002 §2.4.1). Stored separately from `packages` so existing
    /// downstream code that iterates registry-resolved roots stays
    /// untouched; resolver and CLI code paths consult both fields when
    /// they need the full root set.
    pub git_packages: Vec<GitPackageDep>,
}

impl Requires {
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty() && self.capabilities.is_empty() && self.git_packages.is_empty()
    }

    /// Return every root pkgref (registry-resolved + git-source) in a
    /// single iterator. Order: `packages` first (insertion order),
    /// `git_packages` after, matching the wire-form serialization order
    /// when both are non-empty.
    pub fn iter_pkgrefs(&self) -> impl Iterator<Item = (PackageKind, &str)> {
        self.packages
            .iter()
            .map(|p| (p.kind, p.name.as_str()))
            .chain(
                self.git_packages
                    .iter()
                    .map(|g| (g.kind, g.name.as_str())),
            )
    }
}

/// `[requires.packages.<pkgref>]` inline-table value when the package is
/// sourced from an arbitrary git repository instead of a registry.
///
/// Spec: PROP-002 §2.4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPackageDep {
    pub kind: PackageKind,
    pub name: String,
    /// Full git URL of the single-package repository.
    pub url: String,
    /// Exactly one of `tag`, `branch`, `rev` — wire-grammar enforced.
    pub ref_kind: GitRefKind,
    /// Optional verification constraint. After resolving the package
    /// version from `ref_kind`, the constraint must be satisfied; mismatch
    /// is `VersionMismatch` at install time. `None` = accept whatever.
    pub version: Option<VersionSpec>,
    /// Per-source authentication regime (default `none`).
    pub auth: AuthKind,
    /// Env-var name when `auth = "token-env"`. `None` = derive from URL host.
    pub token_env: Option<String>,
}

/// Which kind of git ref the operator declared on a `[requires.packages.*]`
/// git-source entry. Exactly one of the three is required at parse time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRefKind {
    Tag(String),
    Branch(String),
    Rev(String),
}

impl GitRefKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tag(s) | Self::Branch(s) | Self::Rev(s) => s.as_str(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Tag(_) => "tag",
            Self::Branch(_) => "branch",
            Self::Rev(_) => "rev",
        }
    }
}

// ---------------------------------------------------------------------------
// Wire types for `Requires` — private; reached only via Serialize / Deserialize.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiresWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    packages: Option<RequiresPackagesWire>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<CapabilityRef>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum RequiresPackagesWire {
    /// Legacy: `packages = ["flow:wal@^0.3", ...]`.
    LegacyArray(Vec<PackageRef>),
    /// Modern: `[requires.packages]` table.
    ModernMap(BTreeMap<String, RequiresPackageEntryWire>),
}

// Manual `Deserialize` so that the inner `BadPackageRef` error from
// pkgref parsing surfaces directly instead of being wrapped in a
// generic "data did not match any variant of untagged enum" — TOML
// distinguishes array from table at parse time, so we can dispatch on
// that without trial-and-error.
impl<'de> Deserialize<'de> for RequiresPackagesWire {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = RequiresPackagesWire;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(
                    "an array of `<kind>:<name>[@<version>]` strings (legacy) \
                     or a table mapping `<kind>:<name>` to a constraint string \
                     or an inline-table",
                )
            }
            fn visit_seq<A>(self, seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let v = Vec::<PackageRef>::deserialize(
                    serde::de::value::SeqAccessDeserializer::new(seq),
                )?;
                Ok(RequiresPackagesWire::LegacyArray(v))
            }
            fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let m = BTreeMap::<String, RequiresPackageEntryWire>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )?;
                Ok(RequiresPackagesWire::ModernMap(m))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum RequiresPackageEntryWire {
    /// Bare constraint string: `"^0.3"`, `"=1.0"`, `"*"`.
    Constraint(String),
    /// Inline-table: registry-resolved with options OR git-source.
    Inline(InlinePackageDepWire),
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct InlinePackageDepWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    git: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth: Option<AuthKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token_env: Option<String>,
}

impl From<Requires> for RequiresWire {
    fn from(r: Requires) -> Self {
        let mut map: BTreeMap<String, RequiresPackageEntryWire> = BTreeMap::new();
        for p in &r.packages {
            let key = format!("{}:{}", p.kind, p.name);
            let value =
                RequiresPackageEntryWire::Constraint(version_spec_to_constraint_str(&p.version));
            map.insert(key, value);
        }
        for g in &r.git_packages {
            let key = format!("{}:{}", g.kind, g.name);
            let inline = InlinePackageDepWire {
                version: g.version.as_ref().map(version_spec_to_constraint_str),
                git: Some(g.url.clone()),
                tag: match &g.ref_kind {
                    GitRefKind::Tag(s) => Some(s.clone()),
                    _ => None,
                },
                branch: match &g.ref_kind {
                    GitRefKind::Branch(s) => Some(s.clone()),
                    _ => None,
                },
                rev: match &g.ref_kind {
                    GitRefKind::Rev(s) => Some(s.clone()),
                    _ => None,
                },
                auth: if g.auth == AuthKind::None {
                    None
                } else {
                    Some(g.auth)
                },
                token_env: g.token_env.clone(),
            };
            map.insert(key, RequiresPackageEntryWire::Inline(inline));
        }
        let packages = if map.is_empty() {
            None
        } else {
            Some(RequiresPackagesWire::ModernMap(map))
        };
        RequiresWire {
            packages,
            capabilities: r.capabilities,
        }
    }
}

impl TryFrom<RequiresWire> for Requires {
    type Error = String;

    fn try_from(w: RequiresWire) -> std::result::Result<Self, Self::Error> {
        let mut packages: Vec<PackageRef> = Vec::new();
        let mut git_packages: Vec<GitPackageDep> = Vec::new();
        match w.packages {
            None => {}
            Some(RequiresPackagesWire::LegacyArray(arr)) => packages = arr,
            Some(RequiresPackagesWire::ModernMap(map)) => {
                for (key, entry) in map {
                    let (kind, name) = parse_pkgref_key(&key).map_err(|e| e.to_string())?;
                    match entry {
                        RequiresPackageEntryWire::Constraint(spec_str) => {
                            let version =
                                VersionSpec::parse(&spec_str).map_err(|e| e.to_string())?;
                            packages.push(
                                PackageRef::new(kind, name, version).map_err(|e| e.to_string())?,
                            );
                        }
                        RequiresPackageEntryWire::Inline(inline) => {
                            if inline.git.is_some() {
                                git_packages.push(
                                    inline_to_git_dep(kind, name, inline)
                                        .map_err(|e| e.to_string())?,
                                );
                            } else {
                                packages.push(
                                    inline_to_registry_pkgref(kind, name, inline)
                                        .map_err(|e| e.to_string())?,
                                );
                            }
                        }
                    }
                }
            }
        }
        // Reject duplicate (kind, name) across packages and git_packages —
        // the resolver would otherwise have to pick one arbitrarily.
        for g in &git_packages {
            if packages.iter().any(|p| p.kind == g.kind && p.name == g.name) {
                return Err(format!(
                    "dependency `{}:{}` declared as both registry-resolved and git-source",
                    g.kind, g.name
                ));
            }
        }
        Ok(Requires {
            packages,
            capabilities: w.capabilities,
            git_packages,
        })
    }
}

fn parse_pkgref_key(key: &str) -> Result<(PackageKind, String)> {
    if key.contains('@') {
        return Err(Error::BadDependencyDecl {
            input: key.to_string(),
            reason: "version constraint must be the value, not part of the key".to_string(),
        });
    }
    // PackageRef::parse handles `<kind>:<name>` and yields VersionSpec::Latest
    // for keys without `@`. We've just rejected `@`, so this is safe.
    let pr = PackageRef::parse(key)?;
    Ok((pr.kind, pr.name))
}

fn inline_to_registry_pkgref(
    kind: PackageKind,
    name: String,
    inline: InlinePackageDepWire,
) -> Result<PackageRef> {
    let key_for_err = format!("{kind}:{name}");
    if inline.tag.is_some() || inline.branch.is_some() || inline.rev.is_some() {
        return Err(Error::BadDependencyDecl {
            input: key_for_err,
            reason: "registry-resolved dep cannot specify `tag`/`branch`/`rev` without `git`"
                .to_string(),
        });
    }
    if inline.auth.is_some() || inline.token_env.is_some() {
        return Err(Error::BadDependencyDecl {
            input: key_for_err,
            reason: "registry-resolved dep cannot specify `auth`/`token_env` without `git`"
                .to_string(),
        });
    }
    let version = match inline.version.as_deref() {
        Some(s) => VersionSpec::parse(s)?,
        None => VersionSpec::Latest,
    };
    PackageRef::new(kind, name, version)
}

fn inline_to_git_dep(
    kind: PackageKind,
    name: String,
    inline: InlinePackageDepWire,
) -> Result<GitPackageDep> {
    let key_for_err = format!("{kind}:{name}");
    let url = inline.git.expect("caller checked git is Some");
    let ref_kind = match (inline.tag, inline.branch, inline.rev) {
        (Some(t), None, None) => GitRefKind::Tag(t),
        (None, Some(b), None) => GitRefKind::Branch(b),
        (None, None, Some(r)) => GitRefKind::Rev(r),
        (None, None, None) => {
            return Err(Error::BadDependencyDecl {
                input: key_for_err,
                reason: "git-source requires exactly one of `tag`, `branch`, `rev`".to_string(),
            });
        }
        _ => {
            return Err(Error::BadDependencyDecl {
                input: key_for_err,
                reason: "git-source must specify exactly one of `tag`/`branch`/`rev`, not several"
                    .to_string(),
            });
        }
    };
    let version = match inline.version.as_deref() {
        Some(s) => Some(VersionSpec::parse(s)?),
        None => None,
    };
    Ok(GitPackageDep {
        kind,
        name,
        url,
        ref_kind,
        version,
        auth: inline.auth.unwrap_or_default(),
        token_env: inline.token_env,
    })
}

fn version_spec_to_constraint_str(spec: &VersionSpec) -> String {
    match spec {
        VersionSpec::Latest => "*".to_string(),
        VersionSpec::Req(req) => req.to_string(),
    }
}

/// `[[requires_any]]` — one entry per independent disjunction; `one_of` must
/// be satisfied by at least one of its alternatives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiresAny {
    pub one_of: Vec<PackageRef>,
}

/// `[obsoletes]` — packages this one supersedes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obsoletes {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<PackageRef>,
}

impl Obsoletes {
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// `[conflicts]` — packages that cannot coexist with this one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictsList {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<PackageRef>,
}

impl ConflictsList {
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// Legacy `[dependencies]` section — v1 compact form.
///
/// Kept on [`PackageManifest`] purely for backwards-compatible parsing. It
/// is emptied out in [`PackageManifest::normalize_legacy_deps`] and the
/// serializer skips it, so round-tripping a v1 manifest produces a v2 one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageDependencies {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<PackageRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<PackageRef>,
}

impl PackageDependencies {
    pub fn is_empty(&self) -> bool {
        self.required.is_empty() && self.conflicts.is_empty()
    }
}

impl PackageManifest {
    pub const FILENAME: &'static str = "vibe-package.toml";

    /// Read a manifest from disk and migrate any legacy v1 `[dependencies]`
    /// section into the modern fields. Callers always see a manifest in
    /// modern form regardless of which form was on disk.
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let mut m: PackageManifest = read_toml(path)?;
        m.normalize_legacy_deps();
        Ok(m)
    }

    /// Write a manifest to disk. Always serializes the modern form —
    /// `[dependencies]` is omitted even if it was non-empty before
    /// [`Self::normalize_legacy_deps`] was called.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        write_toml(path, self)
    }

    /// Migrate any `[dependencies]` entries into `requires.packages` and
    /// `conflicts.packages`. Idempotent — a no-op after the first call. Safe
    /// to call even on a modern manifest (just returns immediately).
    pub fn normalize_legacy_deps(&mut self) {
        if self.dependencies.is_empty() {
            return;
        }
        let legacy = std::mem::take(&mut self.dependencies);
        for r in legacy.required {
            self.requires.packages.push(r);
        }
        for c in legacy.conflicts {
            self.conflicts.packages.push(c);
        }
    }

    /// Produce a `PackageRef` pinning this package to its exact version.
    pub fn as_package_ref(&self) -> Result<PackageRef> {
        let req = semver::VersionReq::parse(&format!("={}", self.package.version))
            .expect("exact version string always parses as VersionReq");
        PackageRef::new(
            self.package.kind,
            self.package.name.clone(),
            VersionSpec::Req(req),
        )
    }
}

/// `[features]` table — feature definitions per PROP-003 §2.4.
///
/// Each feature maps to a list of activation strings; the strings can
/// be other feature names, dep-references (`dep:foo`, `foo?/feat`), or
/// subskill-references (`subskill:<path>`). The TOML form is a mix of
/// flat string-list keys plus a nested `exclusive` table; we deserialise
/// both via a manual visitor so the public API stays clean.
///
/// ```toml
/// [features]
/// default = ["wal-protocol"]
/// wal-protocol = []
/// rust-stack = ["subskill:stack/rust"]
/// python-stack = ["subskill:stack/python"]
///
/// [features.exclusive]
/// stacks = ["rust-stack", "python-stack"]
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeaturesTable {
    /// `feature-name` → list of activation strings.
    pub features: BTreeMap<String, Vec<String>>,
    /// `[features.exclusive]` — at-most-one named groups.
    pub exclusive: BTreeMap<String, Vec<String>>,
}

impl FeaturesTable {
    pub fn is_empty(&self) -> bool {
        self.features.is_empty() && self.exclusive.is_empty()
    }

    /// Convenience — list of features active by default
    /// (the `default` feature's activation list, if present).
    pub fn defaults(&self) -> &[String] {
        self.features
            .get("default")
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Look up a feature's activation list.
    pub fn get(&self, name: &str) -> Option<&[String]> {
        self.features.get(name).map(|v| v.as_slice())
    }
}

impl Serialize for FeaturesTable {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut total = self.features.len();
        if !self.exclusive.is_empty() {
            total += 1;
        }
        let mut m = s.serialize_map(Some(total))?;
        for (k, v) in &self.features {
            m.serialize_entry(k, v)?;
        }
        if !self.exclusive.is_empty() {
            m.serialize_entry("exclusive", &self.exclusive)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for FeaturesTable {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Receive as a generic `BTreeMap<String, toml::Value>` then split
        // into features (string lists) and the special `exclusive` table.
        let raw: BTreeMap<String, toml::Value> = BTreeMap::deserialize(d)?;
        let mut features: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut exclusive: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, v) in raw {
            if k == "exclusive" {
                let table: BTreeMap<String, Vec<String>> =
                    v.try_into().map_err(serde::de::Error::custom)?;
                exclusive = table;
                continue;
            }
            let arr: Vec<String> = v.try_into().map_err(serde::de::Error::custom)?;
            features.insert(k, arr);
        }
        Ok(FeaturesTable {
            features,
            exclusive,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_MODERN: &str = r#"
[package]
name = "welcome-page"
kind = "feat"
version = "0.3.0"
authors = ["Oleg Chirukhin <oleg@example.com>"]
license = "EULA"
description = "Welcome page demo feat"
keywords = ["welcome", "demo"]

[compatibility]
min_vibe_version = "0.1.0"
requires_kinds = []

[writes]
files = [
    "spec/feats/welcome-page/SPEC.md",
]

[boot_snippet]
filename = "40-feat-welcome-page.md"
source = "boot/40-feat-welcome-page.md"

[provides]
capabilities = ["ui:landing-page@0.3.0", "auth:oauth-callback"]

[requires]
packages = ["flow:atomic-commits@^0.1", "stack:rust-cli@^0.1"]
capabilities = ["db:any@>=1.0"]

[[requires_any]]
one_of = ["stack:rust-cli@^0.1", "stack:rust-axum@^0.2"]

[obsoletes]
packages = ["feat:welcome-page-legacy"]

[conflicts]
packages = ["feat:welcome-page-legacy-v2"]
"#;

    const FIXTURE_LEGACY: &str = r#"
[package]
name = "wal"
kind = "flow"
version = "0.3.0"
authors = ["Oleg Chirukhin <oleg@example.com>"]
license = "EULA"
description = "WAL"
keywords = ["wal"]

[compatibility]
min_vibe_version = "0.1.0"
requires_kinds = []

[writes]
files = [
    "spec/flows/wal/WAL-PROTOCOL.md",
]

[boot_snippet]
filename = "10-flow-wal.md"
source = "boot/10-flow-wal.md"

[dependencies]
required = ["flow:atomic-commits@^0.1"]
conflicts = ["flow:legacy-wal"]
"#;

    const FIXTURE_MINIMAL: &str = r#"
[package]
name = "tiny"
kind = "flow"
version = "0.0.1"
"#;

    #[test]
    fn parses_modern_form() {
        let m: PackageManifest = toml::from_str(FIXTURE_MODERN).unwrap();
        assert_eq!(m.package.kind, PackageKind::Feat);
        assert_eq!(m.package.name, "welcome-page");
        assert_eq!(m.provides.capabilities.len(), 2);
        assert_eq!(m.provides.capabilities[0].qualified(), "ui:landing-page");
        assert_eq!(m.requires.packages.len(), 2);
        assert_eq!(m.requires.packages[0].qualified_name(), "flow:atomic-commits");
        assert_eq!(m.requires.capabilities.len(), 1);
        assert_eq!(m.requires_any.len(), 1);
        assert_eq!(m.requires_any[0].one_of.len(), 2);
        assert_eq!(m.obsoletes.packages.len(), 1);
        assert_eq!(m.conflicts.packages.len(), 1);
        // Legacy section absent.
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn migrates_legacy_dependencies_section() {
        let mut m: PackageManifest = toml::from_str(FIXTURE_LEGACY).unwrap();
        // Before normalization: deps populated, requires empty.
        assert_eq!(m.dependencies.required.len(), 1);
        assert_eq!(m.dependencies.conflicts.len(), 1);
        assert!(m.requires.is_empty());
        assert!(m.conflicts.is_empty());

        m.normalize_legacy_deps();

        // After: deps empty, requires/conflicts populated.
        assert!(m.dependencies.is_empty());
        assert_eq!(m.requires.packages.len(), 1);
        assert_eq!(
            m.requires.packages[0].qualified_name(),
            "flow:atomic-commits"
        );
        assert_eq!(m.conflicts.packages.len(), 1);
        assert_eq!(
            m.conflicts.packages[0].qualified_name(),
            "flow:legacy-wal"
        );
    }

    #[test]
    fn normalize_is_idempotent() {
        let mut m: PackageManifest = toml::from_str(FIXTURE_LEGACY).unwrap();
        m.normalize_legacy_deps();
        let snapshot = m.clone();
        m.normalize_legacy_deps();
        assert_eq!(m, snapshot);
    }

    #[test]
    fn modern_form_roundtrips_unchanged() {
        let m: PackageManifest = toml::from_str(FIXTURE_MODERN).unwrap();
        let rendered = toml::to_string_pretty(&m).unwrap();
        let back: PackageManifest = toml::from_str(&rendered).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn legacy_form_roundtrips_into_modern() {
        let mut m: PackageManifest = toml::from_str(FIXTURE_LEGACY).unwrap();
        m.normalize_legacy_deps();
        let rendered = toml::to_string_pretty(&m).unwrap();
        // After normalization + write, the legacy `[dependencies]` table is gone.
        assert!(!rendered.contains("[dependencies]"));
        // M1.15: `[requires]` packages now serialise as a map-form table
        // `[requires.packages]`. The bare `[requires]` heading no longer
        // appears unless capabilities are non-empty.
        assert!(
            rendered.contains("[requires.packages]") || rendered.contains("[requires]"),
            "expected [requires.packages] or [requires] in:\n{rendered}"
        );
        assert!(rendered.contains("[conflicts]"));
        // And a re-read is byte-identical to the already-normalized state.
        let back: PackageManifest = toml::from_str(&rendered).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn parses_minimal_manifest() {
        let m: PackageManifest = toml::from_str(FIXTURE_MINIMAL).unwrap();
        assert_eq!(m.package.name, "tiny");
        assert!(m.writes.files.is_empty());
        assert!(m.boot_snippet.is_none());
        assert!(m.provides.is_empty());
        assert!(m.requires.is_empty());
        assert!(m.requires_any.is_empty());
        assert!(m.obsoletes.is_empty());
        assert!(m.conflicts.is_empty());
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn rejects_unknown_kind() {
        let raw = r#"
[package]
name = "wal"
kind = "widget"
version = "0.3.0"
"#;
        assert!(toml::from_str::<PackageManifest>(raw).is_err());
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let raw = r#"
[package]
name = "wal"
kind = "flow"
version = "0.3.0"

[bogus]
value = 1
"#;
        assert!(toml::from_str::<PackageManifest>(raw).is_err());
    }

    #[test]
    fn as_package_ref_pins_exact_version() {
        let m: PackageManifest = toml::from_str(FIXTURE_MODERN).unwrap();
        let r = m.as_package_ref().unwrap();
        assert_eq!(r.kind, PackageKind::Feat);
        assert_eq!(r.name, "welcome-page");
        let this = semver::Version::parse("0.3.0").unwrap();
        assert!(r.version.matches(&this));
        let other = semver::Version::parse("0.3.1").unwrap();
        assert!(!r.version.matches(&other));
    }

    #[test]
    fn rejects_invalid_pkgref_in_requires() {
        let raw = r#"
[package]
name = "foo"
kind = "flow"
version = "0.1.0"

[requires]
packages = ["not-a-valid-pkgref"]
"#;
        let err = toml::from_str::<PackageManifest>(raw).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid package reference") || msg.contains("missing `:`"));
    }

    #[test]
    fn parses_conditional_deps_block() {
        let raw = r#"
[package]
name = "x"
kind = "flow"
version = "0.1.0"

[target."context(stack:rust)".dependencies]
packages = ["flow:rust-best-practices@^0.1"]

[target."context(interface:build-system)".dependencies]
packages = ["flow:build-discipline@^0.1"]
"#;
        let m: PackageManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.conditional_deps.len(), 2);
        let rust_target = m
            .conditional_deps
            .get("context(stack:rust)")
            .expect("rust target");
        assert_eq!(rust_target.dependencies.packages.len(), 1);
        assert_eq!(
            rust_target.dependencies.packages[0].qualified_name(),
            "flow:rust-best-practices"
        );
        let build_target = m
            .conditional_deps
            .get("context(interface:build-system)")
            .expect("build target");
        assert_eq!(build_target.dependencies.packages.len(), 1);
    }

    #[test]
    fn rejects_invalid_capability_in_requires() {
        let raw = r#"
[package]
name = "foo"
kind = "flow"
version = "0.1.0"

[requires]
capabilities = ["no-colon-here"]
"#;
        let err = toml::from_str::<PackageManifest>(raw).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid capability reference") || msg.contains("missing `:`"));
    }

    // ---------------------------------------------------------------
    // M1.15 — git-source `[requires.packages]` map-form tests.
    // ---------------------------------------------------------------

    fn pkg_req_from_toml(raw: &str) -> Requires {
        // Wrap a `[requires]` body into a minimal valid `vibe-package.toml`
        // so we can exercise the manifest-level parser directly.
        let prefix = r#"
[package]
name = "p"
kind = "flow"
version = "0.1.0"

"#;
        let manifest: PackageManifest = toml::from_str(&format!("{prefix}{raw}")).unwrap();
        manifest.requires
    }

    #[test]
    fn map_form_bare_constraint_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:wal" = "^0.3"
"feat:auth" = "*"
"#,
        );
        assert_eq!(r.packages.len(), 2);
        assert!(r.git_packages.is_empty());
        // BTreeMap ordering: feat:auth < flow:wal alphabetically.
        assert_eq!(r.packages[0].qualified_name(), "feat:auth");
        assert_eq!(r.packages[1].qualified_name(), "flow:wal");
    }

    #[test]
    fn map_form_inline_table_with_version_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:wal" = { version = "^0.3" }
"#,
        );
        assert_eq!(r.packages.len(), 1);
        assert_eq!(r.packages[0].qualified_name(), "flow:wal");
        assert!(r.git_packages.is_empty());
    }

    #[test]
    fn git_source_with_tag_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:internal" = { git = "https://github.com/me/flow-internal", tag = "v0.1.0" }
"#,
        );
        assert!(r.packages.is_empty());
        assert_eq!(r.git_packages.len(), 1);
        let g = &r.git_packages[0];
        assert_eq!(g.kind, PackageKind::Flow);
        assert_eq!(g.name, "internal");
        assert_eq!(g.url, "https://github.com/me/flow-internal");
        assert!(matches!(&g.ref_kind, GitRefKind::Tag(t) if t == "v0.1.0"));
        assert_eq!(g.ref_kind.label(), "tag");
        assert!(g.version.is_none());
        assert_eq!(g.auth, AuthKind::None);
    }

    #[test]
    fn git_source_with_branch_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:experimental" = { git = "https://github.com/x/y", branch = "main" }
"#,
        );
        assert_eq!(r.git_packages.len(), 1);
        assert!(matches!(&r.git_packages[0].ref_kind, GitRefKind::Branch(b) if b == "main"));
    }

    #[test]
    fn git_source_with_rev_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:fork" = { git = "https://github.com/x/y", rev = "abc12345" }
"#,
        );
        assert_eq!(r.git_packages.len(), 1);
        assert!(matches!(&r.git_packages[0].ref_kind, GitRefKind::Rev(r) if r == "abc12345"));
    }

    #[test]
    fn git_source_with_auth_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:secret" = { git = "https://gitlab.acme.example/x/y", tag = "v1.0", auth = "token-env", token_env = "MY_TOKEN" }
"#,
        );
        let g = &r.git_packages[0];
        assert_eq!(g.auth, AuthKind::TokenEnv);
        assert_eq!(g.token_env.as_deref(), Some("MY_TOKEN"));
    }

    #[test]
    fn git_source_with_version_constraint_parses() {
        let r = pkg_req_from_toml(
            r#"[requires.packages]
"flow:checked" = { git = "https://x/y", tag = "v0.1.0", version = "^0.1" }
"#,
        );
        assert!(r.git_packages[0].version.is_some());
    }

    #[test]
    fn git_source_rejects_no_ref() {
        let raw = r#"[requires.packages]
"flow:bad" = { git = "https://x/y" }
"#;
        let err = toml::from_str::<PackageManifest>(&format!(
            "[package]\nname = \"p\"\nkind = \"flow\"\nversion = \"0.1.0\"\n\n{raw}"
        ))
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("requires exactly one of `tag`, `branch`, `rev`"),
            "expected ref-required message, got: {msg}"
        );
    }

    #[test]
    fn git_source_rejects_multiple_refs() {
        let raw = r#"[requires.packages]
"flow:bad" = { git = "https://x/y", tag = "v1", branch = "main" }
"#;
        let err = toml::from_str::<PackageManifest>(&format!(
            "[package]\nname = \"p\"\nkind = \"flow\"\nversion = \"0.1.0\"\n\n{raw}"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("exactly one of"));
    }

    #[test]
    fn registry_inline_rejects_git_fields() {
        let raw = r#"[requires.packages]
"flow:bad" = { version = "^0.3", tag = "v1" }
"#;
        let err = toml::from_str::<PackageManifest>(&format!(
            "[package]\nname = \"p\"\nkind = \"flow\"\nversion = \"0.1.0\"\n\n{raw}"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("without `git`"));
    }

    #[test]
    fn rejects_at_in_key() {
        // Keys are bare pkgrefs `<kind>:<name>`; version goes in the value.
        let raw = r#"[requires.packages]
"flow:wal@^0.3" = "*"
"#;
        let err = toml::from_str::<PackageManifest>(&format!(
            "[package]\nname = \"p\"\nkind = \"flow\"\nversion = \"0.1.0\"\n\n{raw}"
        ))
        .unwrap_err();
        assert!(err.to_string().contains("must be the value, not part of the key"));
    }

    // Note: duplicate `(kind, name)` between `packages` and `git_packages`
    // is **structurally impossible** through the TOML wire form — both land
    // in the same `[requires.packages]` table, and TOML grammar rejects
    // duplicate keys at parse time. The defensive check inside
    // `TryFrom<RequiresWire>` exists as defence-in-depth in case the wire
    // form ever switches to a Vec-of-pairs shape; it is intentionally
    // unreachable from any valid `vibe.toml`.

    #[test]
    fn git_source_round_trips_through_serialize() {
        let original = pkg_req_from_toml(
            r#"[requires.packages]
"flow:internal" = { git = "https://github.com/me/flow-internal", tag = "v0.1.0", auth = "token-env", token_env = "MY" }
"flow:wal" = "^0.3"
"#,
        );
        // Wrap into a manifest, render, re-parse, verify shape identical.
        let mut m: PackageManifest = toml::from_str(FIXTURE_MINIMAL).unwrap();
        m.requires = original.clone();
        let rendered = toml::to_string_pretty(&m).unwrap();
        let back: PackageManifest = toml::from_str(&rendered).unwrap();
        assert_eq!(back.requires.packages.len(), 1);
        assert_eq!(back.requires.git_packages.len(), 1);
        assert_eq!(back.requires.git_packages[0].name, "internal");
    }
}
