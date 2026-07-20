//! Integration tests for `sentori-saved-view`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_saved_view::{
    SavedViewDraft, SavedViewError, SavedViewPatch, SavedViewService, Scope, Target,
};
use sentori_workspace_identity::{Identity, ProjectId, UserId, WorkspaceId, bootstrap_workspace};
use serde_json::json;
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
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
        include_str!("../../../migrations/0014_saved_views.sql"),
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

// ── create ──────────────────────────────────────────────────

#[tokio::test]
async fn create_workspace_view_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(
            SavedViewDraft::new(
                workspace_id,
                "Prod crashes",
                Target::Issues,
                Scope::Workspace,
            )
            .for_project(pid)
            .with_payload(json!({"environment": "production"})),
        )
        .await
        .unwrap();
    let v = svc.find(id).await.unwrap().unwrap();
    assert_eq!(v.name, "Prod crashes");
    assert_eq!(v.target, Target::Issues);
    assert_eq!(v.scope, Scope::Workspace);
    assert_eq!(v.project_id, Some(pid));
    assert!(v.user_id.is_none());
    assert_eq!(v.payload["environment"], "production");
}

#[tokio::test]
async fn create_personal_view_requires_user() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "x",
            Target::Issues,
            Scope::Personal,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::InvalidInput(_)));
}

#[tokio::test]
async fn create_workspace_view_must_not_set_user() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .create(
            SavedViewDraft::new(workspace_id, "x", Target::Issues, Scope::Workspace)
                .owned_by(UserId::new()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::InvalidInput(_)));
}

#[tokio::test]
async fn create_personal_view_with_user() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u1@example.com").await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(
            SavedViewDraft::new(workspace_id, "My view", Target::Events, Scope::Personal)
                .owned_by(user),
        )
        .await
        .unwrap();
    let v = svc.find(id).await.unwrap().unwrap();
    assert_eq!(v.scope, Scope::Personal);
    assert_eq!(v.user_id, Some(user));
    assert_eq!(v.created_by, Some(user));
}

#[tokio::test]
async fn create_rejects_empty_name() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "  ",
            Target::Issues,
            Scope::Workspace,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::InvalidInput(_)));
}

#[tokio::test]
async fn create_unknown_project_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .create(
            SavedViewDraft::new(workspace_id, "x", Target::Issues, Scope::Workspace)
                .for_project(ProjectId::new()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::ProjectNotFound(_)));
}

#[tokio::test]
async fn create_unknown_user_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .create(
            SavedViewDraft::new(workspace_id, "x", Target::Issues, Scope::Personal)
                .owned_by(UserId::new()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::UserNotFound(_)));
}

// ── list_visible_to ─────────────────────────────────────────

#[tokio::test]
async fn list_visible_to_combines_workspace_and_personal() {
    let (pool, workspace_id) = fresh_pool().await;
    let user_a = seed_user(&pool, workspace_id, "a@x.com").await;
    let user_b = seed_user(&pool, workspace_id, "b@x.com").await;
    let svc = SavedViewService::new(pool);
    // Workspace view — visible to anyone.
    let workspace = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "global",
            Target::Issues,
            Scope::Workspace,
        ))
        .await
        .unwrap();
    // Personal view for user A — visible only to A.
    let personal_a = svc
        .create(
            SavedViewDraft::new(workspace_id, "A's view", Target::Issues, Scope::Personal)
                .owned_by(user_a),
        )
        .await
        .unwrap();
    // Personal view for user B — invisible to A.
    let personal_b = svc
        .create(
            SavedViewDraft::new(workspace_id, "B's view", Target::Issues, Scope::Personal)
                .owned_by(user_b),
        )
        .await
        .unwrap();
    // Different target — should not appear in Issues listing.
    let _other_target = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "events",
            Target::Events,
            Scope::Workspace,
        ))
        .await
        .unwrap();

    let visible_a = svc
        .list_visible_to(user_a, None, Target::Issues)
        .await
        .unwrap();
    let ids: Vec<Uuid> = visible_a.iter().map(|v| v.id).collect();
    assert!(ids.contains(&workspace));
    assert!(ids.contains(&personal_a));
    assert!(!ids.contains(&personal_b));
    assert_eq!(visible_a.len(), 2);
}

#[tokio::test]
async fn list_visible_to_with_project_filter() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    let p1 = seed_project(&pool, workspace_id, "p1").await;
    let p2 = seed_project(&pool, workspace_id, "p2").await;
    let svc = SavedViewService::new(pool);

    let p1_view = svc
        .create(
            SavedViewDraft::new(workspace_id, "p1", Target::Issues, Scope::Workspace)
                .for_project(p1),
        )
        .await
        .unwrap();
    let _p2_view = svc
        .create(
            SavedViewDraft::new(workspace_id, "p2", Target::Issues, Scope::Workspace)
                .for_project(p2),
        )
        .await
        .unwrap();
    let workspace_wide = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "any",
            Target::Issues,
            Scope::Workspace,
        ))
        .await
        .unwrap();

    let visible = svc
        .list_visible_to(user, Some(p1), Target::Issues)
        .await
        .unwrap();
    let ids: Vec<Uuid> = visible.iter().map(|v| v.id).collect();
    assert!(ids.contains(&p1_view));
    assert!(ids.contains(&workspace_wide));
    assert_eq!(visible.len(), 2, "p2_view excluded");
}

#[tokio::test]
async fn list_personal_filters_by_user_and_target() {
    let (pool, workspace_id) = fresh_pool().await;
    let user_a = seed_user(&pool, workspace_id, "a@x.com").await;
    let user_b = seed_user(&pool, workspace_id, "b@x.com").await;
    let svc = SavedViewService::new(pool);
    svc.create(
        SavedViewDraft::new(workspace_id, "a-issues", Target::Issues, Scope::Personal)
            .owned_by(user_a),
    )
    .await
    .unwrap();
    svc.create(
        SavedViewDraft::new(workspace_id, "a-events", Target::Events, Scope::Personal)
            .owned_by(user_a),
    )
    .await
    .unwrap();
    svc.create(
        SavedViewDraft::new(workspace_id, "b-issues", Target::Issues, Scope::Personal)
            .owned_by(user_b),
    )
    .await
    .unwrap();
    let list = svc.list_personal(user_a, Target::Issues).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "a-issues");
}

#[tokio::test]
async fn list_workspace_excludes_personal() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    let svc = SavedViewService::new(pool);
    svc.create(SavedViewDraft::new(
        workspace_id,
        "ws",
        Target::Spans,
        Scope::Workspace,
    ))
    .await
    .unwrap();
    svc.create(
        SavedViewDraft::new(workspace_id, "personal", Target::Spans, Scope::Personal)
            .owned_by(user),
    )
    .await
    .unwrap();
    let list = svc.list_workspace(Target::Spans).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "ws");
}

// ── update / delete ─────────────────────────────────────────

#[tokio::test]
async fn update_patches_name_and_payload() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "orig",
            Target::Replays,
            Scope::Workspace,
        ))
        .await
        .unwrap();
    svc.update(
        id,
        SavedViewPatch {
            name: Some("renamed".into()),
            payload: Some(json!({"k": 1})),
        },
    )
    .await
    .unwrap();
    let v = svc.find(id).await.unwrap().unwrap();
    assert_eq!(v.name, "renamed");
    assert_eq!(v.payload["k"], 1);
}

#[tokio::test]
async fn update_patch_only_name() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(
            SavedViewDraft::new(workspace_id, "orig", Target::Metrics, Scope::Workspace)
                .with_payload(json!({"preserved": true})),
        )
        .await
        .unwrap();
    svc.update(
        id,
        SavedViewPatch {
            name: Some("renamed".into()),
            payload: None,
        },
    )
    .await
    .unwrap();
    let v = svc.find(id).await.unwrap().unwrap();
    assert_eq!(v.name, "renamed");
    assert_eq!(v.payload["preserved"], true);
}

#[tokio::test]
async fn update_unknown_returns_not_found() {
    let (pool, _workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let err = svc
        .update(Uuid::now_v7(), SavedViewPatch::default())
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::ViewNotFound(_)));
}

#[tokio::test]
async fn update_rejects_empty_name() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "ok",
            Target::Issues,
            Scope::Workspace,
        ))
        .await
        .unwrap();
    let err = svc
        .update(
            id,
            SavedViewPatch {
                name: Some("   ".into()),
                payload: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SavedViewError::InvalidInput(_)));
}

#[tokio::test]
async fn delete_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = SavedViewService::new(pool);
    let id = svc
        .create(SavedViewDraft::new(
            workspace_id,
            "x",
            Target::Issues,
            Scope::Workspace,
        ))
        .await
        .unwrap();
    svc.delete(id).await.unwrap();
    assert!(svc.find(id).await.unwrap().is_none());
    // Idempotent.
    svc.delete(id).await.unwrap();
}

// ── cascades ────────────────────────────────────────────────

#[tokio::test]
async fn project_cascade_drops_scoped_views() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "doomed").await;
    let svc = SavedViewService::new(pool.clone());
    let id = svc
        .create(
            SavedViewDraft::new(workspace_id, "p", Target::Issues, Scope::Workspace)
                .for_project(pid),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(pid.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(svc.find(id).await.unwrap().is_none());
}

#[tokio::test]
async fn user_cascade_drops_personal_views() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "byebye@x.com").await;
    let svc = SavedViewService::new(pool.clone());
    let id = svc
        .create(
            SavedViewDraft::new(workspace_id, "p", Target::Issues, Scope::Personal).owned_by(user),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(svc.find(id).await.unwrap().is_none(), "user FK cascade");
}
