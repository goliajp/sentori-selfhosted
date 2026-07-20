//! Integration tests for `sentori-notifier`.
//!
//! Uses [`MockTransport`] for service-level dispatch/retry
//! tests (avoids binding a real SMTP container) and a tiny
//! in-process HTTP server for [`WebhookTransport`] tests
//! (same pattern as K10 cert-monitor).

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
    clippy::needless_pass_by_value,
    clippy::default_trait_access
)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;

use sentori_notifier::{
    Channel, DeliveryStatus, DispatchOutcome, MockInbox, MockTransport, Notification,
    NotifierError, NotifierService, WebhookTransport,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId, bootstrap_workspace};
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
        include_str!("../../../migrations/0010_delivery_log.sql"),
    ] {
        pool.execute(sql).await.expect("migration");
    }
    let workspace_id = bootstrap_workspace(&pool, "test")
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

fn build_service_with_mock(pool: PgPool) -> (NotifierService, MockInbox) {
    let inbox = MockInbox::new();
    let transport = MockTransport::with_inbox(inbox.clone());
    let mut svc = NotifierService::new(pool);
    svc.register(Arc::new(transport));
    (svc, inbox)
}

// ── tiny mock HTTP server (for webhook tests) ───────────────

struct MockHttp {
    base_url: String,
    received: Arc<std::sync::Mutex<Vec<String>>>,
    fail_status: Arc<std::sync::Mutex<Option<u16>>>,
}

impl MockHttp {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let received: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(Default::default());
        let fail_status: Arc<std::sync::Mutex<Option<u16>>> = Arc::new(Default::default());
        let recv_clone = received.clone();
        let fail_clone = fail_status.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    continue;
                };
                let recv = recv_clone.clone();
                let fail = fail_clone.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = vec![0u8; 8192];
                    let Ok(n) = sock.read(&mut buf).await else {
                        return;
                    };
                    let body = String::from_utf8_lossy(&buf[..n]).to_string();
                    recv.lock().unwrap().push(body);
                    let status = {
                        let f = fail.lock().unwrap();
                        f.unwrap_or(200)
                    };
                    let resp = format!("HTTP/1.1 {status} OK\r\nContent-Length: 0\r\n\r\n");
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        Self {
            base_url: format!("http://{addr}"),
            received,
            fail_status,
        }
    }

    fn fail_with(&self, status: u16) {
        *self.fail_status.lock().unwrap() = Some(status);
    }

    fn received_count(&self) -> usize {
        self.received.lock().unwrap().len()
    }
}

// ── dispatch happy paths ─────────────────────────────────────

#[tokio::test]
async fn dispatch_writes_delivery_log_and_routes_to_transport() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, inbox) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "subj", "body").with_project(pid);
    let outcome = svc.dispatch(&n).await.unwrap();
    assert!(outcome.is_delivered());
    assert_eq!(inbox.len(), 1);

    let log = svc.find(outcome.log_id()).await.unwrap().unwrap();
    assert_eq!(log.status, DeliveryStatus::Delivered);
    assert_eq!(log.channel, Channel::Mock);
    assert_eq!(log.project_id, Some(pid));
    assert_eq!(log.body_preview.as_deref(), Some("body"));
    assert!(log.sent_at.is_some());
    assert_eq!(log.retries, 0);
}

#[tokio::test]
async fn dispatch_failed_transport_records_error() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let inbox = MockInbox::new();
    let transport = MockTransport::with_inbox(inbox.clone()).failing_for("bad");
    let mut svc = NotifierService::new(pool);
    svc.register(Arc::new(transport));

    let n = Notification::new(workspace_id, Channel::Mock, "bad", "subj", "body").with_project(pid);
    let outcome = svc.dispatch(&n).await.unwrap();
    match outcome {
        DispatchOutcome::Failed { log_id, error } => {
            let log = svc.find(log_id).await.unwrap().unwrap();
            assert_eq!(log.status, DeliveryStatus::Failed);
            assert_eq!(log.error.as_deref(), Some(error.as_str()));
            assert!(log.sent_at.is_none());
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[tokio::test]
async fn dispatch_no_transport_for_channel() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = NotifierService::new(pool); // no transports registered
    let n = Notification::new(workspace_id, Channel::Email, "a@b.com", "s", "b");
    let err = svc.dispatch(&n).await.unwrap_err();
    assert!(matches!(err, NotifierError::Transport(_)));
}

#[tokio::test]
async fn dispatch_rejects_empty_subject() {
    let (pool, workspace_id) = fresh_pool().await;
    let (svc, _) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "", "body");
    assert!(matches!(
        svc.dispatch(&n).await.unwrap_err(),
        NotifierError::InvalidInput(_)
    ));
}

#[tokio::test]
async fn dispatch_rejects_empty_recipient() {
    let (pool, workspace_id) = fresh_pool().await;
    let (svc, _) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "", "s", "b");
    assert!(matches!(
        svc.dispatch(&n).await.unwrap_err(),
        NotifierError::InvalidInput(_)
    ));
}

#[tokio::test]
async fn dispatch_unknown_project_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let (svc, _) = build_service_with_mock(pool);
    let phantom = ProjectId::new();
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b").with_project(phantom);
    let err = svc.dispatch(&n).await.unwrap_err();
    assert!(matches!(err, NotifierError::ProjectNotFound(_)));
}

// ── dedup ────────────────────────────────────────────────────

#[tokio::test]
async fn dedup_short_circuits_second_dispatch() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, inbox) = build_service_with_mock(pool);

    let n1 = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b")
        .with_project(pid)
        .with_dedup_key("k1");
    let n2 = Notification::new(workspace_id, Channel::Mock, "ops", "s2", "b2")
        .with_project(pid)
        .with_dedup_key("k1"); // same key

    let r1 = svc.dispatch(&n1).await.unwrap();
    assert!(r1.is_delivered());
    let r2 = svc.dispatch(&n2).await.unwrap();
    assert!(r2.is_deduplicated());
    assert_eq!(inbox.len(), 1, "transport called only once");

    if let DispatchOutcome::Deduplicated { existing } = r2 {
        assert_eq!(existing.id, r1.log_id());
        assert_eq!(existing.subject, "s", "kept original row's content");
    }
}

#[tokio::test]
async fn dedup_distinct_keys_dont_block() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, inbox) = build_service_with_mock(pool);
    for i in 0..3 {
        let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b")
            .with_project(pid)
            .with_dedup_key(format!("k{i}"));
        svc.dispatch(&n).await.unwrap();
    }
    assert_eq!(inbox.len(), 3);
}

#[tokio::test]
async fn no_dedup_key_allows_repeats() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, inbox) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b").with_project(pid);
    svc.dispatch(&n).await.unwrap();
    svc.dispatch(&n).await.unwrap();
    svc.dispatch(&n).await.unwrap();
    assert_eq!(inbox.len(), 3);
}

// ── retry ────────────────────────────────────────────────────

#[tokio::test]
async fn retry_one_recovers_failed_dispatch() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;

    // First service: failing transport. Second: succeeding
    // transport. Tests that the retry path actually
    // re-attempts via the registered transport at retry time.
    let inbox = MockInbox::new();
    let failing = Arc::new(MockTransport::with_inbox(inbox.clone()).failing_for("ops"));
    let mut svc_fail = NotifierService::new(pool.clone());
    svc_fail.register(failing);

    let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b").with_project(pid);
    let r1 = svc_fail.dispatch(&n).await.unwrap();
    let log_id = r1.log_id();
    assert!(matches!(r1, DispatchOutcome::Failed { .. }));

    // Swap to a succeeding transport.
    let inbox2 = MockInbox::new();
    let succeeding = Arc::new(MockTransport::with_inbox(inbox2.clone()));
    let mut svc_ok = NotifierService::new(pool);
    svc_ok.register(succeeding);

    let r2 = svc_ok.retry_one(log_id).await.unwrap();
    assert!(r2.is_delivered());
    assert_eq!(inbox2.len(), 1);

    let log = svc_ok.find(log_id).await.unwrap().unwrap();
    assert_eq!(log.status, DeliveryStatus::Delivered);
    assert_eq!(log.retries, 1);
    assert!(log.error.is_none());
}

#[tokio::test]
async fn retry_one_delivered_is_noop() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, inbox) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b").with_project(pid);
    let r1 = svc.dispatch(&n).await.unwrap();
    let r2 = svc.retry_one(r1.log_id()).await.unwrap();
    assert!(r2.is_delivered());
    // Inbox grew by 1 (initial dispatch); retry of a
    // delivered row is a no-op.
    assert_eq!(inbox.len(), 1);
}

#[tokio::test]
async fn retry_one_missing_log_errors() {
    let (pool, _workspace_id) = fresh_pool().await;
    let (svc, _) = build_service_with_mock(pool);
    let err = svc.retry_one(Uuid::now_v7()).await.unwrap_err();
    assert!(matches!(err, NotifierError::LogNotFound(_)));
}

// ── list_recent / list_pending ──────────────────────────────

#[tokio::test]
async fn list_recent_returns_ordered() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, _) = build_service_with_mock(pool);
    for i in 0..5 {
        let n = Notification::new(workspace_id, Channel::Mock, "ops", format!("s{i}"), "b")
            .with_project(pid);
        svc.dispatch(&n).await.unwrap();
    }
    let recent = svc
        .list_recent(pid, time::OffsetDateTime::UNIX_EPOCH, 10)
        .await
        .unwrap();
    assert_eq!(recent.len(), 5);
    // descending by created_at.
    for i in 0..(recent.len() - 1) {
        assert!(recent[i].created_at >= recent[i + 1].created_at);
    }
}

#[tokio::test]
async fn list_recent_respects_since() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let (svc, _) = build_service_with_mock(pool);
    let n = Notification::new(workspace_id, Channel::Mock, "ops", "s", "b").with_project(pid);
    svc.dispatch(&n).await.unwrap();
    let future = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    let recent = svc.list_recent(pid, future, 10).await.unwrap();
    assert!(recent.is_empty());
}

// ── WebhookTransport (in-process mock HTTP) ─────────────────

#[tokio::test]
async fn webhook_transport_sends_payload() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let server = MockHttp::start().await;
    let mut svc = NotifierService::new(pool);
    svc.register(Arc::new(WebhookTransport::new()));
    let n = Notification::new(
        workspace_id,
        Channel::Webhook,
        format!("{}/hook", server.base_url),
        "alert",
        "{\"k\":1}",
    )
    .with_project(pid);
    let outcome = svc.dispatch(&n).await.unwrap();
    assert!(outcome.is_delivered());
    assert_eq!(server.received_count(), 1);
}

#[tokio::test]
async fn webhook_transport_records_502() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let server = MockHttp::start().await;
    server.fail_with(502);
    let mut svc = NotifierService::new(pool);
    svc.register(Arc::new(WebhookTransport::new()));
    let n = Notification::new(
        workspace_id,
        Channel::Webhook,
        format!("{}/hook", server.base_url),
        "alert",
        "{}",
    )
    .with_project(pid);
    let outcome = svc.dispatch(&n).await.unwrap();
    assert!(!outcome.is_delivered());
    let log = svc.find(outcome.log_id()).await.unwrap().unwrap();
    assert_eq!(log.status, DeliveryStatus::Failed);
    assert!(log.error.as_deref().is_some_and(|s| s.contains("502")));
}
