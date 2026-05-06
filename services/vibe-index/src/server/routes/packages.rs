//! Structured query routes for `/v1/packages*`.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::index::search;
use crate::server::error::ApiError;
use crate::server::state::AppState;
use crate::types::{PackageKind, VersionEntry};

#[derive(Debug, Deserialize, Default)]
pub struct ListSearchQuery {
    pub kind: Option<PackageKind>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub command: &'static str,
    pub registry: String,
    pub package_count: u32,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub packages: Vec<PackageRow>,
}

#[derive(Debug, Serialize)]
pub struct PackageRow {
    pub kind: PackageKind,
    pub name: String,
    pub latest_stable: Option<Version>,
    pub versions: Vec<Version>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub command: &'static str,
    pub query: String,
    pub hit_count: usize,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub kind: PackageKind,
    pub name: String,
    pub latest_stable: Option<Version>,
    pub score: u32,
    pub matched_tokens: Vec<String>,
    pub description: Option<String>,
}

pub async fn list_or_search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListSearchQuery>,
) -> Result<axum::response::Response, ApiError> {
    state.stats.note_request();
    let limit = q.limit.unwrap_or(50);
    let offset = q.offset.unwrap_or(0);
    let index = state.index.read().await;

    if let Some(query) = &q.q {
        let hits = search::search(&index, query, q.kind);
        let hits: Vec<SearchHit> = hits
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|h| SearchHit {
                kind: h.kind,
                name: h.name,
                latest_stable: h.latest_stable,
                score: h.score,
                matched_tokens: h.matched_tokens,
                description: h.description,
            })
            .collect();
        let body = SearchResponse {
            command: "search",
            query: query.clone(),
            hit_count: hits.len(),
            hits,
        };
        return Ok(Json(body).into_response());
    }

    // List mode.
    let mut rows: Vec<PackageRow> = index
        .by_pkgref
        .values()
        .filter(|p| q.kind.is_none_or(|k| p.kind == k))
        .map(|p| PackageRow {
            kind: p.kind,
            name: p.name.clone(),
            latest_stable: p.latest_stable.clone(),
            versions: p.versions.iter().map(|v| v.version.clone()).collect(),
            description: p.versions.last().and_then(|v| v.description.clone()),
        })
        .collect();
    rows.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
    let package_count = rows.len() as u32;
    let returned: Vec<PackageRow> = rows.into_iter().skip(offset).take(limit).collect();
    let body = ListResponse {
        command: "list",
        registry: index.registry.clone(),
        package_count,
        returned: returned.len(),
        offset,
        limit,
        packages: returned,
    };
    Ok(Json(body).into_response())
}

pub async fn package_versions(
    State(state): State<Arc<AppState>>,
    Path((kind_str, name)): Path<(String, String)>,
) -> Result<Json<PackageVersionsResponse>, ApiError> {
    state.stats.note_request();
    let kind: PackageKind = kind_str
        .parse()
        .map_err(|_| ApiError::not_found(format!("unknown kind `{kind_str}`")))?;
    let index = state.index.read().await;
    let pkg = index
        .get(kind, &name)
        .ok_or_else(|| ApiError::not_found(format!("`{kind}:{name}` is not in the index")))?;
    Ok(Json(PackageVersionsResponse {
        command: "package",
        kind: pkg.kind,
        name: pkg.name.clone(),
        latest_stable: pkg.latest_stable.clone(),
        versions: pkg.versions.clone(),
    }))
}

#[derive(Serialize)]
pub struct PackageVersionsResponse {
    pub command: &'static str,
    pub kind: PackageKind,
    pub name: String,
    pub latest_stable: Option<Version>,
    pub versions: Vec<VersionEntry>,
}

pub async fn single_version(
    State(state): State<Arc<AppState>>,
    Path((kind_str, name, version_str)): Path<(String, String, String)>,
) -> Result<Json<VersionEntry>, ApiError> {
    state.stats.note_request();
    let kind: PackageKind = kind_str
        .parse()
        .map_err(|_| ApiError::not_found(format!("unknown kind `{kind_str}`")))?;
    let v: Version = version_str
        .parse()
        .map_err(|e| ApiError::bad_request(format!("`{version_str}` is not valid semver: {e}")))?;
    let index = state.index.read().await;
    let pkg = index
        .get(kind, &name)
        .ok_or_else(|| ApiError::not_found(format!("`{kind}:{name}` is not in the index")))?;
    let entry = pkg
        .versions
        .iter()
        .find(|e| e.version == v)
        .ok_or_else(|| {
            ApiError::not_found(format!("`{kind}:{name}@{version_str}` is not in the index"))
        })?
        .clone();
    Ok(Json(entry))
}

use axum::response::IntoResponse;
