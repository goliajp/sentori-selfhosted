//! Integration tests for `sentori-audit-event`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_audit_event::{AuditEntryDraft, AuditError, AuditQuery, AuditService, actions};
use sentori_workspace_identity::{Identity, ProjectId, UserId, WorkspaceId, bootstrap_workspace};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use time::Duration;
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
        include_str!("../../../migrations/0012_audit_log_indexes.sql"),
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

async fn seed_user(pool: &PgPool, workspace_id: WorkspaceId, email: &str) -> UserId {
    Identity::new(pool.clone(), workspace_id)
        .users()
        .create(
            email,
            "$argon2id$v=19$m=19456,t=2,p=1$YWFhYWFhYWE$YWFhYWFhYWE",
        )
        .await
        .expect("user")
        .id
}

// ── record ───────────────────────────────────────────────────

#[tokio::test]
async fn record_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let actor = seed_user(&pool, workspace_id, "a@example.com").await;
    let svc = AuditService::new(pool);

    let id = svc
        .record(
            AuditEntryDraft::new(workspace_id, actions::PROJECT_CREATED)
                .with_project(pid)
                .with_actor(actor)
                .with_target("project", pid.into_uuid().to_string())
                .with_payload(serde_json::json!({"name": "p1"})),
        )
        .await
        .unwrap();
    assert!(!id.is_nil());

    let entry = svc.find(id).await.unwrap().unwrap();
    assert_eq!(entry.action, actions::PROJECT_CREATED);
    assert_eq!(entry.project_id, Some(pid));
    assert_eq!(entry.actor_user_id, Some(actor));
    assert_eq!(entry.target_type.as_deref(), Some("project"));
    assert_eq!(entry.payload["name"], "p1");
}

#[tokio::test]
async fn record_minimal_draft() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    // System action — no project, no actor.
    let id = svc
        .record(AuditEntryDraft::new(workspace_id, actions::SESSION_LOGIN))
        .await
        .unwrap();
    let entry = svc.find(id).await.unwrap().unwrap();
    assert!(entry.project_id.is_none());
    assert!(entry.actor_user_id.is_none());
    assert!(entry.target_type.is_none());
}

#[tokio::test]
async fn record_rejects_empty_action() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let err = svc
        .record(AuditEntryDraft::new(workspace_id, "   "))
        .await
        .unwrap_err();
    assert!(matches!(err, AuditError::InvalidInput(_)));
}

#[tokio::test]
async fn record_rejects_oversize_action() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let long = "x".repeat(500);
    let err = svc
        .record(AuditEntryDraft::new(workspace_id, long))
        .await
        .unwrap_err();
    assert!(matches!(err, AuditError::InvalidInput(_)));
}

#[tokio::test]
async fn record_unknown_project_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let err = svc
        .record(
            AuditEntryDraft::new(workspace_id, actions::PROJECT_CREATED)
                .with_project(ProjectId::new()),
        )
        .await
        .unwrap_err();
    // FK fires — either Project or Actor variant is acceptable
    // (translate_fk uses constraint name first).
    assert!(matches!(err, AuditError::ProjectNotFound(_)));
}

#[tokio::test]
async fn record_unknown_actor_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let phantom = UserId::new();
    let err = svc
        .record(AuditEntryDraft::new(workspace_id, actions::SESSION_LOGIN).with_actor(phantom))
        .await
        .unwrap_err();
    assert!(matches!(err, AuditError::ActorNotFound(_)));
}

// ── query ────────────────────────────────────────────────────

#[tokio::test]
async fn query_filters_by_project() {
    let (pool, workspace_id) = fresh_pool().await;
    let p1 = seed_project(&pool, workspace_id, "p1").await;
    let p2 = seed_project(&pool, workspace_id, "p2").await;
    let svc = AuditService::new(pool);
    for _ in 0..3 {
        svc.record(AuditEntryDraft::new(workspace_id, "x").with_project(p1))
            .await
            .unwrap();
    }
    for _ in 0..2 {
        svc.record(AuditEntryDraft::new(workspace_id, "x").with_project(p2))
            .await
            .unwrap();
    }
    let rows = svc
        .query(AuditQuery::default().with_project(p1))
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    let count_p2 = svc
        .count(AuditQuery::default().with_project(p2))
        .await
        .unwrap();
    assert_eq!(count_p2, 2);
}

#[tokio::test]
async fn query_filters_by_actor() {
    let (pool, workspace_id) = fresh_pool().await;
    let actor = seed_user(&pool, workspace_id, "a@x.com").await;
    let svc = AuditService::new(pool);
    svc.record(AuditEntryDraft::new(workspace_id, "a").with_actor(actor))
        .await
        .unwrap();
    svc.record(AuditEntryDraft::new(workspace_id, "b").with_actor(actor))
        .await
        .unwrap();
    svc.record(AuditEntryDraft::new(workspace_id, "c"))
        .await
        .unwrap();
    let rows = svc
        .query(AuditQuery::default().with_actor(actor))
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn query_filters_by_action() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    svc.record(AuditEntryDraft::new(workspace_id, "project.created"))
        .await
        .unwrap();
    svc.record(AuditEntryDraft::new(workspace_id, "project.deleted"))
        .await
        .unwrap();
    svc.record(AuditEntryDraft::new(workspace_id, "project.created"))
        .await
        .unwrap();
    let rows = svc
        .query(AuditQuery::default().with_action("project.created"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn query_filters_by_target() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    svc.record(AuditEntryDraft::new(workspace_id, "x").with_target("project", "abc"))
        .await
        .unwrap();
    svc.record(AuditEntryDraft::new(workspace_id, "x").with_target("project", "xyz"))
        .await
        .unwrap();
    let rows = svc
        .query(AuditQuery::default().with_target("project", "abc"))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn query_filters_by_time_window() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let id1 = svc
        .record(AuditEntryDraft::new(workspace_id, "a"))
        .await
        .unwrap();
    let e1 = svc.find(id1).await.unwrap().unwrap();
    let future = e1.created_at + Duration::seconds(1);
    let rows = svc
        .query(AuditQuery::default().within(e1.created_at, future))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let past = e1.created_at - Duration::days(1);
    let earlier = e1.created_at - Duration::seconds(1);
    let none = svc
        .query(AuditQuery::default().within(past, earlier))
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn query_orders_descending_by_created_at() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    for _ in 0..5 {
        svc.record(AuditEntryDraft::new(workspace_id, "x"))
            .await
            .unwrap();
    }
    let rows = svc.query(AuditQuery::default()).await.unwrap();
    assert_eq!(rows.len(), 5);
    for i in 0..(rows.len() - 1) {
        assert!(rows[i].created_at >= rows[i + 1].created_at);
    }
}

#[tokio::test]
async fn query_limit_caps_results() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    for _ in 0..10 {
        svc.record(AuditEntryDraft::new(workspace_id, "x"))
            .await
            .unwrap();
    }
    let rows = svc
        .query(AuditQuery::default().with_limit(3))
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn count_independent_of_limit() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    for _ in 0..7 {
        svc.record(AuditEntryDraft::new(workspace_id, "x"))
            .await
            .unwrap();
    }
    let n = svc
        .count(AuditQuery::default().with_limit(3))
        .await
        .unwrap();
    assert_eq!(n, 7);
}

#[tokio::test]
async fn find_recent_convenience() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AuditService::new(pool);
    for _ in 0..3 {
        svc.record(AuditEntryDraft::new(workspace_id, "x").with_project(pid))
            .await
            .unwrap();
    }
    let rows = svc.find_recent(pid, 10).await.unwrap();
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn find_missing_returns_none() {
    let (pool, _workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    assert!(svc.find(Uuid::now_v7()).await.unwrap().is_none());
}

#[tokio::test]
async fn actor_set_null_when_user_deleted() {
    // ON DELETE SET NULL on actor_user_id — entries survive
    // the actor's deletion but lose the back-ref.
    let (pool, workspace_id) = fresh_pool().await;
    let actor = seed_user(&pool, workspace_id, "byebye@example.com").await;
    let svc = AuditService::new(pool.clone());
    let id = svc
        .record(AuditEntryDraft::new(workspace_id, "x").with_actor(actor))
        .await
        .unwrap();
    // Drop the user via raw SQL (K1's user delete might cascade
    // tables we don't want to depend on here).
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(actor.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let entry = svc.find(id).await.unwrap().unwrap();
    assert!(entry.actor_user_id.is_none(), "FK SET NULL on user delete");
}

#[tokio::test]
async fn project_set_null_when_project_deleted() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "doomed").await;
    let svc = AuditService::new(pool.clone());
    let id = svc
        .record(AuditEntryDraft::new(workspace_id, "x").with_project(pid))
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(pid.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let entry = svc.find(id).await.unwrap().unwrap();
    assert!(entry.project_id.is_none(), "FK SET NULL on project delete");
}

#[tokio::test]
async fn payload_round_trip_through_jsonb() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AuditService::new(pool);
    let payload = serde_json::json!({
        "before": {"name": "old"},
        "after": {"name": "new"},
        "nested": [1, 2, {"x": "y"}],
    });
    let id = svc
        .record(AuditEntryDraft::new(workspace_id, "x").with_payload(payload.clone()))
        .await
        .unwrap();
    let entry = svc.find(id).await.unwrap().unwrap();
    assert_eq!(entry.payload, payload);
}
