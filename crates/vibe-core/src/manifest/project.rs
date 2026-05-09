//! `vibe.toml` — the project manifest.
//!
//! Schema: `VIBEVM-SPEC.md` §7.5, [PROP-002 §2.2 / §2.3 / §2.4](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md).
//!
//! The post-M1.1-revision schema treats registries as a priority-ordered
//! array (`[[registry]]`), with optional `[[mirror]]` entries for transparent
//! fallback and `[[override]]` entries that bypass the registry layer for
//! specific pkgrefs. The legacy v1 singleton form (`[registry] url = "..."`)
//! is still accepted on parse and migrates transparently to a single-element
//! array with `name = "default"` and `naming = "kind-name"`.
//!
//! Callers that only need "the first registry" (Phase A code path for v1,
//! where we ship a single-registry runtime) use
//! [`ProjectManifest::primary_registry`]. Multi-registry iteration is in
//! `MultiRegistryResolver` (Phase B / M1.6).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::package::Requires;
use super::{read_toml, write_toml};

/// Top-level `vibe.toml` structure.
///
/// Serializes in modern (array) form. Deserializes accepting both modern
/// and v1 legacy singleton form — see module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub project: ProjectSection,

    /// `[requires]` — packages and capabilities the project directly
    /// declares as dependencies. Spec: `VIBEVM-SPEC.md` §7.5. The
    /// shape mirrors `[requires]` on a package manifest (so the same
    /// `Requires` type covers both); semantics differ — here it is the
    /// user's input list (constraint form like `^0.3`), there it is
    /// the package's own dependency declaration.
    ///
    /// `vibe install <pkgref>` appends to this section; `vibe install`
    /// with no arguments installs every entry. The lockfile mirrors
    /// `requires.packages` into `[meta].root_dependencies` so the
    /// lockfile remains a self-contained snapshot of the solve state.
    #[serde(default, skip_serializing_if = "Requires::is_empty")]
    pub requires: Requires,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<ActiveSection>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmSection>,

    /// Priority-ordered list of registries. The first entry whose registry
    /// has a matching pkgref wins at resolve time; subsequent entries are
    /// fallbacks. Empty = no registry configured (local-only installs via
    /// `--registry <path>`).
    #[serde(default, rename = "registry", skip_serializing_if = "Vec::is_empty")]
    pub registries: Vec<RegistrySection>,

    /// Transparent fallback URLs per registry. `of = "*"` matches any
    /// registry; `of = "<name>"` targets one specifically. See
    /// [PROP-002 §2.3](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#mirror).
    #[serde(default, rename = "mirror", skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorSection>,

    /// Surgical pkgref pins that bypass the registry layer entirely.
    /// Content-hash integrity still enforced. See
    /// [PROP-002 §2.4](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#override).
    #[serde(default, rename = "override", skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<OverrideSection>,

    /// Project-level language preference per PROP-003 §2.7. Default is
    /// English-only canonical content. Set `preferred = "ru"` to make
    /// every install pull Russian-localised content where available.
    #[serde(default, skip_serializing_if = "super::i18n::I18nDecl::is_default")]
    pub i18n: super::i18n::I18nDecl,
}

// ---------------------------------------------------------------------------
// Deserialization: modern array OR v1 singleton form.
// ---------------------------------------------------------------------------

impl<'de> Deserialize<'de> for ProjectManifest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        ProjectManifestWire::deserialize(deserializer).map(Into::into)
    }
}

/// Wire-form used solely for deserialization. Its only job is to accept the
/// legacy singleton `[registry]` shape alongside the modern `[[registry]]`
/// array, and normalize the result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectManifestWire {
    project: ProjectSection,
    #[serde(default)]
    requires: Requires,
    #[serde(default)]
    active: Option<ActiveSection>,
    #[serde(default)]
    llm: Option<LlmSection>,
    /// `Option<RegistryWire>` so TOML `[registry]` (map) and `[[registry]]`
    /// (seq) both land here; absent means no registry configured.
    #[serde(default)]
    registry: Option<RegistryWire>,
    #[serde(default, rename = "mirror")]
    mirrors: Vec<MirrorSection>,
    #[serde(default, rename = "override")]
    overrides: Vec<OverrideSection>,
    #[serde(default)]
    i18n: super::i18n::I18nDecl,
}

/// Two legal TOML shapes for the `registry` key. Untagged enum — serde tries
/// each variant in order and takes the first that parses cleanly.
#[derive(Deserialize)]
#[serde(untagged)]
enum RegistryWire {
    /// `[[registry]]` — modern array form.
    Array(Vec<RegistrySection>),
    /// `[registry] name = "..." url = "..." naming = "..."` — modern singleton
    /// form (unusual, but fully valid — a single-entry array spelled out as
    /// a single table).
    SingleModern(RegistrySection),
    /// `[registry] url = "..." ref = "..."` — v1 legacy form, pre-M1.1-revision.
    /// Migrated to a single-element array with `name = "default"` and
    /// `naming = KindName`.
    SingleLegacy(RegistrySectionLegacy),
}

/// Deserialization-only companion for the v1 legacy `[registry]` form. No
/// `name` field (didn't exist in v1), no `naming` field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrySectionLegacy {
    url: String,
    #[serde(default = "default_ref")]
    r#ref: String,
}

impl From<ProjectManifestWire> for ProjectManifest {
    fn from(w: ProjectManifestWire) -> Self {
        let registries = match w.registry {
            None => Vec::new(),
            Some(RegistryWire::Array(v)) => v,
            Some(RegistryWire::SingleModern(s)) => vec![s],
            Some(RegistryWire::SingleLegacy(l)) => vec![RegistrySection {
                name: DEFAULT_REGISTRY_NAME.to_string(),
                url: l.url,
                r#ref: l.r#ref,
                naming: NamingConvention::KindName,
                auth: AuthKind::None,
                token_env: None,
            }],
        };
        ProjectManifest {
            project: w.project,
            requires: w.requires,
            active: w.active,
            llm: w.llm,
            registries,
            mirrors: w.mirrors,
            overrides: w.overrides,
            i18n: w.i18n,
        }
    }
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSection {
    pub default_provider: String,
    pub default_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

/// A single entry in `[[registry]]` — an organization-root URL plus the
/// naming convention that maps pkgrefs to per-package repos under it,
/// plus the authentication regime to use when fetching from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrySection {
    /// Local alias — used in lockfile `registry` field and to target
    /// `[[mirror]]` / `[[override]]` entries at this registry.
    pub name: String,

    /// Organization-root URL. Generic git URL — any scheme `git` accepts
    /// (`git@host:…`, `ssh://…`, `https://…`, `file://…`).
    pub url: String,

    /// Registry-level ref. Reserved for a future registry-level metadata
    /// branch (capability index, trust policy); not consumed by install
    /// today. Defaults to `main`.
    #[serde(default = "default_ref", skip_serializing_if = "is_default_ref")]
    pub r#ref: String,

    /// Convention mapping a `<kind>:<name>` pkgref to a package repo name
    /// under `url`.
    #[serde(default, skip_serializing_if = "NamingConvention::is_default")]
    pub naming: NamingConvention,

    /// Authentication regime for fetching from this registry. See
    /// [PROP-002 §2.2.1](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#registry-auth).
    /// Default `none` preserves every pre-this-feature `vibe.toml`
    /// behaviour: public read, no credential prompts in scripted runs,
    /// 401 → walk to next registry.
    #[serde(default, skip_serializing_if = "AuthKind::is_default")]
    pub auth: AuthKind,

    /// Override env-var name for `auth = "token-env"`. Default
    /// (when omitted) is derived from the registry host —
    /// `VIBEVM_REGISTRY_TOKEN_<HOST_UPPER>` with dots and hyphens
    /// converted to underscores. Operators set this explicitly when
    /// they want a stable env-var across host migrations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
}

/// Authentication regime per `[[registry]]`. See PROP-002 §2.2.1 for
/// the full semantics matrix; this enum is the schema-level encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthKind {
    /// Public read-only. No credentials sent. In non-TTY / `--unattended`
    /// runs, credential helpers and terminal prompts are silenced so a
    /// 401 / 403 response classifies as `UnknownPackage` and the walk
    /// continues to the next registry. Default.
    #[default]
    #[serde(rename = "none")]
    None,
    /// Token from env-var (default name derived from host, override via
    /// `token_env`). Token is injected into the URL as
    /// `https://x-access-token:<TOKEN>@host/...` for the duration of
    /// each git invocation; never logged, never written to lockfile.
    #[serde(rename = "token-env")]
    TokenEnv,
    /// Opt-in: respect the system git `credential.helper` / `core.askPass`.
    /// On an interactive TTY without `--unattended` a GUI prompt (GCM,
    /// keychain, etc.) may appear; in scripted runs this collapses to
    /// the same behaviour as `None`.
    #[serde(rename = "credential-helper")]
    CredentialHelper,
    /// SSH-based fetch — URL must be ssh-form (`git@host:org`,
    /// `ssh://...`). Authentication delegated to ssh-agent / system
    /// keys; vibe does not touch ssh config.
    #[serde(rename = "ssh")]
    Ssh,
}

impl AuthKind {
    pub fn is_default(&self) -> bool {
        matches!(self, AuthKind::None)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AuthKind::None => "none",
            AuthKind::TokenEnv => "token-env",
            AuthKind::CredentialHelper => "credential-helper",
            AuthKind::Ssh => "ssh",
        }
    }
}

impl RegistrySection {
    /// Resolve the env-var name vibe should consult for the token under
    /// `auth = "token-env"`. Per PROP-002 §2.2.1: explicit `token_env`
    /// wins; otherwise the name is derived from the registry's host —
    /// `VIBEVM_REGISTRY_TOKEN_<HOST_UPPER>` with `.` and `-` mapped to
    /// `_`. Returns `None` when the URL has no parseable host (e.g.
    /// `file://` registries don't carry tokens). Pure function;
    /// caller is free to call regardless of the configured `auth`
    /// regime — the result is meaningful only when `auth ==
    /// AuthKind::TokenEnv`.
    pub fn resolve_token_env_name(&self) -> Option<String> {
        if let Some(explicit) = &self.token_env {
            return Some(explicit.clone());
        }
        let host = registry_host(&self.url)?;
        let mut sanitised = String::with_capacity(host.len());
        for ch in host.chars() {
            match ch {
                '.' | '-' => sanitised.push('_'),
                other if other.is_ascii_alphanumeric() || other == '_' => {
                    sanitised.push(other.to_ascii_uppercase());
                }
                _ => {
                    // Non-ascii or unsupported char in host: bail and
                    // force the operator to provide an explicit
                    // `token_env`. Hosts like punycode IDN should be
                    // handled via the override.
                    return None;
                }
            }
        }
        Some(format!("VIBEVM_REGISTRY_TOKEN_{sanitised}"))
    }
}

/// Best-effort host extraction from a registry URL. Handles both
/// `https://host/path` / `ssh://host/path` (URL-shape) and
/// `git@host:path` (scp-shape). Returns `None` for shapes we can't
/// extract a host from (e.g. `file://`), which is what the
/// token-env-name derivation falls through on.
fn registry_host(url: &str) -> Option<&str> {
    for prefix in ["https://", "http://", "ssh://", "git+ssh://"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return rest.split('/').next()?.split('@').next_back();
        }
    }
    // scp-shape: git@host:path
    if let Some(at_idx) = url.find('@')
        && let Some(colon_idx) = url[at_idx..].find(':')
    {
        let host_start = at_idx + 1;
        let host_end = at_idx + colon_idx;
        if host_end > host_start {
            return Some(&url[host_start..host_end]);
        }
    }
    None
}

/// Convention for mapping a pkgref to a package repository name under a
/// registry's organization URL. The convention is a property of the
/// registry, not a global rule — different registries may ship different
/// conventions without a code change in the CLI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamingConvention {
    /// `flow:wal` → `<org>/flow-wal`. Default; matches the `vibespecs`
    /// organization convention.
    #[default]
    #[serde(rename = "kind-name")]
    KindName,
    /// `flow:wal` → `<org>/wal`. Legal only when names are globally unique
    /// across kinds within a registry.
    #[serde(rename = "name")]
    Name,
    /// `flow:wal` → `<org>/flow/wal`. Requires host support for nested
    /// repository paths (GitLab groups, Gitea orgs).
    #[serde(rename = "kind/name")]
    KindSlashName,
}

impl NamingConvention {
    pub fn is_default(&self) -> bool {
        matches!(self, NamingConvention::KindName)
    }

    /// Compute the repository name for `<kind>:<name>` under this convention.
    pub fn repo_name(&self, kind: crate::package_ref::PackageKind, name: &str) -> String {
        match self {
            NamingConvention::KindName => format!("{}-{name}", kind.as_str()),
            NamingConvention::Name => name.to_string(),
            NamingConvention::KindSlashName => format!("{}/{name}", kind.as_str()),
        }
    }
}

/// A `[[mirror]]` entry: transparent alternative URL for a specific
/// registry (or `*` for any).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MirrorSection {
    /// Target registry name (matches a `[[registry]].name`) or `"*"` for
    /// any registry.
    pub of: String,
    /// Mirror URL. Any git URL.
    pub url: String,
    /// Priority within the target registry's mirror chain — lower = tried
    /// first. Default 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: i32,
}

/// A `[[override]]` entry: direct source pin for a specific pkgref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideSection {
    /// `<kind>:<name>` — the override applies to whatever version the
    /// pinned source / ref resolves to. Version constraints belong on the
    /// source, not here.
    pub pkgref: String,
    /// Source URL (any git URL or `file://`).
    pub source_url: String,
    /// Optional explicit ref — tag, branch, or commit. Defaults to `HEAD`
    /// on the source's default branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    /// Human-readable reason — surfaces in `vibe list --overrides`. Empty
    /// is legal but discouraged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Defaults and helpers
// ---------------------------------------------------------------------------

/// Default registry URL written into every new project's `vibe.toml` by
/// `vibe init` unless the operator overrides it.
///
/// **Org root, not a per-package URL.** Per-package URLs are derived at
/// fetch time via the registry's `naming` convention (default
/// `kind-name` produces `<org>/<kind>-<name>`).
///
/// **Host: GitHub.** The `vibespecs` registry organization moved from
/// GitVerse to GitHub on 2026-04-29 because GitVerse's public REST API
/// does not expose org-scoped repo creation, blocking
/// `vibe registry publish` end-to-end automation. Migration rationale:
/// [PROP-000 §7](../../../spec/common/PROP-000.md#registry) and
/// [PROP-002 §2.10](../../../spec/modules/vibe-registry/PROP-002-decentralized-registry.md#publish).
/// The vibevm tool source itself stays on GitVerse —
/// `git@gitverse.ru:anarchic/vibevm.git` — only the registry org moves.
pub const DEFAULT_REGISTRY_URL: &str = "https://github.com/vibespecs";

/// Default name for the primary registry written by `vibe init` into new
/// projects. Matches the `name` field callers see in `vibe.toml`.
pub const DEFAULT_REGISTRY_NAME: &str = "vibespecs";

/// Default ref on the registry URL — `main`. Applies to both registry-level
/// metadata refs and to the git-backend's `origin/<ref>` fetch target.
pub const DEFAULT_REGISTRY_REF: &str = "main";

/// Secondary `[[registry]]` written by `vibe init` alongside the GitHub
/// primary. Different organisation, different package set: GitHub remains
/// canonical for `vibe registry publish` automation; GitVerse is queried
/// on resolve fall-through (`UnknownPackage` from GitHub) so consumers can
/// install packages that only live on GitVerse without manual setup.
///
/// Note that `vibe registry publish` against this host currently emits a
/// not-implemented stub — the GitVerse public API does not yet expose
/// org-scoped repo creation. The block is provisioned for resolve-time
/// use; publishing remains GitHub-only until the API gains parity.
pub const DEFAULT_REGISTRY_GITVERSE_URL: &str = "https://gitverse.ru/vibespecs";

/// Default name for the secondary GitVerse registry written by `vibe init`.
pub const DEFAULT_REGISTRY_GITVERSE_NAME: &str = "vibespecs-gitverse";

fn default_ref() -> String {
    DEFAULT_REGISTRY_REF.to_string()
}

fn is_default_ref(r: &String) -> bool {
    r == DEFAULT_REGISTRY_REF
}

fn is_zero(x: &i32) -> bool {
    *x == 0
}

impl ProjectManifest {
    pub const FILENAME: &'static str = "vibe.toml";

    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        read_toml(path)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        write_toml(path, self)
    }

    /// The first configured registry, if any. Convenience for the
    /// Phase-A single-registry code path; Phase-B callers iterate
    /// `registries` directly through the `MultiRegistryResolver`.
    pub fn primary_registry(&self) -> Option<&RegistrySection> {
        self.registries.first()
    }

    /// Registry with the given local name, if any.
    pub fn registry_by_name(&self, name: &str) -> Option<&RegistrySection> {
        self.registries.iter().find(|r| r.name == name)
    }

    /// Mirror entries targeting the given registry name, plus any `"*"`
    /// wildcards, sorted by priority ascending.
    pub fn mirrors_for<'a>(&'a self, registry_name: &str) -> Vec<&'a MirrorSection> {
        let mut v: Vec<&'a MirrorSection> = self
            .mirrors
            .iter()
            .filter(|m| m.of == registry_name || m.of == "*")
            .collect();
        v.sort_by_key(|m| m.priority);
        v
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_roundtrip() {
        let raw = r#"
[project]
name = "demo"
version = "0.0.1"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.project.name, "demo");
        assert!(m.active.is_none());
        assert!(m.registries.is_empty());
        assert!(m.mirrors.is_empty());
        assert!(m.overrides.is_empty());
    }

    #[test]
    fn modern_array_form_parses() {
        let raw = r#"
[project]
name = "multi"
version = "0.1.0"

[[registry]]
name = "vibespecs"
url = "git@gitverse.ru:vibespecs"
naming = "kind-name"

[[registry]]
name = "corporate"
url = "git@internal:packages"
ref = "prod"
naming = "name"

[[mirror]]
of = "vibespecs"
url = "https://mirror.internal/vibespecs"
priority = 1

[[override]]
pkgref = "flow:wal"
source_url = "git@mycompany:forks/wal"
ref = "my-fix"
reason = "pending upstream PR"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries.len(), 2);
        assert_eq!(m.registries[0].name, "vibespecs");
        assert_eq!(m.registries[0].url, "git@gitverse.ru:vibespecs");
        assert_eq!(m.registries[0].r#ref, DEFAULT_REGISTRY_REF);
        assert_eq!(m.registries[0].naming, NamingConvention::KindName);
        assert_eq!(m.registries[1].naming, NamingConvention::Name);
        assert_eq!(m.registries[1].r#ref, "prod");
        assert_eq!(m.mirrors.len(), 1);
        assert_eq!(m.mirrors[0].priority, 1);
        assert_eq!(m.overrides.len(), 1);
        assert_eq!(m.overrides[0].pkgref, "flow:wal");
    }

    #[test]
    fn legacy_singleton_migrates_to_array() {
        let raw = r#"
[project]
name = "legacy"
version = "0.1.0"

[registry]
url = "git@gitverse.ru:anarchic/vibespecs.git"
ref = "main"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries.len(), 1);
        let r = &m.registries[0];
        assert_eq!(r.name, DEFAULT_REGISTRY_NAME);
        assert_eq!(r.url, "git@gitverse.ru:anarchic/vibespecs.git");
        assert_eq!(r.r#ref, "main");
        assert_eq!(r.naming, NamingConvention::KindName);
    }

    #[test]
    fn legacy_singleton_serializes_as_array_on_write() {
        let raw = r#"
[project]
name = "legacy"
version = "0.1.0"

[registry]
url = "git@gitverse.ru:anarchic/vibespecs.git"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        let out = toml::to_string_pretty(&m).unwrap();
        // After write, the modern [[registry]] form is used.
        assert!(
            out.contains("[[registry]]"),
            "expected [[registry]] in:\n{out}"
        );
        // And a re-read round-trips.
        let back: ProjectManifest = toml::from_str(&out).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn modern_singleton_table_form_also_works() {
        // Someone writing `[registry] name = "x" url = "y" naming = "kind-name"`
        // (a single modern registry spelled as one table, not as an array) is
        // valid and parses as a single-element array.
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[registry]
name = "local"
url = "file:///tmp/reg"
naming = "name"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries.len(), 1);
        assert_eq!(m.registries[0].name, "local");
        assert_eq!(m.registries[0].naming, NamingConvention::Name);
    }

    #[test]
    fn primary_registry_returns_first() {
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "first"
url = "git@host:a"

[[registry]]
name = "second"
url = "git@host:b"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.primary_registry().unwrap().name, "first");
        assert_eq!(m.registry_by_name("second").unwrap().url, "git@host:b");
        assert!(m.registry_by_name("nope").is_none());
    }

    #[test]
    fn mirrors_for_filters_and_sorts() {
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "vibespecs"
url = "git@host:org"

[[mirror]]
of = "vibespecs"
url = "https://a"
priority = 2

[[mirror]]
of = "vibespecs"
url = "https://b"
priority = 1

[[mirror]]
of = "*"
url = "https://catchall"
priority = 99

[[mirror]]
of = "other"
url = "https://unrelated"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        let ms = m.mirrors_for("vibespecs");
        assert_eq!(ms.len(), 3); // two specific + one wildcard
        assert_eq!(ms[0].url, "https://b"); // priority 1 first
        assert_eq!(ms[1].url, "https://a"); // priority 2 next
        assert_eq!(ms[2].url, "https://catchall"); // wildcard, priority 99 last
    }

    #[test]
    fn naming_convention_serialization() {
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "r1"
url = "git@host:org1"
naming = "kind-name"

[[registry]]
name = "r2"
url = "git@host:org2"
naming = "name"

[[registry]]
name = "r3"
url = "git@host:org3"
naming = "kind/name"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries[0].naming, NamingConvention::KindName);
        assert_eq!(m.registries[1].naming, NamingConvention::Name);
        assert_eq!(m.registries[2].naming, NamingConvention::KindSlashName);
    }

    #[test]
    fn naming_convention_repo_name() {
        use crate::package_ref::PackageKind;
        assert_eq!(
            NamingConvention::KindName.repo_name(PackageKind::Flow, "wal"),
            "flow-wal"
        );
        assert_eq!(
            NamingConvention::Name.repo_name(PackageKind::Stack, "rust-cli"),
            "rust-cli"
        );
        assert_eq!(
            NamingConvention::KindSlashName.repo_name(PackageKind::Feat, "welcome-page"),
            "feat/welcome-page"
        );
    }

    #[test]
    fn full_roundtrip_modern() {
        let raw = r#"
[project]
name = "my-telegram-client"
version = "0.0.1"
authors = ["Oleg <oleg@example.com>"]

[active]
stack = "rust-cli"

[llm]
default_provider = "anthropic"
default_model = "claude-sonnet-4-7"
api_key_env = "ANTHROPIC_API_KEY"

[[registry]]
name = "default"
url = "git@gitverse.ru:vibespecs"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        let rendered = toml::to_string_pretty(&m).unwrap();
        let back: ProjectManifest = toml::from_str(&rendered).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let raw = r#"
[project]
name = "demo"
version = "0.0.1"
mystery_field = true
"#;
        assert!(toml::from_str::<ProjectManifest>(raw).is_err());
    }

    #[test]
    fn rejects_unknown_registry_field() {
        let raw = r#"
[project]
name = "demo"
version = "0.0.1"

[[registry]]
name = "r"
url = "git@host:org"
bogus = 1
"#;
        assert!(toml::from_str::<ProjectManifest>(raw).is_err());
    }

    #[test]
    fn auth_field_defaults_to_none_and_is_skipped_on_serialize() {
        // Back-compat: a `vibe.toml` with no `auth` field on its
        // `[[registry]]` entries — every pre-this-feature project —
        // parses cleanly with auth = None. Round-tripping a
        // None-auth registry must not introduce an `auth = "none"`
        // line: that would create spurious diffs against unchanged
        // manifests.
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "default"
url  = "https://github.com/vibespecs"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries[0].auth, AuthKind::None);
        assert_eq!(m.registries[0].token_env, None);
        let rendered = toml::to_string_pretty(&m).unwrap();
        // Match the actual key — "auth =" — not the bare word, to
        // avoid false-positives with `[project].authors` which always
        // serialises here.
        assert!(
            !rendered.contains("auth ="),
            "default `auth = none` must skip on serialize:\n{rendered}"
        );
        assert!(!rendered.contains("token_env"));
    }

    #[test]
    fn auth_token_env_roundtrips() {
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name      = "internal"
url       = "https://gitlab.company.com/vibespecs"
auth      = "token-env"
token_env = "VIBEVM_REGISTRY_TOKEN_INTERNAL"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.registries[0].auth, AuthKind::TokenEnv);
        assert_eq!(
            m.registries[0].token_env.as_deref(),
            Some("VIBEVM_REGISTRY_TOKEN_INTERNAL")
        );
        let rendered = toml::to_string_pretty(&m).unwrap();
        assert!(rendered.contains("auth = \"token-env\""), "rendered:\n{rendered}");
        assert!(rendered.contains("token_env = \"VIBEVM_REGISTRY_TOKEN_INTERNAL\""));
        let back: ProjectManifest = toml::from_str(&rendered).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn auth_credential_helper_and_ssh_roundtrip() {
        for (raw_value, expected) in [
            ("credential-helper", AuthKind::CredentialHelper),
            ("ssh", AuthKind::Ssh),
        ] {
            let raw = format!(
                r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "r"
url  = "https://example.com/vibespecs"
auth = "{raw_value}"
"#
            );
            let m: ProjectManifest = toml::from_str(&raw).unwrap();
            assert_eq!(m.registries[0].auth, expected);
            let rendered = toml::to_string_pretty(&m).unwrap();
            let back: ProjectManifest = toml::from_str(&rendered).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn auth_rejects_unknown_value() {
        let raw = r#"
[project]
name = "demo"
version = "0.1.0"

[[registry]]
name = "r"
url  = "https://example.com/vibespecs"
auth = "bogus"
"#;
        assert!(toml::from_str::<ProjectManifest>(raw).is_err());
    }

    #[test]
    fn resolve_token_env_name_derives_from_host() {
        let r = RegistrySection {
            name: "r".into(),
            url: "https://gitlab.company.com/vibespecs".into(),
            r#ref: "main".into(),
            naming: NamingConvention::KindName,
            auth: AuthKind::TokenEnv,
            token_env: None,
        };
        assert_eq!(
            r.resolve_token_env_name().as_deref(),
            Some("VIBEVM_REGISTRY_TOKEN_GITLAB_COMPANY_COM")
        );
    }

    #[test]
    fn resolve_token_env_name_honours_explicit_override() {
        let r = RegistrySection {
            name: "r".into(),
            url: "https://gitlab.company.com/vibespecs".into(),
            r#ref: "main".into(),
            naming: NamingConvention::KindName,
            auth: AuthKind::TokenEnv,
            token_env: Some("MY_CUSTOM_TOKEN".to_string()),
        };
        assert_eq!(r.resolve_token_env_name().as_deref(), Some("MY_CUSTOM_TOKEN"));
    }

    #[test]
    fn resolve_token_env_name_handles_scp_form() {
        let r = RegistrySection {
            name: "r".into(),
            url: "git@gitlab.company.com:vibespecs".into(),
            r#ref: "main".into(),
            naming: NamingConvention::KindName,
            auth: AuthKind::Ssh,
            token_env: None,
        };
        assert_eq!(
            r.resolve_token_env_name().as_deref(),
            Some("VIBEVM_REGISTRY_TOKEN_GITLAB_COMPANY_COM")
        );
    }

    #[test]
    fn resolve_token_env_name_returns_none_for_file_url() {
        let r = RegistrySection {
            name: "r".into(),
            url: "file:///tmp/registry".into(),
            r#ref: "main".into(),
            naming: NamingConvention::KindName,
            auth: AuthKind::TokenEnv,
            token_env: None,
        };
        assert!(r.resolve_token_env_name().is_none());
    }

    #[test]
    fn requires_section_roundtrips() {
        let raw = r#"
[project]
name = "with-requires"
version = "0.1.0"

[requires]
packages     = ["flow:wal@^0.3", "stack:rust-cli", "feat:welcome-page@0.2.0"]
capabilities = []
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert_eq!(m.requires.packages.len(), 3);
        assert_eq!(m.requires.packages[0].qualified_name(), "flow:wal");
        assert_eq!(m.requires.packages[1].qualified_name(), "stack:rust-cli");
        assert_eq!(m.requires.packages[2].qualified_name(), "feat:welcome-page");
        // Round-trip through write — the modern map-form comes back
        // (M1.15 wire shape; legacy array form is read-only).
        let rendered = toml::to_string_pretty(&m).unwrap();
        assert!(
            rendered.contains("[requires.packages]"),
            "expected [requires.packages] in:\n{rendered}"
        );
        let back: ProjectManifest = toml::from_str(&rendered).unwrap();
        // Map-form does not preserve input order (BTreeMap is keyed). Compare
        // by serialised idempotency: re-rendering `back` produces the same
        // bytes — write is stable after the first round-trip.
        let rendered2 = toml::to_string_pretty(&back).unwrap();
        assert_eq!(rendered, rendered2);
    }

    #[test]
    fn requires_is_optional_for_legacy_manifests() {
        // A pre-[requires] manifest must still parse cleanly; .requires comes
        // out empty. This is the migration path for projects that were already
        // initialised before [requires] existed in the schema.
        let raw = r#"
[project]
name = "legacy"
version = "0.1.0"
"#;
        let m: ProjectManifest = toml::from_str(raw).unwrap();
        assert!(m.requires.is_empty());
        // Round-trip skips the empty [requires] section.
        let rendered = toml::to_string_pretty(&m).unwrap();
        assert!(
            !rendered.contains("[requires]"),
            "empty [requires] must be skipped on serialize:\n{rendered}"
        );
    }

    #[test]
    fn read_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vibe.toml");
        std::fs::write(
            &path,
            r#"[project]
name = "disk-demo"
version = "0.1.0"
"#,
        )
        .unwrap();
        let m = ProjectManifest::read(&path).unwrap();
        assert_eq!(m.project.name, "disk-demo");
    }
}
