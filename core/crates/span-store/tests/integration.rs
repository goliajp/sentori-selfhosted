//! Integration tests for `sentori-span-store`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_span_store::{
    Cursor, ListTraceFilter, SpanInput, SpanStatus, SpanStore, SpanStoreError,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
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
    for sql in [
        include_str!("../../../migrations/0001_workspace_identity.sql"),
        include_str!("../../../migrations/0005_span_pipeline.sql"),
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

fn root_span(trace_id: Uuid, op: &str, duration_ms: i32, status: SpanStatus) -> SpanInput {
    SpanInput {
        id: Uuid::nil(),
        trace_id,
        parent_span_id: None,
        started_at: OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap(),
        duration_ms,
        op: op.into(),
        name: format!("root {op}"),
        status,
        tags: serde_json::Value::Object(serde_json::Map::new()),
        data: None,
        traceparent: None,
    }
}

fn child_span(
    trace_id: Uuid,
    parent: Uuid,
    op: &str,
    duration_ms: i32,
    status: SpanStatus,
) -> SpanInput {
    SpanInput {
        id: Uuid::nil(),
        trace_id,
        parent_span_id: Some(parent),
        started_at: OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap(),
        duration_ms,
        op: op.into(),
        name: format!("child {op}"),
        status,
        tags: serde_json::json!({ "http.method": "GET" }),
        data: None,
        traceparent: None,
    }
}

// ── ingest_span ──────────────────────────────────────────────

#[tokio::test]
async fn ingest_root_then_child_builds_trace_rollup() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();

    let root = store
        .ingest_span(pid, root_span(trace_id, "navigation", 120, SpanStatus::Ok))
        .await
        .expect("root");
    assert_eq!(root.trace_id, trace_id);
    assert!(root.parent_span_id.is_none());

    let _ = store
        .ingest_span(
            pid,
            child_span(trace_id, root.id, "http.client", 50, SpanStatus::Ok),
        )
        .await
        .expect("child");

    let detail = store.trace_detail(trace_id).await.expect("trace");
    assert_eq!(detail.trace.span_count, 2);
    assert_eq!(detail.trace.root_op.as_deref(), Some("navigation"));
    assert_eq!(detail.trace.duration_ms, 120);
    assert_eq!(detail.trace.status, SpanStatus::Ok);
    assert_eq!(detail.spans.len(), 2);
}

#[tokio::test]
async fn error_child_promotes_trace_status() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();

    let root = store
        .ingest_span(pid, root_span(trace_id, "navigation", 100, SpanStatus::Ok))
        .await
        .unwrap();
    store
        .ingest_span(
            pid,
            child_span(trace_id, root.id, "http.client", 80, SpanStatus::Error),
        )
        .await
        .unwrap();

    let detail = store.trace_detail(trace_id).await.unwrap();
    assert_eq!(detail.trace.status, SpanStatus::Error);
}

#[tokio::test]
async fn cancelled_only_does_not_overwrite_error() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();

    let root = store
        .ingest_span(pid, root_span(trace_id, "nav", 50, SpanStatus::Error))
        .await
        .unwrap();
    store
        .ingest_span(
            pid,
            child_span(trace_id, root.id, "child", 10, SpanStatus::Cancelled),
        )
        .await
        .unwrap();

    let detail = store.trace_detail(trace_id).await.unwrap();
    assert_eq!(detail.trace.status, SpanStatus::Error);
}

#[tokio::test]
async fn child_arrives_before_root_marks_trace_orphan_until_root_lands() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();
    let phantom_root = Uuid::now_v7(); // doesn't exist yet

    // Child first.
    store
        .ingest_span(
            pid,
            child_span(trace_id, phantom_root, "http.client", 30, SpanStatus::Ok),
        )
        .await
        .unwrap();
    let mid = store.trace_detail(trace_id).await.unwrap();
    assert!(mid.trace.is_orphan(), "no root yet");
    assert_eq!(mid.trace.root_op, None);

    // Root lands later.
    store
        .ingest_span(pid, root_span(trace_id, "navigation", 100, SpanStatus::Ok))
        .await
        .unwrap();
    let after = store.trace_detail(trace_id).await.unwrap();
    assert!(!after.trace.is_orphan());
    assert_eq!(after.trace.root_op.as_deref(), Some("navigation"));
    assert_eq!(after.trace.duration_ms, 100);
    assert_eq!(after.trace.span_count, 2);
}

#[tokio::test]
async fn ingest_validates_inputs() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();

    let mut bad = root_span(trace_id, "x", -1, SpanStatus::Ok);
    let err = store.ingest_span(pid, bad.clone()).await.unwrap_err();
    assert!(matches!(err, SpanStoreError::InvalidSpan(_)));

    bad.duration_ms = 0;
    bad.op = String::new();
    let err = store.ingest_span(pid, bad).await.unwrap_err();
    assert!(matches!(err, SpanStoreError::InvalidSpan(_)));
}

#[tokio::test]
async fn ingest_unknown_project_typed_error() {
    let (pool, _ws) = fresh_pool().await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();
    let phantom = ProjectId::new();
    let err = store
        .ingest_span(phantom, root_span(trace_id, "x", 0, SpanStatus::Ok))
        .await
        .unwrap_err();
    assert!(matches!(err, SpanStoreError::ProjectNotFound(_)));
}

#[tokio::test]
async fn spans_for_trace_orders_by_started_at() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);
    let trace_id = Uuid::now_v7();

    let root = store
        .ingest_span(pid, root_span(trace_id, "root", 100, SpanStatus::Ok))
        .await
        .unwrap();
    // Backdate one child to before root.
    let mut early = child_span(trace_id, root.id, "early", 5, SpanStatus::Ok);
    early.started_at = OffsetDateTime::from_unix_timestamp(1_767_225_500).unwrap();
    store.ingest_span(pid, early).await.unwrap();

    let spans = store.spans_for_trace(trace_id).await.unwrap();
    assert_eq!(spans.len(), 2);
    assert!(spans[0].started_at <= spans[1].started_at);
}

#[tokio::test]
async fn trace_detail_not_found() {
    let (pool, _ws) = fresh_pool().await;
    let store = SpanStore::new(pool);
    let err = store.trace_detail(Uuid::now_v7()).await.unwrap_err();
    assert!(matches!(err, SpanStoreError::TraceNotFound(_)));
}

// ── list_traces ──────────────────────────────────────────────

#[tokio::test]
async fn list_traces_filter_and_paginate() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);

    for i in 0..5 {
        let trace_id = Uuid::now_v7();
        store
            .ingest_span(
                pid,
                root_span(trace_id, &format!("op-{i}"), (i + 1) * 10, SpanStatus::Ok),
            )
            .await
            .unwrap();
    }

    // Cursor page 2-by-2.
    let p1 = store
        .list_traces(pid, ListTraceFilter::default(), Cursor::start(2))
        .await
        .unwrap();
    assert_eq!(p1.items.len(), 2);
    assert!(p1.next.is_some());

    let p2 = store
        .list_traces(pid, ListTraceFilter::default(), p1.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p2.items.len(), 2);
    let p3 = store
        .list_traces(pid, ListTraceFilter::default(), p2.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p3.items.len(), 1);
    assert!(p3.next.is_none());

    // Min-duration filter.
    let slow = store
        .list_traces(
            pid,
            ListTraceFilter {
                min_duration_ms: Some(30),
                ..Default::default()
            },
            Cursor::start(50),
        )
        .await
        .unwrap();
    assert_eq!(slow.items.len(), 3); // 30 / 40 / 50ms roots
}

#[tokio::test]
async fn list_traces_status_filter() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = SpanStore::new(pool);

    let ok_trace = Uuid::now_v7();
    store
        .ingest_span(pid, root_span(ok_trace, "a", 10, SpanStatus::Ok))
        .await
        .unwrap();
    let err_trace = Uuid::now_v7();
    store
        .ingest_span(pid, root_span(err_trace, "b", 10, SpanStatus::Error))
        .await
        .unwrap();

    let only_errors = store
        .list_traces(
            pid,
            ListTraceFilter {
                status: Some(SpanStatus::Error),
                ..Default::default()
            },
            Cursor::start(50),
        )
        .await
        .unwrap();
    assert_eq!(only_errors.items.len(), 1);
    assert_eq!(only_errors.items[0].trace_id, err_trace);
}
