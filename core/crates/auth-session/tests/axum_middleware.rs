//! End-to-end test for the [`sentori_auth_session::axum::require_user`]
//! middleware against a real axum Router (driven via tower).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use axum::{Extension, Router, body::Body, middleware, response::IntoResponse, routing::get};
use http::{Request, StatusCode, header};
use http_body_util::BodyExt as _;
use sentori_auth_session::{
    AuthOptions, AuthService, RequestMeta,
    axum::{CurrentUser, build_session_cookie, require_user},
};
use sentori_cookie_session::SecretKey;
use sentori_workspace_identity::{Identity, WorkspaceId};
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::sync::Mutex;
use tower::ServiceExt as _;
use uuid::Uuid;

// ── shared container (duplicated from integration.rs because
//   integration_test binaries don't share modules) ──────────────

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
    pool.execute(include_str!("../../../migrations/0002_auth_session.sql"))
        .await
        .expect("0002");
    let workspace_id = sentori_workspace_identity::bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

// ── small test router ─────────────────────────────────────────

async fn me_handler(Extension(user): Extension<CurrentUser>) -> impl IntoResponse {
    user.email
}

async fn anon_handler() -> &'static str {
    "anon"
}

fn build_router(auth: AuthService) -> Router {
    let protected = Router::new()
        .route("/me", get(me_handler))
        .route_layer(middleware::from_fn_with_state(auth.clone(), require_user));
    let public = Router::new().route("/anon", get(anon_handler));
    Router::new()
        .merge(protected)
        .merge(public)
        .with_state(auth)
}

async fn opts_with_insecure_cookie() -> AuthService {
    // tower::ServiceExt::oneshot serves HTTP without TLS, so
    // browser-equivalent Secure cookies would never round-trip.
    // Tests opt out of Secure to drive the full flow.
    let opts = AuthOptions {
        cookie_secure: false,
        ..AuthOptions::default()
    };
    let key = SecretKey::generate().expect("rng");
    let (pool, ws) = fresh_pool().await;
    AuthService::new(Identity::new(pool, ws), key, opts)
}

async fn body_text(resp: axum::response::Response) -> String {
    String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .expect("collect")
            .to_bytes()
            .to_vec(),
    )
    .expect("utf8")
}

// ── tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn anonymous_route_is_reachable_without_cookie() {
    let auth = opts_with_insecure_cookie().await;
    let router = build_router(auth);

    let resp = router
        .oneshot(Request::builder().uri("/anon").body(Body::empty()).unwrap())
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "anon");
}

#[tokio::test]
async fn protected_route_401s_without_cookie() {
    let auth = opts_with_insecure_cookie().await;
    let router = build_router(auth);

    let resp = router
        .oneshot(Request::builder().uri("/me").body(Body::empty()).unwrap())
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_401s_with_garbage_cookie() {
    let auth = opts_with_insecure_cookie().await;
    let router = build_router(auth);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/me")
                .header(header::COOKIE, "sentori_session=garbage")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_user_with_valid_cookie() {
    let auth = opts_with_insecure_cookie().await;
    // Provision + login flow.
    let (_user, mv) = auth
        .register("axum@example.com", "verysecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let (_user2, minted) = auth
        .login("axum@example.com", "verysecret", &RequestMeta::default())
        .await
        .expect("login");

    let cookie = build_session_cookie(
        &auth,
        &minted.session_id.to_wire_string(),
        minted.session.expires_at,
    );
    let cookie_header = format!("{}={}", cookie.name(), cookie.value());

    let router = build_router(auth);
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/me")
                .header(header::COOKIE, cookie_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_text(resp).await, "axum@example.com");
}

#[tokio::test]
async fn protected_route_401s_after_logout() {
    let auth = opts_with_insecure_cookie().await;
    let (_user, mv) = auth
        .register("logout@example.com", "verysecret")
        .await
        .expect("reg");
    auth.verify_email(&mv.plaintext_token.to_wire_string())
        .await
        .expect("verify");
    let (_user2, minted) = auth
        .login("logout@example.com", "verysecret", &RequestMeta::default())
        .await
        .expect("login");

    let cookie = build_session_cookie(
        &auth,
        &minted.session_id.to_wire_string(),
        minted.session.expires_at,
    );
    let cookie_header = format!("{}={}", cookie.name(), cookie.value());
    let id_hash = hex_decode_32(&minted.session.id_hash_hex);
    auth.logout(&id_hash).await.expect("logout");

    let router = build_router(auth);
    let resp = router
        .oneshot(
            Request::builder()
                .uri("/me")
                .header(header::COOKIE, cookie_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

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
