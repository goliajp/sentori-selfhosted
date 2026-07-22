//! Integration tests for `sentori-tenant-scoping`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_tenant_scoping::{Permission, TenantError, TenantGuard};
use sentori_workspace_identity::{
    Identity, ProjectId, Role, UserId, WorkspaceId, bootstrap_workspace,
};
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
    pool.execute(include_str!(
        "../../../migrations/0001_workspace_identity.sql"
    ))
    .await
    .expect("migration");
    let workspace_id = bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
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

async fn add_member(pool: &PgPool, workspace_id: WorkspaceId, user: UserId, role: Role) {
    Identity::new(pool.clone(), workspace_id)
        .members()
        .add(user, role, None)
        .await
        .expect("member");
}

async fn seed_project(pool: &PgPool, workspace_id: WorkspaceId, slug: &str) -> ProjectId {
    Identity::new(pool.clone(), workspace_id)
        .projects()
        .create(slug, slug, &[0xa5u8; 32])
        .await
        .expect("project")
        .id
}

// ── member_role ─────────────────────────────────────────────

#[tokio::test]
async fn member_role_returns_none_for_non_member() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "ghost@x.com").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert!(guard.member_role(user).await.unwrap().is_none());
}

#[tokio::test]
async fn member_role_returns_role() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::Owner).await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert_eq!(guard.member_role(user).await.unwrap(), Some(Role::Owner));
}

// ── can_view_project ────────────────────────────────────────

#[tokio::test]
async fn owner_can_view_any_project() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, user, Role::Owner).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert!(guard.can_view_project(user, pid).await.unwrap());
}

#[tokio::test]
async fn admin_can_view_any_project() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "a@x.com").await;
    add_member(&pool, workspace_id, user, Role::Admin).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert!(guard.can_view_project(user, pid).await.unwrap());
}

#[tokio::test]
async fn user_can_view_only_granted_project() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid_seen = seed_project(&pool, workspace_id, "yes").await;
    let pid_unseen = seed_project(&pool, workspace_id, "no").await;

    // Grant visibility on pid_seen only.
    Identity::new(pool.clone(), workspace_id)
        .visibility()
        .grant(pid_seen, user, owner)
        .await
        .expect("grant");

    let guard = TenantGuard::new(pool, workspace_id);
    assert!(guard.can_view_project(user, pid_seen).await.unwrap());
    assert!(!guard.can_view_project(user, pid_unseen).await.unwrap());
}

#[tokio::test]
async fn non_member_cannot_view_anything() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "ghost@x.com").await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert!(!guard.can_view_project(user, pid).await.unwrap());
}

// ── visible_projects ────────────────────────────────────────

#[tokio::test]
async fn admin_visible_projects_returns_all() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "a@x.com").await;
    add_member(&pool, workspace_id, user, Role::Admin).await;
    let _p1 = seed_project(&pool, workspace_id, "p1").await;
    let _p2 = seed_project(&pool, workspace_id, "p2").await;
    let _p3 = seed_project(&pool, workspace_id, "p3").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert_eq!(guard.visible_projects(user).await.unwrap().len(), 3);
}

#[tokio::test]
async fn user_visible_projects_returns_granted_only() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let p1 = seed_project(&pool, workspace_id, "p1").await;
    let _p2 = seed_project(&pool, workspace_id, "p2").await;
    Identity::new(pool.clone(), workspace_id)
        .visibility()
        .grant(p1, user, owner)
        .await
        .expect("grant");
    let guard = TenantGuard::new(pool, workspace_id);
    let visible = guard.visible_projects(user).await.unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], p1);
}

#[tokio::test]
async fn non_member_visible_projects_empty() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "ghost@x.com").await;
    let _p = seed_project(&pool, workspace_id, "p").await;
    let guard = TenantGuard::new(pool, workspace_id);
    assert!(guard.visible_projects(user).await.unwrap().is_empty());
}

// ── can_perform / assert_can_perform ────────────────────────

#[tokio::test]
async fn owner_can_perform_all_permissions() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, user, Role::Owner).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    for p in Permission::ALL {
        assert!(
            guard.can_perform(user, pid, p).await.unwrap(),
            "owner ✓ {p}"
        );
        guard.assert_can_perform(user, pid, p).await.unwrap();
    }
}

#[tokio::test]
async fn admin_blocked_on_promote_and_transfer() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "a@x.com").await;
    add_member(&pool, workspace_id, user, Role::Admin).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    for p in [Permission::PromoteToAdmin, Permission::TransferOwner] {
        assert!(!guard.can_perform(user, pid, p).await.unwrap());
        let err = guard.assert_can_perform(user, pid, p).await.unwrap_err();
        assert!(matches!(err, TenantError::InsufficientRole { .. }));
    }
}

#[tokio::test]
async fn user_role_project_scoped_requires_visibility() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool.clone(), workspace_id);

    // Without visibility — denied.
    let err = guard
        .assert_can_perform(user, pid, Permission::ViewProject)
        .await
        .unwrap_err();
    assert!(matches!(err, TenantError::NotVisible { .. }));

    // Grant + retry — allowed.
    Identity::new(pool, workspace_id)
        .visibility()
        .grant(pid, user, owner)
        .await
        .unwrap();
    guard
        .assert_can_perform(user, pid, Permission::ViewProject)
        .await
        .unwrap();
}

#[tokio::test]
async fn user_role_workspace_scoped_blocked_by_role() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    // ManageMembers is workspace-scoped (not project-gated)
    // and only Owner/Admin have the role permit.
    let err = guard
        .assert_can_perform(user, pid, Permission::ManageMembers)
        .await
        .unwrap_err();
    assert!(matches!(err, TenantError::InsufficientRole { .. }));
}

#[tokio::test]
async fn non_member_blocked_by_not_a_member() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "ghost@x.com").await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    let err = guard
        .assert_can_perform(user, pid, Permission::ViewProject)
        .await
        .unwrap_err();
    assert!(matches!(err, TenantError::NotAMember(_)));
}

// ── grant_visibility / revoke_visibility (self-gated) ───────

#[tokio::test]
async fn owner_can_grant_visibility() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    guard.grant_visibility(owner, user, pid).await.unwrap();
    assert!(guard.can_view_project(user, pid).await.unwrap());
}

#[tokio::test]
async fn admin_can_grant_visibility() {
    let (pool, workspace_id) = fresh_pool().await;
    let admin = seed_user(&pool, workspace_id, "a@x.com").await;
    add_member(&pool, workspace_id, admin, Role::Admin).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    guard.grant_visibility(admin, user, pid).await.unwrap();
}

#[tokio::test]
async fn user_cannot_grant_visibility() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let other = seed_user(&pool, workspace_id, "ot@x.com").await;
    add_member(&pool, workspace_id, other, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    let err = guard.grant_visibility(user, other, pid).await.unwrap_err();
    assert!(matches!(err, TenantError::InsufficientRole { .. }));
}

#[tokio::test]
async fn non_member_cannot_grant_visibility() {
    let (pool, workspace_id) = fresh_pool().await;
    let ghost = seed_user(&pool, workspace_id, "g@x.com").await;
    let target = seed_user(&pool, workspace_id, "t@x.com").await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    let err = guard
        .grant_visibility(ghost, target, pid)
        .await
        .unwrap_err();
    assert!(matches!(err, TenantError::NotAMember(_)));
}

#[tokio::test]
async fn revoke_visibility_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let owner = seed_user(&pool, workspace_id, "o@x.com").await;
    add_member(&pool, workspace_id, owner, Role::Owner).await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::User).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    guard.grant_visibility(owner, user, pid).await.unwrap();
    assert!(guard.can_view_project(user, pid).await.unwrap());
    guard.revoke_visibility(owner, user, pid).await.unwrap();
    assert!(!guard.can_view_project(user, pid).await.unwrap());
}

// ── assert_can_view_project sugar ───────────────────────────

#[tokio::test]
async fn assert_can_view_sugar_matches_assert_can_perform_view() {
    let (pool, workspace_id) = fresh_pool().await;
    let user = seed_user(&pool, workspace_id, "u@x.com").await;
    add_member(&pool, workspace_id, user, Role::Admin).await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let guard = TenantGuard::new(pool, workspace_id);
    guard.assert_can_view_project(user, pid).await.unwrap();
}
