//! Integration tests for `sentori-event-pipeline` against a real
//! postgres 18 via testcontainers.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_event_pipeline::{
    Event, EventKind, FrameSite, IngestError, IngestOptions, IngestService, IssueStatus,
    MessageLevel, Platform,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId, bootstrap_workspace};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use time::OffsetDateTime;
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
            .expect("start postgres");
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
    let db_name = format!("t_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create db");
    drop(admin);
    let pool = PgPool::connect(&format!("{base}/{db_name}"))
        .await
        .expect("connect");
    pool.execute(include_str!(
        "../../../migrations/0001_workspace_identity.sql"
    ))
    .await
    .expect("0001");
    pool.execute(include_str!("../../../migrations/0003_event_pipeline.sql"))
        .await
        .expect("0003");
    let workspace_id = bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

// ── seed helpers ──────────────────────────────────────────────

async fn seed_project(pool: &PgPool, workspace_id: WorkspaceId, slug: &str) -> ProjectId {
    let identity = Identity::new(pool.clone(), workspace_id);
    let salt: [u8; 32] = [0xa5; 32];
    identity
        .projects()
        .create(slug, slug, &salt)
        .await
        .expect("create project")
        .id
}

fn ts(unix: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(unix).expect("ts")
}

fn exception_event(release: &str, error_type: &str, msg: &str) -> Event {
    Event::exception(
        Uuid::now_v7(),
        ts(1_767_225_600),
        Platform::Ios,
        release,
        "production",
        error_type,
        msg,
    )
    .with_frame(FrameSite {
        function: Some("renderHeader".into()),
        file: "app/screens/Home.tsx".into(),
    })
}

fn message_event(release: &str, body: &str) -> Event {
    Event::message(
        Uuid::now_v7(),
        ts(1_767_225_600),
        Platform::Javascript,
        release,
        "production",
        MessageLevel::Warning,
        body,
    )
}

// ── ingest (write-through) ────────────────────────────────────

#[tokio::test]
async fn ingest_creates_issue_then_bumps_count() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");

    let out_a = svc
        .ingest(pid, exception_event("app@1.0.0", "TypeError", "boom"))
        .await
        .expect("first");
    assert!(out_a.is_new_issue);
    assert!(!out_a.regressed);

    let out_b = svc
        .ingest(pid, exception_event("app@1.0.0", "TypeError", "boom"))
        .await
        .expect("second");
    assert!(!out_b.is_new_issue);
    assert_eq!(out_a.issue_id, out_b.issue_id);

    let issue = svc
        .find_issue(out_a.issue_id)
        .await
        .expect("ok")
        .expect("present");
    assert_eq!(issue.event_count, 2);
    assert_eq!(issue.status, IssueStatus::Active);
    assert_eq!(svc.count_events_for_issue(out_a.issue_id).await.unwrap(), 2);
}

#[tokio::test]
async fn ingest_different_release_creates_separate_issues() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let a = svc
        .ingest(pid, exception_event("app@1.0.0", "TypeError", "boom"))
        .await
        .expect("a");
    let b = svc
        .ingest(pid, exception_event("app@2.0.0", "TypeError", "boom"))
        .await
        .expect("b");
    assert_ne!(a.issue_id, b.issue_id);
    assert!(a.is_new_issue && b.is_new_issue);
}

#[tokio::test]
async fn ingest_message_kind_groups_by_normalised_body() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let a = svc
        .ingest(pid, message_event("app@1.0.0", "User 12345 timed out"))
        .await
        .expect("a");
    let b = svc
        .ingest(pid, message_event("app@1.0.0", "User 67890 timed out"))
        .await
        .expect("b");
    assert_eq!(a.issue_id, b.issue_id, "dynamic id should not fragment");
}

#[tokio::test]
async fn regression_path_atomic() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let first = svc
        .ingest(pid, exception_event("app@1.0.0", "TypeError", "boom"))
        .await
        .expect("first");

    // Operator resolves the issue.
    svc.set_issue_status(
        first.issue_id,
        IssueStatus::Resolved,
        Some(OffsetDateTime::now_utc()),
    )
    .await
    .expect("resolve");

    // Next event of the SAME release/error/message → same
    // fingerprint → atomic flip to regressed. Per S3 design
    // (per-release isolation), a 1.0.1 event would create a
    // distinct issue rather than flipping; the "regression
    // within the same build" scenario is the one the UPSERT
    // path is designed for. Cross-release regression discovery
    // is a dashboard query, not a status flip.
    let regression = svc
        .ingest(pid, exception_event("app@1.0.0", "TypeError", "boom"))
        .await
        .expect("regression");
    assert!(!regression.is_new_issue);
    assert!(regression.regressed, "should report regression");

    let issue = svc
        .find_issue(first.issue_id)
        .await
        .expect("ok")
        .expect("present");
    assert_eq!(issue.status, IssueStatus::Regressed);
    assert_eq!(issue.regressed_in_release.as_deref(), Some("app@1.0.0"));
}

#[tokio::test]
async fn fingerprint_override_short_circuits_algorithmic() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let override_fp = "payment.card-decline";

    let ev1 = exception_event("app@1.0.0", "TypeError", "card-1234 declined")
        .with_fingerprint_override(override_fp);
    let ev2 = exception_event("app@2.0.0", "DifferentError", "card-9999 declined")
        .with_fingerprint_override(override_fp);

    let a = svc.ingest(pid, ev1).await.expect("a");
    let b = svc.ingest(pid, ev2).await.expect("b");
    assert_eq!(a.issue_id, b.issue_id);

    let issue = svc
        .find_issue_by_fingerprint(pid, override_fp)
        .await
        .expect("ok")
        .expect("present");
    assert_eq!(issue.event_count, 2);
}

#[tokio::test]
async fn ingest_invalid_event_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    // Missing release.
    let bad = Event::exception(
        Uuid::now_v7(),
        ts(0),
        Platform::Ios,
        "",
        "production",
        "T",
        "m",
    );
    let err = svc.ingest(pid, bad).await.unwrap_err();
    assert!(matches!(err, IngestError::InvalidEvent(_)));

    // Message kind missing level.
    let bad_msg = Event {
        id: Uuid::now_v7(),
        timestamp: ts(0),
        kind: EventKind::Message,
        platform: Platform::Ios,
        release: "app@1.0.0".into(),
        environment: "production".into(),
        error_type: None,
        message: Some("body".into()),
        level: None,
        frame: None,
        fingerprint_override: None,
        payload: serde_json::Value::Null,
    };
    let err = svc.ingest(pid, bad_msg).await.unwrap_err();
    assert!(matches!(err, IngestError::InvalidEvent(_)));
}

#[tokio::test]
async fn ingest_unknown_project_surfaces_typed_error() {
    let (pool, _workspace_id) = fresh_pool().await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let phantom = ProjectId::new();
    let err = svc
        .ingest(phantom, exception_event("app@1.0.0", "T", "m"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, IngestError::ProjectNotFound(_)),
        "got: {err:?}",
    );
}

#[tokio::test]
async fn event_payload_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let payload = serde_json::json!({
        "device": { "model": "iPhone15,2", "osVersion": "17.4" },
        "tags": { "feature_flag": "new_search" },
        "breadcrumbs": [{ "type": "nav", "data": { "to": "/home" } }]
    });
    let ev = exception_event("app@1.0.0", "TypeError", "boom").with_payload(payload.clone());
    let event_id = ev.id;
    svc.ingest(pid, ev).await.expect("ingest");
    let stored = svc
        .find_event(event_id)
        .await
        .expect("ok")
        .expect("present");
    assert_eq!(stored.payload, payload);
}

// ── try_enqueue + flush ───────────────────────────────────────

#[tokio::test]
async fn flush_persists_every_queued_event() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions { ring_capacity: 16 }).expect("svc");

    for i in 0..5 {
        svc.try_enqueue(pid, exception_event("app@1.0.0", "T", &format!("msg-{i}")))
            .expect("enqueue");
    }
    assert_eq!(svc.ringbuffer().len(), 5);

    let persisted = svc.flush().await.expect("flush");
    assert_eq!(persisted, 5);
    assert!(svc.ringbuffer().is_empty());

    // 5 messages with distinct body → 5 distinct issues per
    // the fingerprint algorithm.
    let issue = svc
        .find_issue_by_fingerprint(
            pid,
            &IngestService::fingerprint(&exception_event("app@1.0.0", "T", "msg-0")),
        )
        .await
        .expect("ok")
        .expect("present");
    assert_eq!(issue.event_count, 1);
}

#[tokio::test]
async fn try_enqueue_rejects_invalid() {
    let (pool, _workspace_id) = fresh_pool().await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");
    let pid = ProjectId::new();
    let bad = Event::exception(
        Uuid::now_v7(),
        ts(0),
        Platform::Ios,
        "", // empty release
        "production",
        "T",
        "m",
    );
    let err = svc.try_enqueue(pid, bad).unwrap_err();
    assert!(matches!(err, IngestError::InvalidEvent(_)));
    assert_eq!(svc.ringbuffer().len(), 0);
}

#[tokio::test]
async fn flush_drops_per_event_failures_and_keeps_going() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let phantom = ProjectId::new();
    let svc = IngestService::new(pool, IngestOptions { ring_capacity: 16 }).expect("svc");

    // Mix: one for unknown project (will fail FK), two for the real one.
    svc.try_enqueue(phantom, exception_event("app@1.0.0", "T", "phantom"))
        .expect("enqueue phantom");
    svc.try_enqueue(pid, exception_event("app@1.0.0", "T", "real-1"))
        .expect("enqueue real-1");
    svc.try_enqueue(pid, exception_event("app@1.0.0", "T", "real-2"))
        .expect("enqueue real-2");
    assert_eq!(svc.ringbuffer().len(), 3);

    let persisted = svc.flush().await.expect("flush");
    assert_eq!(persisted, 2, "phantom should drop, real-1 + real-2 persist");
    assert!(svc.ringbuffer().is_empty());
}

#[tokio::test]
async fn new_rejects_zero_capacity() {
    let (pool, _workspace_id) = fresh_pool().await;
    let err = IngestService::new(pool, IngestOptions { ring_capacity: 0 }).unwrap_err();
    assert!(matches!(err, IngestError::InvalidEvent(_)));
}

#[tokio::test]
async fn set_issue_status_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool, IngestOptions::default()).expect("svc");

    let outcome = svc
        .ingest(pid, exception_event("app@1.0.0", "T", "m"))
        .await
        .expect("seed");

    svc.set_issue_status(outcome.issue_id, IssueStatus::Ignored, None)
        .await
        .expect("ignore");
    let issue = svc.find_issue(outcome.issue_id).await.unwrap().unwrap();
    assert_eq!(issue.status, IssueStatus::Ignored);
}
