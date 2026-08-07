//! Integration tests for `sentori-cert-monitor`.
//!
//! Uses a tiny in-process HTTP server to mock crt.sh — the
//! `with_base_url` builder makes the K10 monitor hit
//! `http://127.0.0.1:<port>` instead of the real CT log.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls,
    clippy::or_fun_call,
    clippy::needless_pass_by_value
)]

use std::net::SocketAddr;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex as StdMutex};

use sentori_cert_monitor::{CertMonitor, CertMonitorError};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;

// ── shared container ──────────────────────────────────────────

struct PgRig {
    _container: ContainerAsync<Postgres>,
    base_url: String,
}

static PG_RIG: OnceLock<Mutex<Option<PgRig>>> = OnceLock::new();

fn rig_cell() -> &'static Mutex<Option<PgRig>> {
    PG_RIG.get_or_init(|| Mutex::new(None))
}

async fn ensure_rig() -> String {
    let mut guard = rig_cell().lock().await;
    if guard.is_none() {
        let container = Postgres::default()
            .with_tag("18")
            .start()
            .await
            .expect("pg");
        let host = container.get_host().await.expect("host");
        let port = container.get_host_port_ipv4(5432).await.expect("port");
        *guard = Some(PgRig {
            _container: container,
            base_url: format!("postgres://postgres:postgres@{host}:{port}"),
        });
    }
    guard.as_ref().expect("rig").base_url.clone()
}

async fn fresh_pool() -> (PgPool, WorkspaceId) {
    let base = ensure_rig().await;
    let admin = PgPool::connect(&format!("{base}/postgres"))
        .await
        .expect("admin");
    let db = format!("t_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE \"{db}\""))
        .execute(&admin)
        .await
        .expect("create");
    drop(admin);
    let pool = PgPool::connect(&format!("{base}/{db}"))
        .await
        .expect("connect");
    for sql in [
        include_str!("../../../migrations/0001_workspace_identity.sql"),
        include_str!("../../../migrations/0009_cert_observations.sql"),
    ] {
        pool.execute(sql).await.expect("migration");
    }
    let workspace_id = sentori_workspace_identity::bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

async fn seed_project(pool: &PgPool, workspace_id: WorkspaceId, slug: &str) -> ProjectId {
    Identity::new(pool.clone(), workspace_id)
        .projects()
        .create(slug, slug, &[0xa5u8; 32])
        .await
        .expect("project")
        .id
}

// ── tiny mock crt.sh server ──────────────────────────────────

/// Programmable response. Tests push `(domain → json body)`
/// pairs; the server returns the matching body or 404.
type Responses = Arc<StdMutex<std::collections::HashMap<String, MockResponse>>>;

#[derive(Clone)]
struct MockResponse {
    status: u16,
    body: String,
}

struct MockServer {
    base_url: String,
    responses: Responses,
}

impl MockServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let base_url = format!("http://{addr}");
        let responses: Responses = Arc::new(StdMutex::new(std::collections::HashMap::new()));
        let responses_clone = responses.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    continue;
                };
                let responses = responses_clone.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = vec![0u8; 8192];
                    let Ok(n) = sock.read(&mut buf).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    // Crude HTTP: GET /?q=…&output=json → first
                    // line first token + query.
                    let first_line = request.lines().next().unwrap_or("");
                    // Extract the `q=` value.
                    let q = first_line
                        .split('?')
                        .nth(1)
                        .and_then(|qs| qs.split('&').find_map(|kv| kv.strip_prefix("q=")))
                        .map(|raw| {
                            // raw is %25.<domain-percent-encoded>.
                            // Strip the %25. prefix and percent-decode.
                            let stripped = raw.strip_prefix("%25.").unwrap_or(raw);
                            urlencoding::decode(stripped)
                                .map_or_else(|_| stripped.to_string(), |c| c.into_owned())
                        })
                        .unwrap_or_default();

                    let resp = {
                        // Scope the StdMutex guard so it doesn't
                        // cross the await boundary (would make the
                        // spawned future !Send).
                        let map = responses.lock().unwrap();
                        map.get(&q).cloned().unwrap_or(MockResponse {
                            status: 404,
                            body: "[]".to_string(),
                        })
                    };
                    let headers = format!(
                        "HTTP/1.1 {status} OK\r\nContent-Length: {len}\r\nContent-Type: application/json\r\n\r\n",
                        status = resp.status,
                        len = resp.body.len()
                    );
                    let _ = sock.write_all(headers.as_bytes()).await;
                    let _ = sock.write_all(resp.body.as_bytes()).await;
                });
            }
        });
        Self {
            base_url,
            responses,
        }
    }

    fn add(&self, domain: &str, body: serde_json::Value) {
        self.responses.lock().unwrap().insert(
            domain.to_string(),
            MockResponse {
                status: 200,
                body: body.to_string(),
            },
        );
    }

    fn add_status(&self, domain: &str, status: u16) {
        self.responses.lock().unwrap().insert(
            domain.to_string(),
            MockResponse {
                status,
                body: String::new(),
            },
        );
    }
}

fn sample_cert(id: i64, common_name: &str, not_after_iso: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "common_name": common_name,
        "name_value": format!("{common_name},*.example.com"),
        "issuer_name": "Test CA",
        "not_before": "2024-01-01T00:00:00",
        "not_after": not_after_iso,
    })
}

// ── add_watch / list_watched / remove_watch ──────────────────

#[tokio::test]
async fn add_watch_round_trip() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let monitor = CertMonitor::new(pool);
    let id = monitor.add_watch(pid, "example.com", None).await.unwrap();
    assert!(!id.is_nil());
    let watched = monitor.list_watched(pid).await.unwrap();
    assert_eq!(watched.len(), 1);
    assert_eq!(watched[0].domain, "example.com");
}

#[tokio::test]
async fn add_watch_idempotent() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let monitor = CertMonitor::new(pool);
    let id1 = monitor.add_watch(pid, "example.com", None).await.unwrap();
    let id2 = monitor.add_watch(pid, "EXAMPLE.com", None).await.unwrap();
    assert_eq!(id1, id2, "case-insensitive dedup");
    assert_eq!(monitor.list_watched(pid).await.unwrap().len(), 1);
}

#[tokio::test]
async fn add_watch_rejects_invalid_domain() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let monitor = CertMonitor::new(pool);
    assert!(matches!(
        monitor.add_watch(pid, "", None).await.unwrap_err(),
        CertMonitorError::InvalidDomain(_)
    ));
    assert!(matches!(
        monitor
            .add_watch(pid, ".example.com", None)
            .await
            .unwrap_err(),
        CertMonitorError::InvalidDomain(_)
    ));
    assert!(matches!(
        monitor.add_watch(pid, "café.com", None).await.unwrap_err(),
        CertMonitorError::InvalidDomain(_)
    ));
}

#[tokio::test]
async fn add_watch_unknown_project_fk() {
    let (pool, _ws) = fresh_pool().await;
    let monitor = CertMonitor::new(pool);
    let err = monitor
        .add_watch(ProjectId::new(), "example.com", None)
        .await
        .unwrap_err();
    assert!(matches!(err, CertMonitorError::ProjectNotFound(_)));
}

#[tokio::test]
async fn remove_watch_idempotent() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let monitor = CertMonitor::new(pool);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    monitor.remove_watch(pid, "example.com").await.unwrap();
    assert!(monitor.list_watched(pid).await.unwrap().is_empty());
    // Removing again is silent.
    monitor.remove_watch(pid, "example.com").await.unwrap();
}

// ── poll_domain ──────────────────────────────────────────────

#[tokio::test]
async fn poll_domain_inserts_new_observations() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add(
        "example.com",
        serde_json::json!([
            sample_cert(1, "www.example.com", "2030-01-01T00:00:00"),
            sample_cert(2, "api.example.com", "2030-06-01T00:00:00"),
        ]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();

    let new = monitor.poll_domain(pid, "example.com").await.unwrap();
    assert_eq!(new.len(), 2);
    assert_eq!(new[0].domain, "example.com");
}

#[tokio::test]
async fn poll_domain_dedup_via_pk() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add(
        "example.com",
        serde_json::json!([sample_cert(1, "www.example.com", "2030-01-01T00:00:00")]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    let first = monitor.poll_domain(pid, "example.com").await.unwrap();
    let second = monitor.poll_domain(pid, "example.com").await.unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 0, "second poll dedups same cert_id");
}

#[tokio::test]
async fn poll_domain_upstream_500() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add_status("example.com", 502);
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    let err = monitor.poll_domain(pid, "example.com").await.unwrap_err();
    assert!(matches!(err, CertMonitorError::UpstreamStatus { .. }));
}

#[tokio::test]
async fn poll_domain_stamps_last_polled_at() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add(
        "example.com",
        serde_json::json!([sample_cert(1, "www.example.com", "2030-01-01T00:00:00")]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    monitor.poll_domain(pid, "example.com").await.unwrap();
    let watched = monitor.list_watched(pid).await.unwrap();
    assert!(watched[0].last_polled_at.is_some());
}

// ── poll_once ───────────────────────────────────────────────

#[tokio::test]
async fn poll_once_fans_out_per_domain() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add(
        "a.com",
        serde_json::json!([sample_cert(10, "www.a.com", "2030-01-01T00:00:00")]),
    );
    server.add(
        "b.com",
        serde_json::json!([sample_cert(20, "www.b.com", "2030-01-01T00:00:00")]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "a.com", None).await.unwrap();
    monitor.add_watch(pid, "b.com", None).await.unwrap();
    let outcome = monitor.poll_once().await.unwrap();
    assert_eq!(outcome.domains_polled, 2);
    assert_eq!(outcome.domains_ok, 2);
    assert_eq!(outcome.new_count(), 2);
    assert_eq!(outcome.domains_failed(), 0);
}

#[tokio::test]
async fn poll_once_skips_failed_domain_continues() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add_status("broken.com", 502);
    server.add(
        "ok.com",
        serde_json::json!([sample_cert(30, "www.ok.com", "2030-01-01T00:00:00")]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "broken.com", None).await.unwrap();
    monitor.add_watch(pid, "ok.com", None).await.unwrap();
    let outcome = monitor.poll_once().await.unwrap();
    assert_eq!(outcome.domains_polled, 2);
    assert_eq!(outcome.domains_ok, 1);
    assert_eq!(outcome.new_count(), 1);
    assert_eq!(outcome.domains_failed(), 1);
    assert_eq!(outcome.per_domain_errors[0].0, "broken.com");
}

// ── list_observations + list_expiring ───────────────────────

#[tokio::test]
async fn list_observations_returns_recent() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    server.add(
        "example.com",
        serde_json::json!([sample_cert(99, "www.example.com", "2030-01-01T00:00:00")]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    monitor.poll_domain(pid, "example.com").await.unwrap();
    let recent = monitor
        .list_observations(pid, time::OffsetDateTime::UNIX_EPOCH)
        .await
        .unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].cert_id, 99);
}

#[tokio::test]
async fn list_expiring_within_window() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let server = MockServer::start().await;
    // "now" sentinel for the test: 2024-12-01.
    let now = time::macros::datetime!(2024-12-01 00:00:00 UTC);
    // 3 certs: in-window (15d), out-of-window (90d), and
    // already-expired (-5d).
    server.add(
        "example.com",
        serde_json::json!([
            sample_cert(1, "soon.example.com", "2024-12-16T00:00:00"),
            sample_cert(2, "later.example.com", "2025-03-01T00:00:00"),
            sample_cert(3, "expired.example.com", "2024-11-26T00:00:00"),
        ]),
    );
    let monitor = CertMonitor::new(pool).with_base_url(&server.base_url);
    monitor.add_watch(pid, "example.com", None).await.unwrap();
    monitor.poll_domain(pid, "example.com").await.unwrap();
    let expiring = monitor
        .list_expiring(pid, now, time::Duration::days(30))
        .await
        .unwrap();
    assert_eq!(expiring.len(), 1);
    assert_eq!(expiring[0].cert_id, 1);
}
