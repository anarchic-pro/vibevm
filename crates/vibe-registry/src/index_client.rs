//! Optional HTTP client that lets `GitPackageRegistry` consult an
//! upstream index (PROP-005 §2.10) for cheap version enumeration
//! before falling back to `git ls-remote`. Slice 10.
//!
//! The client is resilient: any failure (4xx, 5xx, connect-fail,
//! malformed JSON) returns an error that the caller treats as a
//! fall-through trigger. Identity (`content_hash`) is verified at
//! fetch time per [PROP-002 §2.1] regardless of how versions were
//! enumerated, so a compromised index can at worst mislead the
//! version selector — never substitute content.

use std::time::Duration;

use semver::Version;
use serde::Deserialize;
use thiserror::Error;
use vibe_core::PackageKind;

const PROBE_TIMEOUT_SECS: u64 = 5;
const FETCH_TIMEOUT_SECS: u64 = 10;

/// Resolved client — `file_base` is the URL prefix that, when joined
/// with `repomd.json` or `by-name/<kind>/<name>.json`, addresses the
/// per-file endpoints. Built via [`IndexClient::probe`] which
/// auto-detects whether the supplied operator URL points at a
/// vibe-index server (`/v1/index/...`) or a static raw-file root.
#[derive(Debug, Clone)]
pub struct IndexClient {
    file_base: String,
}

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("HTTP request to `{url}` failed: {message}")]
    Http { url: String, message: String },
    #[error("index at `{url}` returned status {status}")]
    Status { url: String, status: u16 },
    #[error("index at `{url}` returned malformed JSON: {message}")]
    Malformed { url: String, message: String },
}

impl IndexClient {
    /// Probe the operator-supplied base URL. Returns `Some(client)`
    /// if `<base>/repomd.json` OR `<base>/v1/index/repomd.json`
    /// responds with HTTP 200; `None` otherwise (no index there).
    /// Probe timeout is short (5s) so a misconfigured URL does not
    /// stall every install.
    pub fn probe(base: &str) -> Option<IndexClient> {
        let trimmed = base.trim_end_matches('/');
        let client = match Self::build_client(Duration::from_secs(PROBE_TIMEOUT_SECS)) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(target: "vibe_registry::index_client", "could not build probe client: {e}");
                return None;
            }
        };
        for candidate in [
            format!("{trimmed}/v1/index"),
            trimmed.to_string(),
        ] {
            let url = format!("{candidate}/repomd.json");
            if let Ok(resp) = client.get(&url).send()
                && resp.status().is_success()
            {
                tracing::debug!(target: "vibe_registry::index_client", "probe succeeded at {url}");
                return Some(IndexClient { file_base: candidate });
            }
        }
        tracing::debug!(target: "vibe_registry::index_client", "no index found at base `{base}`");
        None
    }

    /// Construct directly without probing. Used by tests where the
    /// caller has set up the server and knows its layout.
    pub fn at(file_base: impl Into<String>) -> IndexClient {
        IndexClient {
            file_base: file_base.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn file_base(&self) -> &str {
        &self.file_base
    }

    /// Fetch `by-name/<kind>/<name>.json` and return the versions in
    /// ascending semver order. Returns `Ok(None)` for 404 (package
    /// absent in the index — the caller should fall through to
    /// `git ls-remote`); `Ok(Some(versions))` for 200; `Err(...)`
    /// for any other failure.
    pub fn list_versions(
        &self,
        kind: PackageKind,
        name: &str,
    ) -> Result<Option<Vec<Version>>, IndexError> {
        let url = format!("{}/by-name/{}/{}.json", self.file_base, kind.as_str(), name);
        let client = Self::build_client(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .map_err(|e| IndexError::Http {
                url: url.clone(),
                message: e.to_string(),
            })?;
        let resp = client.get(&url).send().map_err(|e| IndexError::Http {
            url: url.clone(),
            message: e.to_string(),
        })?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(IndexError::Status {
                url,
                status: status.as_u16(),
            });
        }
        let body = resp.bytes().map_err(|e| IndexError::Http {
            url: url.clone(),
            message: e.to_string(),
        })?;
        let parsed: PackageEntryView = serde_json::from_slice(&body).map_err(|e| {
            IndexError::Malformed {
                url: url.clone(),
                message: e.to_string(),
            }
        })?;
        let mut versions: Vec<Version> = parsed.versions.into_iter().map(|v| v.version).collect();
        versions.sort();
        Ok(Some(versions))
    }

    fn build_client(timeout: Duration) -> Result<reqwest::blocking::Client, reqwest::Error> {
        reqwest::blocking::Client::builder()
            .user_agent(concat!("vibe-registry/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .build()
    }
}

#[derive(Debug, Deserialize)]
struct PackageEntryView {
    versions: Vec<VersionEntryView>,
}

#[derive(Debug, Deserialize)]
struct VersionEntryView {
    version: Version,
}

/// Resolve `<index_url>` for the named registry from environment.
/// Mirrors the `VIBEVM_INDEX_URL_<REGISTRY>` shape used by
/// `vibe-publish::post_hook`.
pub fn index_url_for(registry: &str) -> Option<String> {
    let suffix = registry_env_suffix(registry);
    if suffix.is_empty() {
        return None;
    }
    std::env::var(format!("VIBEVM_INDEX_URL_{suffix}"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

fn registry_env_suffix(registry: &str) -> String {
    let mut out = String::with_capacity(registry.len());
    for c in registry.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_env_suffix_uppercases() {
        assert_eq!(registry_env_suffix("vibespecs"), "VIBESPECS");
        assert_eq!(
            registry_env_suffix("vibespecs-gitverse"),
            "VIBESPECS_GITVERSE"
        );
    }

    #[test]
    fn at_strips_trailing_slash() {
        let c = IndexClient::at("https://example.com/foo/");
        assert_eq!(c.file_base(), "https://example.com/foo");
    }
}
