//! Integration tests for `sentori-auth-session` against a real
//! postgres 18 via testcontainers. Same database-per-test
//! pattern as K1 — one container per test bin, fresh database
//! per test.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_auth_session::{AuthError, AuthOptions, AuthService, RequestMeta};
use sentori_cookie_session::SecretKey;
use sentori_workspace_identity::{Identity, Role, WorkspaceId};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
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
        let base_url = format!("postgres://postgres:postgres@{host}:{port}");
        *guard = Some(PgRig {
            _container: container,
            base_url,
        });
    }
    guard.as_ref().expect("rig set").base_url.clone()
}

async fn fresh_pool() -> (PgPool, WorkspaceId) {
    let base = ensure_rig().await;
    let admin_url = format!("{base}/postgres");
    let admin = PgPool::connect(&admin_url).await.expect("connect admin");
    let db_name = format!("t_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test db");
    drop(admin);

    let url = format!("{base}/{db_name}");
    let pool = PgPool::connect(&url).await.expect("connect test db");
    apply_migrations(&pool).await;
    let workspace_id = sentori_workspace_identity::bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

async fn apply_migrations(pool: &PgPool) {
    let m0001 = include_str!("../../../migrations/0001_workspace_identity.sql");
    let m0002 = include_str!("../../../migrations/0002_auth_session.sql");
    pool.execute(m0001).await.expect("0001 migration");
    pool.execute(m0002).await.expect("0002 migration");
}

// ── service builder ───────────────────────────────────────────

fn cookie_key() -> SecretKey {
    SecretKey::generate().expect("rng")
}

fn service(pool: PgPool, workspace_id: WorkspaceId) -> AuthService {
    AuthService::new(
        Identity::new(pool, workspace_id),
        cookie_key(),
        AuthOptions::default(),
    )
}

fn meta() -> RequestMeta {
    RequestMeta {
        ip: Some("127.0.0.1".into()),
        user_agent: Some("integration-test/1".into()),
    }
}

async fn seed_owner_membership(auth: &AuthService, user_id: sentori_workspace_identity::UserId) {
    auth.identity()
        .members()
        .add(user_id, Role::Owner, None)
        .await
        .expect("seed owner");
}

// ── register + verify ────────────────────────────────────────

#[tokio::test]
async fn register_and_verify_round_trip() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, minted) = auth
        .register("Alice@example.com", "verysecret")
        .await
        .expect("register");
    assert_eq!(user.email, "alice@example.com"); // normalised
    assert!(!user.email_verified);
    assert_eq!(minted.plaintext_token.to_wire_string().len(), 43);

    let uid = auth
        .verify_email(&minted.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    assert_eq!(uid, user.id);
    let after = auth
        .identity()
        .users()
        .find_by_id(user.id)
        .await
        .expect("ok")
        .expect("present");
    assert!(after.email_verified);
}

#[tokio::test]
async fn register_rejects_invalid_inputs() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let err = auth.register("noatsign", "verysecret").await.unwrap_err();
    assert!(matches!(err, AuthError::EmailInvalid));

    let err = auth
        .register("alice@example.com", "short")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::PasswordTooShort { .. }));
}

#[tokio::test]
async fn register_dup_email_surfaces_identity_taken() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    auth.register("dup@example.com", "verysecret")
        .await
        .expect("first");
    let err = auth
        .register("DUP@example.com", "verysecret2")
        .await
        .unwrap_err();
    // Email collision propagates as identity error.
    assert!(matches!(err, AuthError::Identity(_)));
}

#[tokio::test]
async fn verify_email_rejects_bad_tokens() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let err = auth.verify_email("!!").await.unwrap_err();
    assert!(matches!(err, AuthError::TokenInvalid));

    let phantom = sentori_auth_session::EmailVerifyToken::generate().expect("gen");
    let err = auth
        .verify_email(&phantom.to_wire_string())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::TokenInvalid));
}

#[tokio::test]
async fn double_verify_fails_after_first() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let (_user, minted) = auth
        .register("once@example.com", "verysecret")
        .await
        .expect("register");
    let wire = minted.plaintext_token.to_wire_string();
    auth.verify_email(&wire).await.expect("first");
    let err = auth.verify_email(&wire).await.unwrap_err();
    assert!(matches!(err, AuthError::TokenInvalid));
}

#[tokio::test]
async fn resend_email_verification_is_silent_when_unknown_or_verified() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let resent = auth
        .resend_email_verification("ghost@example.com")
        .await
        .expect("ok");
    assert!(resent.is_none());

    let (_user, minted) = auth
        .register("bob@example.com", "verysecret")
        .await
        .expect("register");
    auth.verify_email(&minted.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let resent = auth
        .resend_email_verification("bob@example.com")
        .await
        .expect("ok");
    assert!(resent.is_none());
}

// ── login ─────────────────────────────────────────────────────

#[tokio::test]
async fn login_happy_path_and_cookie_lookup() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, minted_verify) = auth
        .register("login@example.com", "verysecret")
        .await
        .expect("register");
    auth.verify_email(&minted_verify.plaintext_token.to_wire_string())
        .await
        .expect("verify");

    let (logged_user, minted_session) = auth
        .login("login@example.com", "verysecret", &meta())
        .await
        .expect("login");
    assert_eq!(logged_user.id, user.id);
    assert_eq!(minted_session.session.user_id, user.id);

    // Build cookie via axum helper + look it up through service.
    let cookie = sentori_auth_session::axum::build_session_cookie(
        &auth,
        &minted_session.session_id.to_wire_string(),
        minted_session.session.expires_at,
    );
    let cookie_value = cookie.value().to_string();
    let resolved = auth
        .lookup_session(&cookie_value)
        .await
        .expect("lookup")
        .expect("present");
    assert_eq!(resolved.0.id, user.id);
    assert_eq!(resolved.1.id_hash_hex, minted_session.session.id_hash_hex);
}

#[tokio::test]
async fn login_rejects_unknown_or_wrong_password_uniformly() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let err = auth
        .login("ghost@example.com", "anything", &meta())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));

    let (_u, minted) = auth
        .register("user@example.com", "rightsecret")
        .await
        .expect("register");
    auth.verify_email(&minted.plaintext_token.to_wire_string())
        .await
        .expect("verify");

    let err = auth
        .login("user@example.com", "wrongsecret", &meta())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
}

#[tokio::test]
async fn login_blocks_unverified_user() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    auth.register("unv@example.com", "rightsecret")
        .await
        .expect("register");
    let err = auth
        .login("unv@example.com", "rightsecret", &meta())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::EmailNotVerified));
}

// ── session lifecycle ─────────────────────────────────────────

#[tokio::test]
async fn lookup_session_returns_none_for_bad_cookie() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let result = auth
        .lookup_session("not-a-signed-cookie")
        .await
        .expect("ok");
    assert!(result.is_none());
}

#[tokio::test]
async fn logout_invalidates_session() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, mv) = auth
        .register("out@example.com", "verysecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let (_user, minted) = auth
        .login("out@example.com", "verysecret", &meta())
        .await
        .expect("login");
    let cookie = sentori_auth_session::axum::build_session_cookie(
        &auth,
        &minted.session_id.to_wire_string(),
        minted.session.expires_at,
    );
    let cookie_value = cookie.value().to_string();

    let resolved = auth.lookup_session(&cookie_value).await.expect("ok");
    assert!(resolved.is_some());

    let id_hash = hex_decode_32(&minted.session.id_hash_hex);
    auth.logout(&id_hash).await.expect("logout");

    let resolved = auth.lookup_session(&cookie_value).await.expect("ok");
    assert!(resolved.is_none());

    let _ = user;
}

#[tokio::test]
async fn sign_out_everywhere_keeps_one() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, mv) = auth
        .register("multi@example.com", "verysecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");

    let (_u1, s1) = auth
        .login("multi@example.com", "verysecret", &meta())
        .await
        .expect("login 1");
    let (_u2, s2) = auth
        .login("multi@example.com", "verysecret", &meta())
        .await
        .expect("login 2");
    let (_u3, s3) = auth
        .login("multi@example.com", "verysecret", &meta())
        .await
        .expect("login 3");

    let keep_hash = hex_decode_32(&s2.session.id_hash_hex);
    let deleted = auth
        .sign_out_everywhere(user.id, &keep_hash)
        .await
        .expect("sign out everywhere");
    assert_eq!(deleted, 2);

    // Only s2 should survive.
    let list = auth.sessions().list_for_user(user.id).await.expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id_hash_hex, s2.session.id_hash_hex);
    let _ = (s1, s3);
}

#[tokio::test]
async fn prune_expired_collects_only_past() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, mv) = auth
        .register("prune@example.com", "verysecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let (_u, _) = auth
        .login("prune@example.com", "verysecret", &meta())
        .await
        .expect("login");
    // Force expire one via direct UPDATE.
    sqlx::query(
        "UPDATE auth_sessions SET expires_at = now() - INTERVAL '1 second' WHERE user_id = $1",
    )
    .bind(user.id.into_uuid())
    .execute(auth.raw_pool())
    .await
    .expect("force expire");

    let deleted = auth.sessions().prune_expired().await.expect("prune");
    assert!(deleted >= 1);
}

// ── password reset ────────────────────────────────────────────

#[tokio::test]
async fn forgot_password_returns_none_for_unknown_email() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let result = auth.forgot_password("ghost@x.com").await.expect("ok");
    assert!(result.is_none());
}

#[tokio::test]
async fn reset_password_flow_rotates_hash_and_drops_sessions() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);

    let (user, mv) = auth
        .register("reset@example.com", "oldsecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    seed_owner_membership(&auth, user.id).await;

    let (_user, _session) = auth
        .login("reset@example.com", "oldsecret", &meta())
        .await
        .expect("login");

    let minted = auth
        .forgot_password("reset@example.com")
        .await
        .expect("forgot")
        .expect("present");

    auth.reset_password(&minted.plaintext_token.to_wire_string(), "newsecret")
        .await
        .expect("reset");

    // Old session should be gone.
    let list = auth.sessions().list_for_user(user.id).await.expect("list");
    assert!(list.is_empty());

    // Old password no longer works; new password does.
    let err = auth
        .login("reset@example.com", "oldsecret", &meta())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
    let (_logged, _) = auth
        .login("reset@example.com", "newsecret", &meta())
        .await
        .expect("login with new");
}

#[tokio::test]
async fn reset_password_rejects_short_or_bad_token() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let (_u, mv) = auth
        .register("badreset@example.com", "oldsecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");

    let minted = auth
        .forgot_password("badreset@example.com")
        .await
        .expect("forgot")
        .expect("present");
    let wire = minted.plaintext_token.to_wire_string();

    let err = auth.reset_password(&wire, "short").await.unwrap_err();
    assert!(matches!(err, AuthError::PasswordTooShort { .. }));

    let phantom = sentori_auth_session::PasswordResetToken::generate().expect("gen");
    let err = auth
        .reset_password(&phantom.to_wire_string(), "longenough")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::TokenInvalid));
}

// ── change password ───────────────────────────────────────────

#[tokio::test]
async fn change_password_keeps_current_session_drops_others() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let (user, mv) = auth
        .register("change@example.com", "oldsecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    seed_owner_membership(&auth, user.id).await;

    let (_u1, s1) = auth
        .login("change@example.com", "oldsecret", &meta())
        .await
        .expect("login 1 (other)");
    let (_u2, s2) = auth
        .login("change@example.com", "oldsecret", &meta())
        .await
        .expect("login 2 (this)");

    let keep_hash = hex_decode_32(&s2.session.id_hash_hex);
    auth.change_password(user.id, "oldsecret", "newsecret", &keep_hash)
        .await
        .expect("change");

    let list = auth.sessions().list_for_user(user.id).await.expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id_hash_hex, s2.session.id_hash_hex);
    let _ = s1;

    // Old pwd dead, new pwd works.
    let err = auth
        .login("change@example.com", "oldsecret", &meta())
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidCredentials));
    let (_user, _) = auth
        .login("change@example.com", "newsecret", &meta())
        .await
        .expect("new pwd login");
}

#[tokio::test]
async fn change_password_rejects_wrong_current_pwd() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let (user, mv) = auth
        .register("changewrong@example.com", "oldsecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let (_u, s) = auth
        .login("changewrong@example.com", "oldsecret", &meta())
        .await
        .expect("login");
    let keep = hex_decode_32(&s.session.id_hash_hex);

    let err = auth
        .change_password(user.id, "WRONG", "newsecret", &keep)
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::CurrentPasswordWrong));
}

#[tokio::test]
async fn change_password_rejects_unknown_user() {
    let (pool, ws) = fresh_pool().await;
    let auth = service(pool, ws);
    let phantom_user = sentori_workspace_identity::UserId::new();
    let err = auth
        .change_password(phantom_user, "x", "verysecret", &[0u8; 32])
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::UserNotFound(_)));
}

// ── helpers ───────────────────────────────────────────────────

fn hex_decode_32(s: &str) -> [u8; 32] {
    assert_eq!(s.len(), 64);
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_nibble(chunk[0]).expect("hex");
        let lo = hex_nibble(chunk[1]).expect("hex");
        out[i] = (hi << 4) | lo;
    }
    out
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
