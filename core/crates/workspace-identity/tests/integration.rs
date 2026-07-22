//! Integration tests for `sentori-workspace-identity` against a
//! real postgres 18 spun up via testcontainers.
//!
//! One container is started per test bin (`OnceLock`), shared
//! across tests via a database-per-test pattern: each test
//! creates its own pristine database inside the container, runs
//! the migration, and drops the database on completion. This
//! keeps tests isolated without paying container-startup cost
//! per test (3-5s × N tests would be brutal).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_workspace_identity::{
    Identity, IdentityError, InviteRole, InviteToken, Role, UserId, WorkspaceId,
    bootstrap_workspace,
};
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
        // Locked to postgres:18 — the version our prod target
        // runs. CI must have the image pre-pulled (no fallback
        // env knob: drift between dev / CI / prod versions is
        // exactly the kind of "works on my box" the K-tier is
        // supposed to prevent).
        let container = Postgres::default()
            .with_tag("18")
            .start()
            .await
            .expect("start postgres container");
        let host = container.get_host().await.expect("host");
        let port = container.get_host_port_ipv4(5432).await.expect("port");
        // testcontainers-modules' Postgres image defaults: user=postgres,
        // password=postgres, db=postgres. We connect to `postgres` to
        // create per-test databases.
        let base_url = format!("postgres://postgres:postgres@{host}:{port}");
        *guard = Some(PgRig {
            _container: container,
            base_url,
        });
    }
    guard.as_ref().expect("rig set").base_url.clone()
}

/// Spawn a fresh database in the shared postgres container,
/// apply the migration, return a connected pool.
async fn fresh_pool() -> (PgPool, WorkspaceId) {
    let base = ensure_rig().await;
    let admin_url = format!("{base}/postgres");
    let admin = PgPool::connect(&admin_url).await.expect("connect admin db");

    // Unique db name — uuid simple form (no dashes; Postgres
    // identifier rules don't love hyphens without quoting).
    let db_name = format!("t_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test db");
    drop(admin);

    let url = format!("{base}/{db_name}");
    let pool = PgPool::connect(&url).await.expect("connect test db");
    apply_migration(&pool).await;
    let workspace_id = bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

async fn apply_migration(pool: &PgPool) {
    let sql = include_str!("../../../migrations/0001_workspace_identity.sql");
    // Postgres simple-query protocol accepts multi-statement
    // strings; sqlx exposes it via `Executor::execute(sql)` on
    // `&str`. No fragile splitter needed.
    pool.execute(sql).await.expect("apply migration");
}

// ── helpers ───────────────────────────────────────────────────

async fn make_user(identity: &Identity, email: &str) -> UserId {
    identity
        .users()
        .create(email, "$2b$05$fake.hash")
        .await
        .expect("create user")
        .id
}

async fn make_owner(identity: &Identity, email: &str) -> UserId {
    let id = make_user(identity, email).await;
    identity
        .members()
        .add(id, Role::Owner, None)
        .await
        .expect("seed owner");
    id
}

const fn fake_salt() -> [u8; 32] {
    [0xa5; 32]
}

// ── users ─────────────────────────────────────────────────────

#[tokio::test]
async fn users_create_and_lookup_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let u = id
        .users()
        .create("alice@example.com", "$2b$05$hash")
        .await
        .expect("create");
    assert_eq!(u.email, "alice@example.com");
    assert!(!u.email_verified);

    let by_id = id
        .users()
        .find_by_id(u.id)
        .await
        .expect("by id")
        .expect("found");
    assert_eq!(by_id.id, u.id);

    // Case-insensitive email lookup.
    let by_email = id
        .users()
        .find_by_email("ALICE@example.com")
        .await
        .expect("by email")
        .expect("found");
    assert_eq!(by_email.id, u.id);

    let lookup = id
        .users()
        .lookup_password_hash("Alice@example.com")
        .await
        .expect("lookup hash")
        .expect("present");
    assert_eq!(lookup.0, u.id);
    assert_eq!(lookup.1, "$2b$05$hash");
}

#[tokio::test]
async fn users_duplicate_email_case_insensitive_taken() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    id.users()
        .create("bob@x.com", "$2b$05$h")
        .await
        .expect("first");
    let err = id
        .users()
        .create("BOB@x.com", "$2b$05$h2")
        .await
        .expect_err("dup");
    assert!(matches!(err, IdentityError::EmailTaken), "got: {err:?}");
}

#[tokio::test]
async fn users_mark_verified_and_change_password() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let u = id
        .users()
        .create("carol@x.com", "$2b$05$old")
        .await
        .expect("create");
    id.users().mark_email_verified(u.id).await.expect("verify");
    let after = id
        .users()
        .find_by_id(u.id)
        .await
        .expect("ok")
        .expect("found");
    assert!(after.email_verified);

    id.users()
        .update_password_hash(u.id, "$2b$05$new")
        .await
        .expect("update");
    let lookup = id
        .users()
        .lookup_password_hash("carol@x.com")
        .await
        .expect("lookup")
        .expect("present");
    assert_eq!(lookup.1, "$2b$05$new");

    // Errors on unknown user.
    let phantom = UserId::new();
    assert!(matches!(
        id.users().mark_email_verified(phantom).await.unwrap_err(),
        IdentityError::UserNotFound(_)
    ));
    assert!(matches!(
        id.users()
            .update_password_hash(phantom, "x")
            .await
            .unwrap_err(),
        IdentityError::UserNotFound(_)
    ));
}

#[tokio::test]
async fn users_lookup_unknown_email_is_none() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    assert!(
        id.users()
            .find_by_email("ghost@x.com")
            .await
            .expect("ok")
            .is_none()
    );
    assert!(
        id.users()
            .lookup_password_hash("ghost@x.com")
            .await
            .expect("ok")
            .is_none()
    );
}

// ── members ───────────────────────────────────────────────────

#[tokio::test]
async fn members_owner_and_admins() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let admin_id = make_user(&id, "admin@x.com").await;
    id.members()
        .add(admin_id, Role::Admin, Some(owner_id))
        .await
        .expect("add admin");

    let owner = id
        .members()
        .find_owner()
        .await
        .expect("owner")
        .expect("present");
    assert_eq!(owner.user_id, owner_id);
    assert_eq!(owner.role, Role::Owner);
    assert_eq!(owner.added_by, None);

    let list = id.members().list().await.expect("list");
    assert_eq!(list.len(), 2);
    assert!(
        list.iter()
            .any(|m| m.user_id == admin_id && m.role == Role::Admin)
    );
}

#[tokio::test]
async fn members_second_owner_rejected_by_db_constraint() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    make_owner(&id, "owner@x.com").await;
    let second = make_user(&id, "second@x.com").await;
    // Direct add() with role=owner should hit the partial unique index.
    let err = id
        .members()
        .add(second, Role::Owner, None)
        .await
        .expect_err("second owner");
    assert!(matches!(err, IdentityError::Db(_)), "got: {err:?}");
}

#[tokio::test]
async fn members_set_role_refuses_owner_and_demote() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let admin_id = make_user(&id, "admin@x.com").await;
    id.members()
        .add(admin_id, Role::Admin, Some(owner_id))
        .await
        .expect("add");

    // Cannot set anyone to Owner via set_role.
    let err = id
        .members()
        .set_role(admin_id, Role::Owner)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::Db(_)), "got: {err:?}");

    // Cannot demote the sole owner.
    let err = id
        .members()
        .set_role(owner_id, Role::User)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::Db(_)), "got: {err:?}");

    // Demoting admin to user works.
    id.members()
        .set_role(admin_id, Role::User)
        .await
        .expect("demote");
    let m = id
        .members()
        .find(admin_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(m.role, Role::User);

    // Unknown user surfaces NotAMember.
    let phantom = UserId::new();
    assert!(matches!(
        id.members()
            .set_role(phantom, Role::User)
            .await
            .unwrap_err(),
        IdentityError::NotAMember(_)
    ));
}

#[tokio::test]
async fn members_remove_owner_blocked_and_unknown_is_not_a_member() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let user_id = make_user(&id, "user@x.com").await;
    id.members()
        .add(user_id, Role::User, Some(owner_id))
        .await
        .expect("add");

    let err = id.members().remove(owner_id).await.unwrap_err();
    assert!(matches!(err, IdentityError::Db(_)), "got: {err:?}");

    id.members().remove(user_id).await.expect("remove user");
    assert!(id.members().find(user_id).await.expect("find").is_none());

    let phantom = UserId::new();
    assert!(matches!(
        id.members().remove(phantom).await.unwrap_err(),
        IdentityError::NotAMember(_)
    ));
}

#[tokio::test]
async fn members_transfer_owner_atomic() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let next_id = make_user(&id, "next@x.com").await;
    id.members()
        .add(next_id, Role::Admin, Some(owner_id))
        .await
        .expect("seed admin");

    id.members()
        .transfer_owner(next_id)
        .await
        .expect("transfer");

    let owner = id
        .members()
        .find_owner()
        .await
        .expect("owner")
        .expect("present");
    assert_eq!(owner.user_id, next_id);

    let prev = id
        .members()
        .find(owner_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(prev.role, Role::Admin);
}

#[tokio::test]
async fn members_transfer_errors() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let phantom = UserId::new();
    let err = id.members().transfer_owner(phantom).await.unwrap_err();
    assert!(
        matches!(err, IdentityError::TransferTargetNotMember(_)),
        "got: {err:?}"
    );

    let err = id.members().transfer_owner(owner_id).await.unwrap_err();
    assert!(
        matches!(err, IdentityError::TransferTargetAlreadyOwner),
        "got: {err:?}"
    );
}

// ── projects ──────────────────────────────────────────────────

#[tokio::test]
async fn projects_create_and_lookup() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let p = id
        .projects()
        .create("Frontend", "frontend", &fake_salt())
        .await
        .expect("create");
    assert_eq!(p.name, "Frontend");
    assert_eq!(p.slug, "frontend");

    let by_id = id
        .projects()
        .find(p.id)
        .await
        .expect("by id")
        .expect("present");
    assert_eq!(by_id.id, p.id);
    let by_slug = id
        .projects()
        .find_by_slug("frontend")
        .await
        .expect("by slug")
        .expect("present");
    assert_eq!(by_slug.id, p.id);

    let all = id.projects().list_all().await.expect("list");
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn projects_duplicate_slug_taken() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    id.projects()
        .create("Web", "web", &fake_salt())
        .await
        .expect("first");
    let err = id
        .projects()
        .create("Web 2", "web", &fake_salt())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::SlugTaken(_)), "got: {err:?}");
}

#[tokio::test]
async fn projects_delete_and_not_found() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let p = id
        .projects()
        .create("Tmp", "tmp", &fake_salt())
        .await
        .expect("create");
    id.projects().delete(p.id).await.expect("delete");
    assert!(id.projects().find(p.id).await.expect("ok").is_none());

    let err = id.projects().delete(p.id).await.unwrap_err();
    assert!(
        matches!(err, IdentityError::ProjectNotFound(_)),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn projects_list_visible_owner_admin_user() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let admin_id = make_user(&id, "admin@x.com").await;
    id.members()
        .add(admin_id, Role::Admin, Some(owner_id))
        .await
        .expect("add admin");
    let user_id = make_user(&id, "user@x.com").await;
    id.members()
        .add(user_id, Role::User, Some(owner_id))
        .await
        .expect("add user");

    let p1 = id
        .projects()
        .create("Frontend", "frontend", &fake_salt())
        .await
        .expect("p1");
    let p2 = id
        .projects()
        .create("Backend", "backend", &fake_salt())
        .await
        .expect("p2");

    // Owner sees both.
    let owner_view = id.projects().list_visible_to(owner_id).await.expect("ok");
    assert_eq!(owner_view.len(), 2);

    // Admin sees both.
    let admin_view = id.projects().list_visible_to(admin_id).await.expect("ok");
    assert_eq!(admin_view.len(), 2);

    // Plain user sees none yet.
    let user_view = id.projects().list_visible_to(user_id).await.expect("ok");
    assert!(user_view.is_empty());

    // Grant user visibility on p1.
    id.visibility()
        .grant(p1.id, user_id, owner_id)
        .await
        .expect("grant");
    let user_view = id.projects().list_visible_to(user_id).await.expect("ok");
    assert_eq!(user_view.len(), 1);
    assert_eq!(user_view[0].id, p1.id);

    // Non-member sees none.
    let stranger = make_user(&id, "stranger@x.com").await;
    let v = id.projects().list_visible_to(stranger).await.expect("ok");
    assert!(v.is_empty());

    // p2 still hidden.
    let _ = p2;
}

// ── visibility ────────────────────────────────────────────────

#[tokio::test]
async fn visibility_grant_refused_for_admin_and_owner() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let admin_id = make_user(&id, "admin@x.com").await;
    id.members()
        .add(admin_id, Role::Admin, Some(owner_id))
        .await
        .expect("add");

    let p = id
        .projects()
        .create("X", "x", &fake_salt())
        .await
        .expect("p");

    let err = id
        .visibility()
        .grant(p.id, owner_id, owner_id)
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::VisibilityRefusedForElevatedRole),
        "got: {err:?}"
    );
    let err = id
        .visibility()
        .grant(p.id, admin_id, owner_id)
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::VisibilityRefusedForElevatedRole),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn visibility_grant_for_unknown_user_or_project() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let user_id = make_user(&id, "user@x.com").await;
    id.members()
        .add(user_id, Role::User, Some(owner_id))
        .await
        .expect("add");

    let phantom_project = sentori_workspace_identity::ProjectId::new();
    let err = id
        .visibility()
        .grant(phantom_project, user_id, owner_id)
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::ProjectNotFound(_)),
        "got: {err:?}"
    );

    let p = id
        .projects()
        .create("Y", "y", &fake_salt())
        .await
        .expect("p");
    let stranger = UserId::new();
    let err = id
        .visibility()
        .grant(p.id, stranger, owner_id)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::NotAMember(_)), "got: {err:?}");
}

#[tokio::test]
async fn visibility_grant_idempotent_and_revoke_idempotent() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let user_id = make_user(&id, "user@x.com").await;
    id.members()
        .add(user_id, Role::User, Some(owner_id))
        .await
        .expect("add");

    let p = id
        .projects()
        .create("Z", "z", &fake_salt())
        .await
        .expect("p");
    id.visibility()
        .grant(p.id, user_id, owner_id)
        .await
        .expect("first");
    id.visibility()
        .grant(p.id, user_id, owner_id)
        .await
        .expect("second (idempotent)");

    let users_for_p = id.visibility().list_for_project(p.id).await.expect("list");
    assert_eq!(users_for_p, vec![user_id]);

    let projs_for_u = id.visibility().list_for_user(user_id).await.expect("list");
    assert_eq!(projs_for_u, vec![p.id]);

    id.visibility().revoke(p.id, user_id).await.expect("revoke");
    id.visibility()
        .revoke(p.id, user_id)
        .await
        .expect("revoke (idempotent)");
    assert!(
        id.visibility()
            .list_for_project(p.id)
            .await
            .expect("ok")
            .is_empty()
    );
}

#[tokio::test]
async fn visibility_promotion_cleans_up_user_acl_rows() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let user_id = make_user(&id, "user@x.com").await;
    id.members()
        .add(user_id, Role::User, Some(owner_id))
        .await
        .expect("add");

    let p = id
        .projects()
        .create("Q", "q", &fake_salt())
        .await
        .expect("p");
    id.visibility()
        .grant(p.id, user_id, owner_id)
        .await
        .expect("grant");

    // Promote to admin — should wipe the ACL row.
    id.members()
        .set_role(user_id, Role::Admin)
        .await
        .expect("promote");
    assert!(
        id.visibility()
            .list_for_project(p.id)
            .await
            .expect("ok")
            .is_empty()
    );
}

// ── invites ───────────────────────────────────────────────────

#[tokio::test]
async fn invites_mint_list_revoke() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;

    let minted = id
        .invites()
        .create("new@x.com", InviteRole::Admin, owner_id, 7)
        .await
        .expect("mint");

    assert_eq!(minted.invite.email, "new@x.com");
    assert_eq!(minted.invite.role, InviteRole::Admin);
    assert_eq!(minted.invite.invited_by, owner_id);
    assert!(minted.invite.is_pending());
    assert!(!minted.invite.is_expired(OffsetDateTime::now_utc()));
    assert_eq!(minted.plaintext_token.to_wire_string().len(), 43);

    let pending = id.invites().list_pending().await.expect("list pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, minted.invite.id);

    let all = id.invites().list_all().await.expect("list all");
    assert_eq!(all.len(), 1);

    id.invites().revoke(minted.invite.id).await.expect("revoke");
    id.invites()
        .revoke(minted.invite.id)
        .await
        .expect("revoke idempotent");
    assert!(id.invites().list_pending().await.expect("ok").is_empty());
}

#[tokio::test]
async fn invites_create_expiry_validation() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;

    let err = id
        .invites()
        .create("a@x.com", InviteRole::User, owner_id, 0)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        IdentityError::InviteExpiryOutOfRange { got: 0, .. }
    ));

    let err = id
        .invites()
        .create("a@x.com", InviteRole::User, owner_id, 31)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        IdentityError::InviteExpiryOutOfRange { got: 31, .. }
    ));
}

#[tokio::test]
async fn invites_accept_happy_path_and_double_accept_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let minted = id
        .invites()
        .create("teammate@x.com", InviteRole::User, owner_id, 7)
        .await
        .expect("mint");

    // The accepting user must exist first (UI flow: invite link
    // → register form → POST accept).
    let new_user = make_user(&id, "teammate@x.com").await;

    let member = id
        .invites()
        .accept(&minted.plaintext_token.to_wire_string(), new_user)
        .await
        .expect("accept");
    assert_eq!(member.user_id, new_user);
    assert_eq!(member.role, Role::User);

    // Double-accept fails.
    let err = id
        .invites()
        .accept(&minted.plaintext_token.to_wire_string(), new_user)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::InviteInvalid), "got: {err:?}");
}

#[tokio::test]
async fn invites_accept_invalid_token_errors() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let _owner_id = make_owner(&id, "owner@x.com").await;
    let new_user = make_user(&id, "anon@x.com").await;

    // Random-but-well-formed token; no matching hash.
    let phantom_token = InviteToken::generate().expect("rng");
    let err = id
        .invites()
        .accept(&phantom_token.to_wire_string(), new_user)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::InviteInvalid), "got: {err:?}");

    // Malformed token (not base64url, wrong length).
    let err = id.invites().accept("!!!", new_user).await.unwrap_err();
    assert!(matches!(err, IdentityError::InviteInvalid));
}

#[tokio::test]
async fn invites_accept_expired_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;
    let minted = id
        .invites()
        .create("teammate@x.com", InviteRole::User, owner_id, 1)
        .await
        .expect("mint");

    // Force expire via direct UPDATE.
    sqlx::query(
        "UPDATE workspace_invites SET expires_at = now() - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(minted.invite.id)
    .execute(id.pool())
    .await
    .expect("force expire");

    let new_user = make_user(&id, "teammate@x.com").await;
    let err = id
        .invites()
        .accept(&minted.plaintext_token.to_wire_string(), new_user)
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::InviteInvalid), "got: {err:?}");
}

#[tokio::test]
async fn invites_list_pending_skips_expired_and_accepted() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);

    let owner_id = make_owner(&id, "owner@x.com").await;

    // Pending invite.
    let pending = id
        .invites()
        .create("p@x.com", InviteRole::User, owner_id, 7)
        .await
        .expect("mint");

    // Already-accepted invite.
    let acc = id
        .invites()
        .create("a@x.com", InviteRole::User, owner_id, 7)
        .await
        .expect("mint");
    let new_user = make_user(&id, "a@x.com").await;
    id.invites()
        .accept(&acc.plaintext_token.to_wire_string(), new_user)
        .await
        .expect("accept");

    // Expired invite.
    let exp = id
        .invites()
        .create("e@x.com", InviteRole::User, owner_id, 1)
        .await
        .expect("mint");
    sqlx::query(
        "UPDATE workspace_invites SET expires_at = now() - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(exp.invite.id)
    .execute(id.pool())
    .await
    .expect("force expire");

    let pending_only = id.invites().list_pending().await.expect("list pending");
    assert_eq!(pending_only.len(), 1);
    assert_eq!(pending_only[0].id, pending.invite.id);

    let all = id.invites().list_all().await.expect("list all");
    assert_eq!(all.len(), 3);
}

// ── Identity surface ──────────────────────────────────────────

#[tokio::test]
async fn identity_pool_accessor_works() {
    let (pool, workspace_id) = fresh_pool().await;
    let id = Identity::new(pool, workspace_id);
    let row: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(id.pool())
        .await
        .expect("query");
    assert_eq!(row.0, 1);
}
