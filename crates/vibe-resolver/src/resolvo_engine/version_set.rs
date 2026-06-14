//! `SemverVersionSet` — vibevm's version constraints expressed as a
//! resolvo [`VersionSet`] over `semver::Version` (PROP-017 §2, §3).

use std::fmt;

use resolvo::utils::VersionSet;
use vibe_core::VersionSpec;

/// A set of `semver::Version`s — the unit resolvo interns and asks about
/// through `filter_candidates`. `Any` is the match-all set (the encoding
/// of `VersionSpec::Latest`); `Req` wraps a semver range. A match-nothing
/// variant for `[conflicts]` / `[obsoletes]` arrives with that slice.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum SemverVersionSet {
    Any,
    Req(semver::VersionReq),
}

impl SemverVersionSet {
    /// Lift a vibevm `VersionSpec` into a version set.
    pub(crate) fn from_spec(spec: &VersionSpec) -> Self {
        match spec {
            VersionSpec::Latest => SemverVersionSet::Any,
            VersionSpec::Req(req) => SemverVersionSet::Req(req.clone()),
        }
    }

    /// `true` iff `version` is a member of the set.
    pub(crate) fn contains(&self, version: &semver::Version) -> bool {
        match self {
            SemverVersionSet::Any => true,
            SemverVersionSet::Req(req) => req.matches(version),
        }
    }
}

impl VersionSet for SemverVersionSet {
    type V = semver::Version;
}

impl fmt::Display for SemverVersionSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemverVersionSet::Any => f.write_str("*"),
            SemverVersionSet::Req(req) => write!(f, "{req}"),
        }
    }
}
