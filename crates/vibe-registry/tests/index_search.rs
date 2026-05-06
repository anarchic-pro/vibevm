//! Integration coverage of `IndexClient::search` against a mock
//! axum server matching the wire shape that
//! `services/vibe-index::server::routes::packages::list_or_search`
//! emits. The mock avoids depending on the live services workspace
//! by hand-rolling the response — keeps this test hermetic to the
//! `vibe-registry` crate while still gating the wire contract.

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use serde::Deserialize;
use tokio::net::TcpListener;

use vibe_core::PackageKind;
use vibe_registry::IndexClient;

#[derive(Default)]
struct CannedSearch {
    /// Return value for a successful search; `None` => respond with
    /// `status_code` instead of `200 + JSON`.
    response: Option<serde_json::Value>,
    status_code: u16,
    /// Most recent query parameters observed by the handler.
    last_q: Option<String>,
    last_kind: Option<String>,
    last_limit: Option<usize>,
}

#[derive(Clone)]
struct MockState {
    files: Arc<Mutex<CannedSearch>>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    kind: Option<String>,
    limit: Option<usize>,
}

async fn search_handler(
    State(state): State<MockState>,
    Query(q): Query<SearchQuery>,
) -> axum::response::Response {
    let mut files = state.files.lock().unwrap();
    files.last_q = q.q;
    files.last_kind = q.kind;
    files.last_limit = q.limit;
    if let Some(body) = files.response.clone() {
        return (StatusCode::OK, axum::Json(body)).into_response();
    }
    let status = StatusCode::from_u16(files.status_code).unwrap_or(StatusCode::OK);
    (status, "").into_response()
}

struct Mock {
    base_url: String,
    files: Arc<Mutex<CannedSearch>>,
    _thread: thread::JoinHandle<()>,
}

fn spawn_mock(canned: CannedSearch) -> Mock {
    let files = Arc::new(Mutex::new(canned));
    let files_for_thread = files.clone();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let state = MockState {
                files: files_for_thread,
            };
            let app = Router::new()
                .route("/v1/packages", get(search_handler))
                .with_state(state);
            tx.send(format!("http://{addr}")).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    Mock {
        base_url: rx.recv().unwrap(),
        files,
        _thread: handle,
    }
}

#[test]
fn search_decodes_response_and_propagates_query_params() {
    let canned = CannedSearch {
        response: Some(serde_json::json!({
            "command": "search",
            "query": "wal log",
            "hit_count": 2,
            "hits": [
                {
                    "kind": "flow",
                    "name": "wal",
                    "latest_stable": "0.1.0",
                    "score": 2,
                    "matched_tokens": ["wal"],
                    "description": "Write-ahead log."
                },
                {
                    "kind": "feat",
                    "name": "audit-log",
                    "latest_stable": "0.2.0",
                    "score": 1,
                    "matched_tokens": ["log"],
                    "description": "Append-only audit trail."
                }
            ]
        })),
        status_code: 200,
        ..CannedSearch::default()
    };
    let mock = spawn_mock(canned);
    let client = IndexClient::at(&mock.base_url);

    let result = client
        .search("wal log", Some(PackageKind::Flow), Some(50))
        .expect("search should succeed");

    assert_eq!(result.query, "wal log");
    assert_eq!(result.hit_count, 2);
    assert_eq!(result.hits.len(), 2);
    assert_eq!(result.hits[0].kind, PackageKind::Flow);
    assert_eq!(result.hits[0].name, "wal");
    assert_eq!(result.hits[0].score, 2);
    assert_eq!(result.hits[0].latest_stable.as_ref().unwrap().to_string(), "0.1.0");
    assert_eq!(result.hits[0].matched_tokens, vec!["wal".to_string()]);
    assert_eq!(result.hits[1].name, "audit-log");

    let observed = mock.files.lock().unwrap();
    assert_eq!(observed.last_q.as_deref(), Some("wal log"));
    assert_eq!(observed.last_kind.as_deref(), Some("flow"));
    assert_eq!(observed.last_limit, Some(50));
}

#[test]
fn search_passes_only_q_when_kind_and_limit_unset() {
    let canned = CannedSearch {
        response: Some(serde_json::json!({
            "command": "search",
            "query": "atomic",
            "hit_count": 0,
            "hits": []
        })),
        status_code: 200,
        ..CannedSearch::default()
    };
    let mock = spawn_mock(canned);
    let client = IndexClient::at(&mock.base_url);

    let result = client.search("atomic", None, None).unwrap();
    assert_eq!(result.hit_count, 0);

    let observed = mock.files.lock().unwrap();
    assert_eq!(observed.last_q.as_deref(), Some("atomic"));
    assert!(observed.last_kind.is_none());
    assert!(observed.last_limit.is_none());
}

#[test]
fn search_surfaces_non_2xx_as_status_error() {
    let canned = CannedSearch {
        response: None,
        status_code: 503,
        ..CannedSearch::default()
    };
    let mock = spawn_mock(canned);
    let client = IndexClient::at(&mock.base_url);

    let err = client.search("wal", None, None).unwrap_err();
    match err {
        vibe_registry::IndexError::Status { status, .. } => {
            assert_eq!(status, 503);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn search_surfaces_404_when_route_absent_on_static_mirror() {
    // A static-file mirror does not mount /v1/packages — it 404s. The
    // CLI consumer treats this as "search not available on this
    // registry", not "package missing".
    let canned = CannedSearch {
        response: None,
        status_code: 404,
        ..CannedSearch::default()
    };
    let mock = spawn_mock(canned);
    let client = IndexClient::at(&mock.base_url);

    let err = client.search("wal", None, None).unwrap_err();
    assert!(matches!(
        err,
        vibe_registry::IndexError::Status { status: 404, .. }
    ));
}
