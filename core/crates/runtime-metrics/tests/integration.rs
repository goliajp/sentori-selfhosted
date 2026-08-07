//! Integration tests for `sentori-runtime-metrics`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc,
    clippy::cast_precision_loss,
    clippy::redundant_clone,
    clippy::cloned_ref_to_slice_refs
)]

use std::sync::OnceLock;

use sentori_runtime_metrics::{
    DropReason, MetricPoint, MetricsStore, RollupTier, RuntimeMetricsError,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use time::{Duration, OffsetDateTime, macros::datetime};
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
        include_str!("../../../migrations/0008_runtime_metrics.sql"),
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

fn ts_within_bootstrap_partition(minute: u8) -> OffsetDateTime {
    // 2026-01-01 partition covers [2026-01-01, 2026-01-02).
    let base = datetime!(2026-01-01 00:00:00 UTC);
    base + Duration::minutes(i64::from(minute))
}

// ── ingest ───────────────────────────────────────────────────

#[tokio::test]
async fn ingest_batch_writes_rows() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let points = vec![
        MetricPoint::new(
            pid,
            "app.startup_ms",
            ts_within_bootstrap_partition(0),
            142.0,
        ),
        MetricPoint::new(
            pid,
            "app.startup_ms",
            ts_within_bootstrap_partition(1),
            160.0,
        ),
    ];
    let written = store.ingest_batch(&points).await.unwrap();
    assert_eq!(written, 2);
}

#[tokio::test]
async fn ingest_dedup_via_pk() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    // Same project + ts + name + tags → same tags_hash → PK conflict.
    let p = MetricPoint::new(pid, "m", ts_within_bootstrap_partition(0), 1.0);
    assert_eq!(store.ingest_batch(&[p.clone()]).await.unwrap(), 1);
    // Re-insert the same shape: ON CONFLICT DO NOTHING → 0 written.
    assert_eq!(store.ingest_batch(&[p]).await.unwrap(), 0);
}

#[tokio::test]
async fn ingest_rejects_invalid_value() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let bad = MetricPoint::new(pid, "m", ts_within_bootstrap_partition(0), f64::NAN);
    let err = store.ingest_batch(&[bad]).await.unwrap_err();
    assert!(matches!(err, RuntimeMetricsError::InvalidInput(_)));
}

#[tokio::test]
async fn ingest_rejects_empty_name() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let bad = MetricPoint::new(pid, "", ts_within_bootstrap_partition(0), 1.0);
    let err = store.ingest_batch(&[bad]).await.unwrap_err();
    assert!(matches!(err, RuntimeMetricsError::InvalidInput(_)));
}

#[tokio::test]
async fn ingest_unknown_project_fk() {
    let (pool, _ws) = fresh_pool().await;
    let store = MetricsStore::new(pool);
    let phantom = ProjectId::new();
    let p = MetricPoint::new(phantom, "m", ts_within_bootstrap_partition(0), 1.0);
    let err = store.ingest_batch(&[p]).await.unwrap_err();
    assert!(matches!(err, RuntimeMetricsError::ProjectNotFound(_)));
}

#[tokio::test]
async fn ingest_empty_batch_returns_zero() {
    let (pool, _ws) = fresh_pool().await;
    let store = MetricsStore::new(pool);
    assert_eq!(store.ingest_batch(&[]).await.unwrap(), 0);
}

// ── rollups ──────────────────────────────────────────────────

#[tokio::test]
async fn roll_raw_to_1m_aggregates() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);

    // 3 points in the same minute, different ts seconds.
    let base = datetime!(2026-01-01 12:00:00 UTC);
    let points = vec![
        MetricPoint::new(pid, "m", base + Duration::seconds(1), 10.0)
            .with_release("v1")
            .with_environment("prod"),
        MetricPoint::new(pid, "m", base + Duration::seconds(20), 20.0)
            .with_release("v1")
            .with_environment("prod"),
        MetricPoint::new(pid, "m", base + Duration::seconds(30), 30.0)
            .with_release("v1")
            .with_environment("prod"),
    ];
    store.ingest_batch(&points).await.unwrap();

    let written = store
        .roll_raw_to_1m(base, base + Duration::seconds(60))
        .await
        .unwrap();
    assert_eq!(written, 1, "single (project, name, release, env, dc) row");

    let rows = store
        .query(
            pid,
            "m",
            RollupTier::Minute,
            base,
            base + Duration::seconds(60),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.count, 3);
    assert!((r.avg - 20.0).abs() < 1e-9);
    assert!((r.sum - 60.0).abs() < 1e-9);
    assert!((r.p50 - 20.0).abs() < 1e-9);
}

#[tokio::test]
async fn roll_raw_to_1m_idempotent() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let base = datetime!(2026-01-01 13:00:00 UTC);
    store
        .ingest_batch(&[MetricPoint::new(pid, "m", base, 5.0)])
        .await
        .unwrap();
    let a = store
        .roll_raw_to_1m(base, base + Duration::seconds(60))
        .await
        .unwrap();
    let b = store
        .roll_raw_to_1m(base, base + Duration::seconds(60))
        .await
        .unwrap();
    assert_eq!(a, b);
    let rows = store
        .query(
            pid,
            "m",
            RollupTier::Minute,
            base,
            base + Duration::seconds(60),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].count, 1, "no double-count under repeated roll");
}

#[tokio::test]
async fn roll_1m_to_1h_cascade() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let hour_start = datetime!(2026-01-02 09:00:00 UTC);
    // 3 minutes within hour, each with a single point.
    for m in 0i64..3 {
        let ts = hour_start + Duration::minutes(m);
        let value = (m + 1) as f64 * 10.0;
        store
            .ingest_batch(&[MetricPoint::new(pid, "m", ts, value)])
            .await
            .unwrap();
        store
            .roll_raw_to_1m(ts, ts + Duration::seconds(60))
            .await
            .unwrap();
    }
    let written = store
        .roll_1m_to_1h(hour_start, hour_start + Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(written, 1);
    let rows = store
        .query(
            pid,
            "m",
            RollupTier::Hour,
            hour_start,
            hour_start + Duration::hours(1),
        )
        .await
        .unwrap();
    assert_eq!(rows[0].count, 3);
    assert!((rows[0].sum - 60.0).abs() < 1e-9); // 10 + 20 + 30
}

#[tokio::test]
async fn roll_1h_to_1d_cascade() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let day_start = datetime!(2026-01-03 00:00:00 UTC);
    // 2 hours within day.
    for h in 0i64..2 {
        let hour = day_start + Duration::hours(h);
        let value = (h + 1) as f64 * 100.0;
        store
            .ingest_batch(&[MetricPoint::new(pid, "m", hour, value)])
            .await
            .unwrap();
        store
            .roll_raw_to_1m(hour, hour + Duration::seconds(60))
            .await
            .unwrap();
        store
            .roll_1m_to_1h(hour, hour + Duration::hours(1))
            .await
            .unwrap();
    }
    let written = store
        .roll_1h_to_1d(day_start, day_start + Duration::days(1))
        .await
        .unwrap();
    assert_eq!(written, 1);
    let rows = store
        .query(
            pid,
            "m",
            RollupTier::Day,
            day_start,
            day_start + Duration::days(1),
        )
        .await
        .unwrap();
    assert_eq!(rows[0].count, 2);
    assert!((rows[0].sum - 300.0).abs() < 1e-9); // 100 + 200
}

// ── query ────────────────────────────────────────────────────

#[tokio::test]
async fn query_returns_empty_when_no_data() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let rows = store
        .query(
            pid,
            "noexist",
            RollupTier::Minute,
            datetime!(2026-01-01 00:00:00 UTC),
            datetime!(2026-01-01 01:00:00 UTC),
        )
        .await
        .unwrap();
    assert!(rows.is_empty());
}

// ── drop counters ────────────────────────────────────────────

#[tokio::test]
async fn record_drop_accumulates() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = MetricsStore::new(pool);
    let day = datetime!(2026-01-01 00:00:00 UTC).date();
    store
        .record_drop(pid, day, DropReason::RateLimit, 5)
        .await
        .unwrap();
    store
        .record_drop(pid, day, DropReason::RateLimit, 7)
        .await
        .unwrap();
    store
        .record_drop(pid, day, DropReason::Malformed, 3)
        .await
        .unwrap();
    let rows = store.list_drops(pid, day, day).await.unwrap();
    assert_eq!(rows.len(), 2);
    let rl = rows
        .iter()
        .find(|r| r.reason == DropReason::RateLimit)
        .unwrap();
    assert_eq!(rl.count, 12);
    let mf = rows
        .iter()
        .find(|r| r.reason == DropReason::Malformed)
        .unwrap();
    assert_eq!(mf.count, 3);
}

// ── partition lifecycle ─────────────────────────────────────

#[tokio::test]
async fn list_existing_returns_bootstrap_plus_default() {
    let (pool, _ws) = fresh_pool().await;
    let store = MetricsStore::new(pool);
    let existing = store.partitions().list_existing().await.unwrap();
    assert!(existing.contains(&"runtime_metrics_raw_2026_01_01".to_string()));
    assert!(existing.contains(&"runtime_metrics_raw_default".to_string()));
    assert!(existing.len() >= 6); // 5 bootstrap + default
}

#[tokio::test]
async fn ensure_future_creates_missing_days() {
    let (pool, _ws) = fresh_pool().await;
    let store = MetricsStore::new(pool);
    // Bootstrap is 2026-01-01 .. 2026-01-05 (5 days).
    // Pretend "now" is 2026-01-06; ask for 2 days ahead.
    let now = datetime!(2026-01-06 12:00:00 UTC);
    let created = store.partitions().ensure_future(now, 2).await.unwrap();
    assert_eq!(created, 3, "missing days 01-06 / 01-07 / 01-08");
    let existing = store.partitions().list_existing().await.unwrap();
    for d in 6..=8 {
        let name = format!("runtime_metrics_raw_2026_01_{d:02}");
        assert!(existing.contains(&name));
    }
    // Idempotent: second call creates 0.
    let again = store.partitions().ensure_future(now, 2).await.unwrap();
    assert_eq!(again, 0);
}

#[tokio::test]
async fn drop_before_drops_old_partitions() {
    let (pool, _ws) = fresh_pool().await;
    let store = MetricsStore::new(pool);
    // Bootstrap has 2026-01-01 .. 2026-01-05. Cutoff at
    // 2026-01-03 00:00 means partitions whose upper bound ≤
    // 2026-01-03 should be dropped: 2026-01-01 (upper 01-02)
    // and 2026-01-02 (upper 01-03). 2 dropped.
    let cutoff = datetime!(2026-01-03 00:00:00 UTC);
    let dropped = store.partitions().drop_before(cutoff).await.unwrap();
    assert_eq!(dropped, 2);
    let after = store.partitions().list_existing().await.unwrap();
    assert!(!after.contains(&"runtime_metrics_raw_2026_01_01".to_string()));
    assert!(!after.contains(&"runtime_metrics_raw_2026_01_02".to_string()));
    assert!(after.contains(&"runtime_metrics_raw_2026_01_03".to_string()));
    assert!(after.contains(&"runtime_metrics_raw_default".to_string()));
}
