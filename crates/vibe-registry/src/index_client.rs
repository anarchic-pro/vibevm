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

/// Resolved client.
///
/// `file_base` is the URL prefix that, when joined with `repomd.json`
/// or `by-name/<kind>/<name>.json`, addresses the per-file endpoints
/// (the static-mirror-friendly read surface from PROP-005 §2.4).
/// `server_base` is the URL prefix for structured live-server routes
/// (`/v1/packages`, `/v1/capabilities/{cap}`, etc. from PROP-005
/// §2.10). Built via [`IndexClient::probe`] which auto-detects
/// whether the supplied operator URL points at a vibe-index server
/// (`<base>/v1/index/...`) or a static raw-file root (`<base>/...`)
/// — `server_base` is always the bare `<base>` regardless, since the
/// structured routes only exist on a live server and never on a
/// static mirror.
#[derive(Debug, Clone)]
pub struct IndexClient {
    file_base: String,
    server_base: String,
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
                return Some(IndexClient {
                    file_base: candidate,
                    server_base: trimmed.to_string(),
                });
            }
        }
        tracing::debug!(target: "vibe_registry::index_client", "no index found at base `{base}`");
        None
    }

    /// Construct directly without probing. Used by tests where the
    /// caller has set up the server and knows its layout. Both
    /// `file_base` and `server_base` are set to the supplied URL —
    /// suitable for the in-tree `tests/` mock servers that mount
    /// raw-file routes (`/repomd.json`, `/by-name/...`) and the
    /// structured server routes (`/v1/packages`) on the same root.
    pub fn at(base: impl Into<String>) -> IndexClient {
        let trimmed = base.into().trim_end_matches('/').to_string();
        IndexClient {
            file_base: trimmed.clone(),
            server_base: trimmed,
        }
    }

    pub fn file_base(&self) -> &str {
        &self.file_base
    }

    pub fn server_base(&self) -> &str {
        &self.server_base
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

    /// Run a full-text search against the live-server route
    /// `<server_base>/v1/packages?q=<query>[&kind=&limit=]` from
    /// PROP-005 §2.10. Returns the structured response on 200; any
    /// non-2xx status surfaces as [`IndexError::Status`] so the
    /// caller can decide whether to fall through to another registry
    /// or surface the error. A 404 here means the URL is a raw-file
    /// mirror (no live server), not "package absent" — there is no
    /// "package absent" case for this endpoint, since search returns
    /// an empty `hits` array on no matches. Identity / integrity
    /// invariants are unaffected: search is metadata-only and never
    /// resolves into a fetch without the consumer running through
    /// the regular `MultiRegistryResolver` path that re-verifies
    /// `content_hash` per [PROP-002 §2.1].
    pub fn search(
        &self,
        query: &str,
        kind: Option<PackageKind>,
        limit: Option<usize>,
    ) -> Result<SearchResults, IndexError> {
        let url = format!("{}/v1/packages", self.server_base);
        let client = Self::build_client(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .map_err(|e| IndexError::Http {
                url: url.clone(),
                message: e.to_string(),
            })?;
        let mut req = client.get(&url).query(&[("q", query)]);
        if let Some(k) = kind {
            req = req.query(&[("kind", k.as_str())]);
        }
        if let Some(lim) = limit {
            req = req.query(&[("limit", lim.to_string())]);
        }
        let resp = req.send().map_err(|e| IndexError::Http {
            url: url.clone(),
            message: e.to_string(),
        })?;
        let status = resp.status();
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
        let parsed: SearchResults = serde_json::from_slice(&body).map_err(|e| {
            IndexError::Malformed {
                url: url.clone(),
                message: e.to_string(),
            }
        })?;
        Ok(parsed)
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

/// Decoded body of the structured search route. Mirrors the wire
/// shape produced by `services/vibe-index::server::routes::packages::SearchResponse`.
/// Extra fields on the wire (today: `command`) are tolerated
/// silently — kept simple so a server-side envelope addition does
/// not force a client bump.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResults {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub hit_count: usize,
    #[serde(default)]
    pub hits: Vec<SearchHit>,
}

/// One package matched by the index's search backend.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub kind: PackageKind,
    pub name: String,
    #[serde(default)]
    pub latest_stable: Option<Version>,
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub matched_tokens: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
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
        assert_eq!(c.server_base(), "https://example.com/foo");
    }

    #[test]
    fn search_results_decode_minimal_envelope() {
        let body = serde_json::json!({
            "command": "search",
            "query": "wal",
            "hit_count": 1,
            "hits": [
                {
                    "kind": "flow",
                    "name": "wal",
                    "latest_stable": "0.1.0",
                    "score": 3,
                    "matched_tokens": ["wal"],
                    "description": "Write-ahead log"
                }
            ]
        });
        let parsed: SearchResults = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.query, "wal");
        assert_eq!(parsed.hit_count, 1);
        assert_eq!(parsed.hits.len(), 1);
        assert_eq!(parsed.hits[0].kind, PackageKind::Flow);
        assert_eq!(parsed.hits[0].name, "wal");
        assert_eq!(parsed.hits[0].score, 3);
        assert_eq!(parsed.hits[0].latest_stable.as_ref().unwrap().to_string(), "0.1.0");
        assert_eq!(parsed.hits[0].matched_tokens, vec!["wal".to_string()]);
        assert_eq!(parsed.hits[0].description.as_deref(), Some("Write-ahead log"));
    }

    #[test]
    fn search_hit_tolerates_missing_optional_fields() {
        let body = serde_json::json!({
            "kind": "feat",
            "name": "atomic-commits"
        });
        let parsed: SearchHit = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.kind, PackageKind::Feat);
        assert_eq!(parsed.name, "atomic-commits");
        assert_eq!(parsed.score, 0);
        assert!(parsed.latest_stable.is_none());
        assert!(parsed.matched_tokens.is_empty());
        assert!(parsed.description.is_none());
    }
}
