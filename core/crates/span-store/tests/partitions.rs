//! Partition-lifecycle integration tests.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_span_store::{SpanInput, SpanStatus, SpanStore};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use time::{OffsetDateTime, macros::datetime};
use tokio::sync::Mutex;
use uuid::Uuid;

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
            .expect("start");
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

async fn seed_project(pool: &PgPool, workspace_id: WorkspaceId) -> ProjectId {
    Identity::new(pool.clone(), workspace_id)
        .projects()
        .create("p", "p", &[0xa5u8; 32])
        .await
        .expect("project")
        .id
}

// ── tests ────────────────────────────────────────────────────

#[tokio::test]
async fn list_existing_returns_bootstrap_plus_default() {
    let (pool, _ws) = fresh_pool().await;
    let store = SpanStore::new(pool);
    let existing = store.partitions().list_existing().await.expect("list");
    // Migration creates 6 monthly bootstraps + default = 7.
    assert!(existing.contains(&"spans_2026_01".to_string()));
    assert!(existing.contains(&"spans_default".to_string()));
    assert!(existing.len() >= 7);
}

#[tokio::test]
async fn ensure_future_creates_missing_months() {
    let (pool, _ws) = fresh_pool().await;
    let store = SpanStore::new(pool);

    // Migration bootstraps 2026-01..2026-06. Pretend "now" is
    // 2026-07 and ask for 4 more months — expect 4 new.
    let now = datetime!(2026-07-15 12:00:00 UTC);
    let created = store
        .partitions()
        .ensure_future(now, 4)
        .await
        .expect("ensure");
    assert_eq!(created, 4, "2026-07..2026-10 missing → 4 created");
    let existing = store.partitions().list_existing().await.unwrap();
    for month in 7..=10 {
        let name = format!("spans_2026_{month:02}");
        assert!(existing.contains(&name), "missing {name}");
    }

    // Idempotent: second call creates 0.
    let again = store
        .partitions()
        .ensure_future(now, 4)
        .await
        .expect("ensure2");
    assert_eq!(again, 0);
}

#[tokio::test]
async fn drop_before_drops_monthly_but_not_default() {
    let (pool, _ws) = fresh_pool().await;
    let store = SpanStore::new(pool);

    // Bootstrap has 2026-01..2026-06. Cutoff at 2026-04-01 means
    // partitions whose upper bound ≤ 2026-04-01 should be dropped
    // — that's 2026-01 (upper 2026-02-01), 2026-02 (upper
    // 2026-03-01), 2026-03 (upper 2026-04-01). 3 dropped.
    let cutoff = datetime!(2026-04-01 00:00:00 UTC);
    let dropped = store.partitions().drop_before(cutoff).await.expect("drop");
    assert_eq!(dropped, 3);

    let after = store.partitions().list_existing().await.unwrap();
    assert!(!after.contains(&"spans_2026_01".to_string()));
    assert!(!after.contains(&"spans_2026_02".to_string()));
    assert!(!after.contains(&"spans_2026_03".to_string()));
    assert!(after.contains(&"spans_2026_04".to_string()));
    assert!(
        after.contains(&"spans_default".to_string()),
        "DEFAULT never dropped"
    );
}

#[tokio::test]
async fn prune_traces_before_deletes_old_rows() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws).await;
    let store = SpanStore::new(pool.clone());
    let trace_id = Uuid::now_v7();

    store
        .ingest_span(
            pid,
            SpanInput {
                id: Uuid::nil(),
                trace_id,
                parent_span_id: None,
                started_at: OffsetDateTime::now_utc(),
                duration_ms: 5,
                op: "test".into(),
                name: "n".into(),
                status: SpanStatus::Ok,
                tags: serde_json::Value::Object(serde_json::Map::new()),
                data: None,
                traceparent: None,
            },
        )
        .await
        .expect("ingest");

    // Force the trace's last_seen back into the past.
    sqlx::query("UPDATE traces SET last_seen = now() - INTERVAL '30 days' WHERE trace_id = $1")
        .bind(trace_id)
        .execute(&pool)
        .await
        .expect("backdate");

    let cutoff = OffsetDateTime::now_utc() - time::Duration::days(14);
    let deleted = store
        .partitions()
        .prune_traces_before(cutoff)
        .await
        .expect("prune");
    assert_eq!(deleted, 1);
}

#[tokio::test]
async fn prune_orphan_traces_respects_grace() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws).await;
    let store = SpanStore::new(pool.clone());
    let trace_id = Uuid::now_v7();
    let phantom_root = Uuid::now_v7();

    // Only a child — root never lands → orphan.
    store
        .ingest_span(
            pid,
            SpanInput {
                id: Uuid::nil(),
                trace_id,
                parent_span_id: Some(phantom_root),
                started_at: OffsetDateTime::now_utc(),
                duration_ms: 5,
                op: "child".into(),
                name: "n".into(),
                status: SpanStatus::Ok,
                tags: serde_json::Value::Object(serde_json::Map::new()),
                data: None,
                traceparent: None,
            },
        )
        .await
        .expect("ingest");

    // Grace 1h, now = ingest+30min → should NOT delete.
    let now_plus_30m = OffsetDateTime::now_utc() + time::Duration::minutes(30);
    let deleted = store
        .partitions()
        .prune_orphan_traces(now_plus_30m, 1)
        .await
        .expect("prune");
    assert_eq!(deleted, 0, "within grace");

    // Now backdate the trace's last_seen by 2h → past grace.
    sqlx::query("UPDATE traces SET last_seen = now() - INTERVAL '2 hours' WHERE trace_id = $1")
        .bind(trace_id)
        .execute(&pool)
        .await
        .unwrap();
    let now = OffsetDateTime::now_utc();
    let deleted = store
        .partitions()
        .prune_orphan_traces(now, 1)
        .await
        .unwrap();
    assert_eq!(deleted, 1, "past grace");
}
