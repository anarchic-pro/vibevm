//! VVM domain model: version kinds, the canonical version id, and the
//! on-disk inventory (PROP-019 §2.4).

specmark::scope!("spec://vibevm/common/PROP-019#layout");

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specmark::spec;

/// What a version is pinned to (PROP-019 §2.4). The kind namespaces the
/// on-disk layout so a tag `1.2.3` and a branch `1.2.3` never collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[spec(implements = "spec://vibevm/common/PROP-019#layout")]
pub enum Kind {
    Tag,
    Branch,
    Commit,
}

impl Kind {
    /// The lowercase wire/disk token (`tag` / `branch` / `commit`).
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Tag => "tag",
            Kind::Branch => "branch",
            Kind::Commit => "commit",
        }
    }
}

/// The canonical identity of an installed version: `<kind>:<id>` (PROP-019
/// §2.4). Rendered with `:` for humans, split into `<kind>/<id>` on disk —
/// the same segment under both `versions/` and `src/` so the two agree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[spec(implements = "spec://vibevm/common/PROP-019#layout")]
pub struct VersionId {
    pub kind: Kind,
    /// The git-side identifier: a tag name, a branch name, or a commit hash.
    pub id: String,
}

impl VersionId {
    pub fn new(kind: Kind, id: impl Into<String>) -> Self {
        VersionId {
            kind,
            id: id.into(),
        }
    }

    /// The on-disk path segment `<kind>/<id>` (PROP-019 §2.4).
    pub fn path_segment(&self) -> PathBuf {
        PathBuf::from(self.kind.as_str()).join(&self.id)
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.id)
    }
}

/// One installed version's metadata, recorded at install time (PROP-019
/// §2.7) so a moving-branch install stays reproducible after the fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[spec(implements = "spec://vibevm/common/PROP-019#layout")]
pub struct InstallRecord {
    pub kind: Kind,
    pub id: String,
    /// The commit the selector resolved to at install time.
    pub commit: String,
    /// The toolchain that built it (e.g. `rustc 1.93.0`).
    pub toolchain: String,
    /// `debug` or `release`.
    pub profile: String,
    /// RFC3339 install timestamp.
    pub installed_at: String,
}

impl InstallRecord {
    /// The canonical id of this install.
    pub fn version_id(&self) -> VersionId {
        VersionId::new(self.kind, self.id.clone())
    }
}

/// The on-disk inventory at `<root>/vibevm/state.toml` (PROP-019 §2.4).
///
/// The *active* version is deliberately not stored here — it is named by
/// the `VIBEVM_HOME` env var (PROP-019 §2.5), the single source of truth.
/// This file is the inventory of what is installed, nothing more.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[spec(implements = "spec://vibevm/common/PROP-019#layout")]
pub struct State {
    #[serde(default, rename = "install")]
    pub installs: Vec<InstallRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use specmark::verifies;

    #[test]
    #[verifies("spec://vibevm/common/PROP-019#layout", r = 1)]
    fn version_id_renders_and_splits_by_kind() {
        let v = VersionId::new(Kind::Tag, "1.2.3");
        assert_eq!(v.to_string(), "tag:1.2.3");
        assert_eq!(
            v.path_segment(),
            PathBuf::from("tag").join("1.2.3"),
            "on-disk segment is <kind>/<id>"
        );
        // A branch and a tag with the same name never share a path.
        let b = VersionId::new(Kind::Branch, "1.2.3");
        assert_ne!(v.path_segment(), b.path_segment());
    }

    #[test]
    #[verifies("spec://vibevm/common/PROP-019#layout", r = 1)]
    fn state_round_trips_through_toml() {
        let state = State {
            installs: vec![InstallRecord {
                kind: Kind::Branch,
                id: "main".into(),
                commit: "abc1234def".into(),
                toolchain: "rustc 1.93.0".into(),
                profile: "debug".into(),
                installed_at: "2026-06-17T00:00:00Z".into(),
            }],
        };
        let text = toml::to_string(&state).unwrap();
        let back: State = toml::from_str(&text).unwrap();
        assert_eq!(state, back);
        assert_eq!(back.installs[0].version_id().to_string(), "branch:main");
    }

    #[test]
    #[verifies("spec://vibevm/common/PROP-019#layout", r = 1)]
    fn empty_state_is_the_default() {
        let back: State = toml::from_str("").unwrap();
        assert_eq!(back, State::default());
        assert!(back.installs.is_empty());
    }
}
