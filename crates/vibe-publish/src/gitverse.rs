//! GitVerse public-API [`RepoCreator`] impl.
//!
//! GitVerse exposes a Gitea-compatible REST API at
//! `https://gitverse.ru/api/v1`. The two endpoints we use:
//!
//! - `GET /api/v1/repos/{org}/{repo}` — repo presence check.
//!   - 200 → exists.
//!   - 404 → does not exist (or org missing — distinguished via the
//!     org-presence check on first failure).
//!   - 401 / 403 → auth issue.
//! - `POST /api/v1/orgs/{org}/repos` — create repo in an org.
//!   - 201 → created.
//!   - 409 → already exists (race condition).
//!   - 401 / 403 / 404 mapped per PROP-002 §2.10.
//!
//! Auth uses the Gitea-style `Authorization: token <value>` header.
//! Token loading lives in [`crate::token`].
//!
//! Implementation note: the exact GitVerse response shapes will be
//! verified against the live API on first run. The shapes assumed
//! below are the Gitea-compatible defaults; if GitVerse diverges, this
//! module is the only place that needs adjustment — `RepoCreator`
//! consumers stay host-agnostic.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::token::Token;
use crate::{CreateOpts, PublishError, RepoCreator, RepoInfo};

/// Default base URL for the GitVerse REST API.
pub const DEFAULT_GITVERSE_API_BASE: &str = "https://gitverse.ru/api/v1";

/// Default human-readable host name for error messages.
pub const DEFAULT_GITVERSE_HOST_NAME: &str = "gitverse.ru";

pub struct GitVerseCreator {
    api_base: String,
    host_name: String,
    token: Token,
    client: reqwest::blocking::Client,
}

impl GitVerseCreator {
    pub fn new(token: Token) -> Result<Self, PublishError> {
        Self::with_endpoint(token, DEFAULT_GITVERSE_API_BASE, DEFAULT_GITVERSE_HOST_NAME)
    }

    pub fn with_endpoint(
        token: Token,
        api_base: &str,
        host_name: &str,
    ) -> Result<Self, PublishError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| PublishError::HttpFailed {
                host: host_name.to_string(),
                message: format!("constructing HTTP client: {e}"),
            })?;
        Ok(GitVerseCreator {
            api_base: api_base.trim_end_matches('/').to_string(),
            host_name: host_name.to_string(),
            token,
            client,
        })
    }

    fn auth_header(&self) -> String {
        format!("token {}", self.token.value())
    }
}

#[derive(Debug, Serialize)]
struct CreateRepoBody<'a> {
    name: &'a str,
    private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_branch: Option<&'a str>,
    auto_init: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    website: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    /// Gitea-compatible field carrying the SSH clone URL.
    #[serde(default)]
    ssh_url: Option<String>,
    /// Gitea-compatible field carrying the HTTPS clone URL.
    #[serde(default)]
    clone_url: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

impl RepoCreator for GitVerseCreator {
    fn host_name(&self) -> &str {
        &self.host_name
    }

    fn repo_exists(&self, org: &str, name: &str) -> Result<bool, PublishError> {
        let url = format!("{}/repos/{}/{}", self.api_base, org, name);
        let res = self
            .client
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .map_err(|e| classify_send_error(e, &self.host_name))?;
        let status = res.status();
        if status.is_success() {
            return Ok(true);
        }
        match status.as_u16() {
            404 => Ok(false),
            401 | 403 => Err(PublishError::AuthForbidden {
                host: self.host_name.clone(),
                org: org.to_string(),
            }),
            other => {
                let body = res.text().unwrap_or_default();
                Err(PublishError::UnexpectedResponse {
                    host: self.host_name.clone(),
                    status: other,
                    body,
                })
            }
        }
    }

    fn create_repo(
        &self,
        org: &str,
        name: &str,
        opts: &CreateOpts,
    ) -> Result<RepoInfo, PublishError> {
        let url = format!("{}/orgs/{}/repos", self.api_base, org);
        let body = CreateRepoBody {
            name,
            private: false,
            description: opts.description.as_deref(),
            default_branch: opts.default_branch.as_deref(),
            // We push our own initial commit; never let the host
            // pre-populate, that would conflict with our first push.
            auto_init: false,
            website: opts.homepage.as_deref(),
        };
        let res = self
            .client
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header())
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&body)
            .send()
            .map_err(|e| classify_send_error(e, &self.host_name))?;
        let status = res.status();
        if status.is_success() {
            let parsed: RepoResponse = res.json().map_err(|e| PublishError::HttpFailed {
                host: self.host_name.clone(),
                message: format!("parsing create-repo response: {e}"),
            })?;
            // Prefer SSH for clone URL since contributors typically have
            // SSH keys configured against the host. Fall back to HTTPS if
            // the host omitted SSH.
            let clone_url = parsed
                .ssh_url
                .or(parsed.clone_url)
                .ok_or_else(|| PublishError::UnexpectedResponse {
                    host: self.host_name.clone(),
                    status: status.as_u16(),
                    body: "create-repo response missing both ssh_url and clone_url".to_string(),
                })?;
            let html_url = parsed
                .html_url
                .unwrap_or_else(|| clone_url.trim_end_matches(".git").to_string());
            return Ok(RepoInfo {
                html_url,
                clone_url,
            });
        }
        match status.as_u16() {
            401 | 403 => Err(PublishError::AuthForbidden {
                host: self.host_name.clone(),
                org: org.to_string(),
            }),
            404 => Err(PublishError::OrgNotFound {
                host: self.host_name.clone(),
                org: org.to_string(),
            }),
            409 => {
                // Race condition: someone created the repo between our
                // exists-check and our create. Treat as OK (re-fetch
                // info) — but keep it tight: bubble UnexpectedResponse
                // so the operator notices in the (unlikely) production
                // case. They can re-run; the second invocation's
                // exists-check will pick up the now-existing repo.
                Err(PublishError::UnexpectedResponse {
                    host: self.host_name.clone(),
                    status: 409,
                    body: format!(
                        "repo `{org}/{name}` already exists (created concurrently?). \
                         Re-run `vibe registry publish` — the existing repo will be reused."
                    ),
                })
            }
            other => {
                let body = res.text().unwrap_or_default();
                Err(PublishError::UnexpectedResponse {
                    host: self.host_name.clone(),
                    status: other,
                    body,
                })
            }
        }
    }
}

fn classify_send_error(e: reqwest::Error, host: &str) -> PublishError {
    if e.is_connect() || e.is_timeout() {
        return PublishError::HostUnreachable {
            host: host.to_string(),
        };
    }
    PublishError::HttpFailed {
        host: host.to_string(),
        message: e.to_string(),
    }
}
