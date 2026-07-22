//! Integration tests for `sentori-billing`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_billing::{
    BillingError, BillingService, CounterKind, Decision, Plan, PlanStatus, period_key,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId, bootstrap_workspace};
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
        include_str!("../../../migrations/0015_billing.sql"),
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

const fn now() -> OffsetDateTime {
    datetime!(2026-06-15 10:00:00 UTC)
}

// ── billing CRUD ────────────────────────────────────────────

#[tokio::test]
async fn ensure_default_creates_singleton_idempotent() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    assert!(svc.ensure_default().await.unwrap());
    assert!(!svc.ensure_default().await.unwrap(), "second call is no-op");
    let billing = svc.get().await.unwrap();
    assert_eq!(billing.plan, Plan::Free);
    assert_eq!(billing.status, PlanStatus::Active);
}

#[tokio::test]
async fn get_before_init_errors() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    let err = svc.get().await.unwrap_err();
    assert!(matches!(err, BillingError::NotInitialised));
}

#[tokio::test]
async fn set_plan_creates_singleton_when_absent() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    svc.set_plan(Plan::Pro, Some("cus_123"), None)
        .await
        .unwrap();
    let billing = svc.get().await.unwrap();
    assert_eq!(billing.plan, Plan::Pro);
    assert_eq!(billing.stripe_customer_id.as_deref(), Some("cus_123"));
}

#[tokio::test]
async fn set_plan_upserts_existing_singleton() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    svc.set_plan(Plan::Pro, Some("cus_xxx"), None)
        .await
        .unwrap();
    svc.set_plan(Plan::Enterprise, None, None).await.unwrap();
    let billing = svc.get().await.unwrap();
    assert_eq!(billing.plan, Plan::Enterprise);
    // stripe_customer_id retained (COALESCE preserves it).
    assert_eq!(billing.stripe_customer_id.as_deref(), Some("cus_xxx"));
}

#[tokio::test]
async fn set_status_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    svc.set_status(PlanStatus::PastDue).await.unwrap();
    assert_eq!(svc.get().await.unwrap().status, PlanStatus::PastDue);
    svc.set_status(PlanStatus::Canceled).await.unwrap();
    assert_eq!(svc.get().await.unwrap().status, PlanStatus::Canceled);
}

#[tokio::test]
async fn set_status_before_init_errors() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    let err = svc.set_status(PlanStatus::Active).await.unwrap_err();
    assert!(matches!(err, BillingError::NotInitialised));
}

// ── check_and_record ────────────────────────────────────────

#[tokio::test]
async fn check_and_record_allow_under_limit() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    let decision = svc
        .check_and_record(pid, CounterKind::Events, 1, now())
        .await
        .unwrap();
    assert!(matches!(decision, Decision::Allow { new_count: 1, .. }));
}

#[tokio::test]
async fn check_and_record_increments_existing_row() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    for i in 1..=5 {
        let d = svc
            .check_and_record(pid, CounterKind::Events, 1, now())
            .await
            .unwrap();
        match d {
            Decision::Allow { new_count, .. } => assert_eq!(new_count, i),
            other => panic!("expected Allow, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn check_and_record_at_limit_then_over() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    // Free replays_monthly = 1000. Pre-jump to 999.
    let d_999 = svc
        .check_and_record(pid, CounterKind::Replays, 999, now())
        .await
        .unwrap();
    assert!(matches!(d_999, Decision::Allow { new_count: 999, .. }));
    let d_1000 = svc
        .check_and_record(pid, CounterKind::Replays, 1, now())
        .await
        .unwrap();
    assert!(matches!(
        d_1000,
        Decision::AtLimit {
            new_count: 1000,
            ..
        }
    ));
    let d_over = svc
        .check_and_record(pid, CounterKind::Replays, 1, now())
        .await
        .unwrap();
    match d_over {
        Decision::OverLimit {
            current_count,
            limit,
        } => {
            assert_eq!(current_count, 1000);
            assert_eq!(limit, 1000);
        }
        other => panic!("expected OverLimit, got {other:?}"),
    }
}

#[tokio::test]
async fn check_and_record_first_call_over_limit_rolls_back() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    // Free replays_monthly = 1000. First call with delta
    // way over → INSERT path triggers, rollback fires.
    let d = svc
        .check_and_record(pid, CounterKind::Replays, 10_000, now())
        .await
        .unwrap();
    assert!(matches!(
        d,
        Decision::OverLimit {
            current_count: 0,
            ..
        }
    ));
    // Counter row is absent (rolled back).
    let rows = svc.usage(pid, &period_key(now())).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn enterprise_plan_never_over_limit() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.set_plan(Plan::Enterprise, None, None).await.unwrap();
    let d = svc
        .check_and_record(pid, CounterKind::Events, 10_000_000, now())
        .await
        .unwrap();
    assert!(d.was_recorded());
    assert!(!d.is_over_limit());
}

#[tokio::test]
async fn check_and_record_rejects_zero_delta() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    let err = svc
        .check_and_record(pid, CounterKind::Events, 0, now())
        .await
        .unwrap_err();
    assert!(matches!(err, BillingError::InvalidInput(_)));
}

#[tokio::test]
async fn check_and_record_unknown_project_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    let phantom = ProjectId::new();
    let err = svc
        .check_and_record(phantom, CounterKind::Events, 1, now())
        .await
        .unwrap_err();
    assert!(matches!(err, BillingError::ProjectNotFound(_)));
}

#[tokio::test]
async fn check_and_record_before_init_falls_back_to_free() {
    // A workspace with no `workspace_billing` row yet meters against
    // Free limits instead of erroring — a fresh tenant's very first
    // ingest must not 500 just because `ensure_default` hasn't run.
    // The limit is driven by the project's workspace plan, which
    // falls back to Free when the workspace has no billing row.
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    let decision = svc
        .check_and_record(pid, CounterKind::Events, 1, now())
        .await
        .unwrap();
    assert!(matches!(decision, Decision::Allow { new_count: 1, .. }));
}

#[tokio::test]
async fn different_periods_have_independent_counters() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    let ts_jun = datetime!(2026-06-15 10:00:00 UTC);
    let ts_jul = datetime!(2026-07-15 10:00:00 UTC);
    svc.check_and_record(pid, CounterKind::Events, 50, ts_jun)
        .await
        .unwrap();
    svc.check_and_record(pid, CounterKind::Events, 10, ts_jul)
        .await
        .unwrap();
    let jun = svc.usage(pid, &period_key(ts_jun)).await.unwrap();
    let jul = svc.usage(pid, &period_key(ts_jul)).await.unwrap();
    assert_eq!(jun[0].count, 50);
    assert_eq!(jul[0].count, 10);
}

// ── record_drop ─────────────────────────────────────────────

#[tokio::test]
async fn record_drop_increments() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    svc.record_drop(pid, CounterKind::Events, 3, now())
        .await
        .unwrap();
    svc.record_drop(pid, CounterKind::Events, 7, now())
        .await
        .unwrap();
    let rows = svc.usage(pid, &period_key(now())).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].dropped_count, 10);
    assert_eq!(rows[0].count, 0);
}

#[tokio::test]
async fn record_drop_rejects_zero_delta() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    let err = svc
        .record_drop(pid, CounterKind::Events, 0, now())
        .await
        .unwrap_err();
    assert!(matches!(err, BillingError::InvalidInput(_)));
}

// ── usage / workspace_usage ─────────────────────────────────

#[tokio::test]
async fn usage_returns_all_counter_kinds() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    svc.check_and_record(pid, CounterKind::Events, 5, now())
        .await
        .unwrap();
    svc.check_and_record(pid, CounterKind::Spans, 100, now())
        .await
        .unwrap();
    svc.check_and_record(pid, CounterKind::Replays, 2, now())
        .await
        .unwrap();
    let rows = svc.usage(pid, &period_key(now())).await.unwrap();
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn workspace_usage_sums_across_projects() {
    let (pool, workspace_id) = fresh_pool().await;
    let p1 = seed_project(&pool, workspace_id, "p1").await;
    let p2 = seed_project(&pool, workspace_id, "p2").await;
    let svc = BillingService::new(pool, workspace_id);
    svc.ensure_default().await.unwrap();
    svc.check_and_record(p1, CounterKind::Events, 10, now())
        .await
        .unwrap();
    svc.check_and_record(p2, CounterKind::Events, 20, now())
        .await
        .unwrap();
    let sum = svc.workspace_usage(&period_key(now())).await.unwrap();
    assert_eq!(sum.len(), 1);
    assert_eq!(sum[0].0, CounterKind::Events);
    assert_eq!(sum[0].1, 30);
}

// ── project cascade ─────────────────────────────────────────

#[tokio::test]
async fn project_cascade_drops_usage_rows() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "doomed").await;
    let svc = BillingService::new(pool.clone(), workspace_id);
    svc.ensure_default().await.unwrap();
    svc.check_and_record(pid, CounterKind::Events, 1, now())
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(pid.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let rows = svc.usage(pid, &period_key(now())).await.unwrap();
    assert!(rows.is_empty());
}
