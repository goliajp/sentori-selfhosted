//! Integration tests for `sentori-integration-traits`.

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

use sentori_integration_traits::{
    ExternalRef, IntegrationAdapter, IntegrationError, IntegrationService, IssueContext,
    IssueLifecycleEvent, MockAdapter, MockFailMode, RecordedCall, SlackAdapter,
};
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
        include_str!("../../../migrations/0003_event_pipeline.sql"),
        include_str!("../../../migrations/0011_integrations.sql"),
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

async fn seed_issue(pool: &PgPool, project_id: ProjectId) -> Uuid {
    let id = Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    sqlx::query(
        r"
        INSERT INTO issues
            (id, project_id, fingerprint, error_type, message_sample,
             kind, status, first_seen, last_seen, event_count,
             last_environment, last_release)
        VALUES ($1, $2, $3, $4, $5, 'error', 'active', $6, $6, 1,
                'production', 'app@1.0.0')
        ",
    )
    .bind(id)
    .bind(project_id.into_uuid())
    .bind(format!("fp-{}", Uuid::now_v7()))
    .bind("TypeError")
    .bind("boom")
    .bind(now)
    .execute(pool)
    .await
    .expect("seed issue");
    id
}

fn ctx_for(issue_id: Uuid, project_id: ProjectId) -> IssueContext {
    IssueContext {
        issue_id,
        project_id,
        error_type: "TypeError".into(),
        error_message: "x is undefined".into(),
        release: "app@1.0.0".into(),
        environment: "production".into(),
        url: format!("https://sentori.example.com/issues/{issue_id}"),
        event_count: 7,
        crash_site: None,
    }
}

// ── tiny mock HTTP server (for Slack tests) ─────────────────

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
                    let resp = format!("HTTP/1.1 {status} OK\r\nContent-Length: 2\r\n\r\nok");
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

    fn last_body(&self) -> Option<String> {
        self.received.lock().unwrap().last().cloned()
    }
}

// ── config CRUD ──────────────────────────────────────────────

#[tokio::test]
async fn store_config_round_trip() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));

    let id = svc
        .store_config(pid, "mock", serde_json::json!({"k": 1}), None)
        .await
        .unwrap();
    assert!(!id.is_nil());

    let cfg = svc.get_config(pid, "mock").await.unwrap().unwrap();
    assert_eq!(cfg.kind, "mock");
    assert!(cfg.active);
    assert_eq!(cfg.config["k"], 1);
}

#[tokio::test]
async fn store_config_idempotent_on_project_kind() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    let id1 = svc
        .store_config(pid, "mock", serde_json::json!({"v": 1}), None)
        .await
        .unwrap();
    let id2 = svc
        .store_config(pid, "mock", serde_json::json!({"v": 2}), None)
        .await
        .unwrap();
    assert_eq!(id1, id2);
    let cfg = svc.get_config(pid, "mock").await.unwrap().unwrap();
    assert_eq!(cfg.config["v"], 2, "second store overwrites blob");
}

#[tokio::test]
async fn store_config_unknown_adapter_errors() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let svc = IntegrationService::new(pool); // no adapters
    let err = svc
        .store_config(pid, "ghost", serde_json::json!({}), None)
        .await
        .unwrap_err();
    assert!(matches!(err, IntegrationError::NoAdapter(_)));
}

#[tokio::test]
async fn store_config_unknown_project_fk() {
    let (pool, _ws) = fresh_pool().await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    let err = svc
        .store_config(ProjectId::new(), "mock", serde_json::json!({}), None)
        .await
        .unwrap_err();
    assert!(matches!(err, IntegrationError::ProjectNotFound(_)));
}

#[tokio::test]
async fn deactivate_then_reactivate() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    svc.deactivate(pid, "mock").await.unwrap();
    let cfg = svc.get_config(pid, "mock").await.unwrap().unwrap();
    assert!(!cfg.active);

    // Re-storing re-activates.
    svc.store_config(pid, "mock", serde_json::json!({"new": true}), None)
        .await
        .unwrap();
    let cfg = svc.get_config(pid, "mock").await.unwrap().unwrap();
    assert!(cfg.active);
}

#[tokio::test]
async fn remove_config_drops_row() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    svc.remove_config(pid, "mock").await.unwrap();
    assert!(svc.get_config(pid, "mock").await.unwrap().is_none());
}

#[tokio::test]
async fn list_for_project_ordering() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new().with_kind("a")));
    svc.register(Arc::new(MockAdapter::new().with_kind("b")));
    svc.store_config(pid, "a", serde_json::json!({}), None)
        .await
        .unwrap();
    svc.store_config(pid, "b", serde_json::json!({}), None)
        .await
        .unwrap();
    let list = svc.list_for_project(pid).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].kind, "a");
    assert_eq!(list[1].kind, "b");
}

// ── dispatch: Created ────────────────────────────────────────

#[tokio::test]
async fn dispatch_created_records_link() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let history = sentori_integration_traits::MockHistory::new();
    let adapter = MockAdapter::new().with_history(history.clone());
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(adapter));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();

    let ctx = ctx_for(issue_id, pid);
    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(outcome.successes.len(), 1);
    assert!(outcome.is_clean());

    let link = svc.get_link(issue_id, "mock").await.unwrap().unwrap();
    assert_eq!(link.external_id, "mock-id");

    // Adapter recorded the CreateIssue call.
    let history = history.snapshot();
    assert!(matches!(
        history[0],
        RecordedCall::CreateIssue { issue_id: id } if id == issue_id
    ));
}

#[tokio::test]
async fn dispatch_created_skips_already_linked() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();

    let ctx = ctx_for(issue_id, pid);
    let _ = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    let again = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(again.skipped.len(), 1);
    assert!(again.successes.is_empty());
    assert!(again.skipped[0].1.contains("already linked"));
}

#[tokio::test]
async fn dispatch_created_failure_captured_not_aborts() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(
        MockAdapter::new()
            .with_kind("a")
            .failing(MockFailMode::Create),
    ));
    svc.register(Arc::new(MockAdapter::new().with_kind("b"))); // ok
    svc.store_config(pid, "a", serde_json::json!({}), None)
        .await
        .unwrap();
    svc.store_config(pid, "b", serde_json::json!({}), None)
        .await
        .unwrap();

    let ctx = ctx_for(issue_id, pid);
    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].0, "a");
    assert_eq!(outcome.successes.len(), 1);
    assert_eq!(outcome.successes[0].0, "b");
    // Link only persisted for the succeeding adapter.
    assert!(svc.get_link(issue_id, "a").await.unwrap().is_none());
    assert!(svc.get_link(issue_id, "b").await.unwrap().is_some());
}

// ── dispatch: Regressed / Resolved ──────────────────────────

#[tokio::test]
async fn dispatch_resolved_calls_update_status() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let history = sentori_integration_traits::MockHistory::new();
    let adapter = MockAdapter::new().with_history(history.clone());
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(adapter));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    let ctx = ctx_for(issue_id, pid);
    let _ = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();

    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Resolved)
        .await
        .unwrap();
    assert_eq!(outcome.successes.len(), 1);

    let history = history.snapshot();
    let found_resolved = history.iter().any(|c| {
        matches!(c, RecordedCall::UpdateStatus { event, .. }
            if *event == IssueLifecycleEvent::Resolved)
    });
    assert!(found_resolved);
}

#[tokio::test]
async fn dispatch_resolved_skips_unlinked() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();

    // No prior Created → no link → Resolved skips.
    let ctx = ctx_for(issue_id, pid);
    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Resolved)
        .await
        .unwrap();
    assert_eq!(outcome.skipped.len(), 1);
    assert!(outcome.skipped[0].1.contains("not linked"));
}

#[tokio::test]
async fn dispatch_resolved_failure_captured() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    // First adapter succeeds Created, fails Update.
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new().failing(MockFailMode::Update)));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    let ctx = ctx_for(issue_id, pid);
    let created = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(created.successes.len(), 1);
    let resolved = svc
        .dispatch(&ctx, IssueLifecycleEvent::Resolved)
        .await
        .unwrap();
    assert_eq!(resolved.failures.len(), 1);
}

// ── inactive adapter skipped ────────────────────────────────

#[tokio::test]
async fn dispatch_skips_inactive_integration() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    svc.deactivate(pid, "mock").await.unwrap();

    let ctx = ctx_for(issue_id, pid);
    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    // active = false → not in the iteration → total = 0.
    assert_eq!(outcome.total(), 0);
}

// ── reverse lookup ──────────────────────────────────────────

#[tokio::test]
async fn find_link_by_external_round_trip() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(MockAdapter::new()));
    svc.store_config(pid, "mock", serde_json::json!({}), None)
        .await
        .unwrap();
    let ctx = ctx_for(issue_id, pid);
    let _ = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();

    let link = svc
        .find_link_by_external("mock", "mock-id")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(link.issue_id, issue_id);

    let missing = svc.find_link_by_external("mock", "nope").await.unwrap();
    assert!(missing.is_none());
}

// ── Slack reference adapter end-to-end ──────────────────────

#[tokio::test]
async fn slack_create_and_update_through_mock_http() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let server = MockHttp::start().await;

    let adapter = SlackAdapter::new().with_base_url(&server.base_url);
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(adapter));
    // Manual config — webhook URL stored as if it came from
    // the dashboard form.
    svc.store_config(
        pid,
        "slack",
        serde_json::json!({"webhookUrl": "https://hooks.slack.com/services/T/B/abcdef"}),
        None,
    )
    .await
    .unwrap();
    let ctx = ctx_for(issue_id, pid);
    let created = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(created.successes.len(), 1);
    assert_eq!(server.received_count(), 1);
    let body = server.last_body().unwrap();
    assert!(body.contains("TypeError"));

    let resolved = svc
        .dispatch(&ctx, IssueLifecycleEvent::Resolved)
        .await
        .unwrap();
    assert_eq!(resolved.successes.len(), 1);
    assert_eq!(server.received_count(), 2);
    let body = server.last_body().unwrap();
    assert!(body.contains("Resolved"));
    assert!(body.contains(":white_check_mark:"));
}

#[tokio::test]
async fn slack_5xx_recorded_as_failure() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let issue_id = seed_issue(&pool, pid).await;
    let server = MockHttp::start().await;
    server.fail_with(500);

    let adapter = SlackAdapter::new().with_base_url(&server.base_url);
    let mut svc = IntegrationService::new(pool);
    svc.register(Arc::new(adapter));
    svc.store_config(
        pid,
        "slack",
        serde_json::json!({"webhookUrl": "https://hooks.slack.com/services/T/B/xxx"}),
        None,
    )
    .await
    .unwrap();

    let ctx = ctx_for(issue_id, pid);
    let outcome = svc
        .dispatch(&ctx, IssueLifecycleEvent::Created)
        .await
        .unwrap();
    assert_eq!(outcome.failures.len(), 1);
    assert!(outcome.failures[0].1.contains("non-2xx 500"));
    // No link persisted on Create failure.
    assert!(svc.get_link(issue_id, "slack").await.unwrap().is_none());
}

// ── manual mode validation ──────────────────────────────────

#[tokio::test]
async fn slack_manual_config_validation() {
    let a = SlackAdapter::new();
    let bad = serde_json::json!({"webhookUrl": "ftp://nope"});
    let err = a.accept_manual_config(bad).await.unwrap_err();
    assert!(matches!(err, IntegrationError::InvalidInput(_)));
}

// ── ExternalRef shape ───────────────────────────────────────

#[tokio::test]
async fn record_link_unknown_issue_fk() {
    let (pool, _ws) = fresh_pool().await;
    let svc = IntegrationService::new(pool);
    let ext = ExternalRef {
        external_id: "x".into(),
        external_url: "u".into(),
    };
    let err = svc
        .record_link(Uuid::now_v7(), "k", &ext)
        .await
        .unwrap_err();
    assert!(matches!(err, IntegrationError::IssueNotFound(_)));
}
