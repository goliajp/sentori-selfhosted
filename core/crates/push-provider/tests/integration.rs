//! Integration tests for `sentori-push-provider`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::{Arc, OnceLock};

use sentori_push_provider::{
    DispatchTarget, MockProvider, NativeMessage, PerTokenOutcome, ProviderKind, ProviderRegistry,
    PushDispatcher, PushError, RateLimits, SendOutcome,
};
use sentori_secrets_vault::{KeyId, MasterKey, Vault};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
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
        include_str!("../../../migrations/0006_push_tokens.sql"),
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

fn vault() -> Vault {
    Vault::new(
        MasterKey::generate().expect("rng"),
        KeyId::new("k1").expect("kid"),
    )
}

fn build_dispatcher(pool: PgPool, mock: Arc<MockProvider>, kind: ProviderKind) -> PushDispatcher {
    let mut reg = ProviderRegistry::new();
    reg.register(kind, mock);
    PushDispatcher::new(pool, reg, vault(), RateLimits::default())
}

async fn seed_credential(disp: &PushDispatcher, pid: ProjectId, kind: ProviderKind) {
    disp.credentials()
        .upsert(
            pid,
            kind,
            &serde_json::json!({ "vendor_project_id": "demo" }),
            b"sealed-secret-bytes-go-here",
        )
        .await
        .expect("seed cred");
}

async fn seed_token(
    disp: &PushDispatcher,
    pid: ProjectId,
    kind: ProviderKind,
    native: &str,
    app_user: Option<&str>,
) -> Uuid {
    disp.tokens()
        .upsert(pid, kind, native, None, app_user)
        .await
        .expect("upsert token")
        .id
}

// ── tokens CRUD ──────────────────────────────────────────────

#[tokio::test]
async fn token_upsert_idempotent() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    let first = disp
        .tokens()
        .upsert(
            pid,
            ProviderKind::Apns,
            "DEVICE-TOKEN",
            None,
            Some("user-1"),
        )
        .await
        .unwrap();
    assert!(first.is_new);
    let second = disp
        .tokens()
        .upsert(
            pid,
            ProviderKind::Apns,
            "DEVICE-TOKEN",
            None,
            Some("user-1"),
        )
        .await
        .unwrap();
    assert!(!second.is_new);
    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn token_upsert_rejects_empty() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    let err = disp
        .tokens()
        .upsert(pid, ProviderKind::Apns, "  ", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, PushError::InvalidInput(_)));
}

#[tokio::test]
async fn token_upsert_unknown_project_fk() {
    let (pool, _ws) = fresh_pool().await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    let err = disp
        .tokens()
        .upsert(ProjectId::new(), ProviderKind::Apns, "X", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, PushError::ProjectNotFound(_)));
}

#[tokio::test]
async fn token_quarantine_skips_from_live_list() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    let id = seed_token(&disp, pid, ProviderKind::Apns, "T1", None).await;
    disp.tokens().quarantine(id, "manual").await.unwrap();
    let live = disp
        .tokens()
        .list_live(pid, ProviderKind::Apns)
        .await
        .unwrap();
    assert!(live.is_empty());
    let row = disp.tokens().find(id).await.unwrap().unwrap();
    assert!(row.is_quarantined());
    assert_eq!(row.quarantine_reason.as_deref(), Some("manual"));
}

#[tokio::test]
async fn token_upsert_clears_quarantine() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    let id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    disp.tokens().quarantine(id, "stale").await.unwrap();
    let upd = disp
        .tokens()
        .upsert(pid, ProviderKind::Apns, "T", None, None)
        .await
        .unwrap();
    assert_eq!(upd.id, id);
    let row = disp.tokens().find(id).await.unwrap().unwrap();
    assert!(!row.is_quarantined());
}

#[tokio::test]
async fn list_for_user_returns_only_user_tokens() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    seed_token(&disp, pid, ProviderKind::Apns, "U1-iphone", Some("user-1")).await;
    seed_token(&disp, pid, ProviderKind::Fcm, "U1-android", Some("user-1")).await;
    seed_token(&disp, pid, ProviderKind::Apns, "U2-iphone", Some("user-2")).await;

    let u1 = disp.tokens().list_for_user(pid, "user-1").await.unwrap();
    assert_eq!(u1.len(), 2);
    assert!(u1.iter().any(|t| t.kind == ProviderKind::Apns));
    assert!(u1.iter().any(|t| t.kind == ProviderKind::Fcm));
}

// ── credentials ──────────────────────────────────────────────

#[tokio::test]
async fn credentials_round_trip_seal_unseal() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    disp.credentials()
        .upsert(
            pid,
            ProviderKind::Apns,
            &serde_json::json!({ "key_id": "ABC123" }),
            b"this-is-the-p8-bytes",
        )
        .await
        .unwrap();
    let loaded = disp
        .credentials()
        .load(pid, ProviderKind::Apns)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.secret_payload, b"this-is-the-p8-bytes");
    assert_eq!(loaded.config["key_id"], "ABC123");
}

#[tokio::test]
async fn credentials_missing_returns_none() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let disp = build_dispatcher(
        pool,
        Arc::new(MockProvider::always(SendOutcome::Sent)),
        ProviderKind::Apns,
    );
    assert!(
        disp.credentials()
            .load(pid, ProviderKind::Apns)
            .await
            .unwrap()
            .is_none()
    );
}

// ── dispatch happy paths ─────────────────────────────────────

#[tokio::test]
async fn dispatch_single_token_calls_provider_once() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let disp = build_dispatcher(pool, mock.clone(), ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    let token_id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    let out = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("Hi", "Body"),
        )
        .await
        .unwrap();
    assert_eq!(out.targeted, 1);
    assert_eq!(out.sent, 1);
    assert_eq!(mock.send_calls(), 1);
}

#[tokio::test]
async fn dispatch_project_kind_fans_out() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let disp = build_dispatcher(pool, mock.clone(), ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    for n in 0..3 {
        seed_token(&disp, pid, ProviderKind::Apns, &format!("T{n}"), None).await;
    }
    let out = disp
        .dispatch(
            DispatchTarget::ProjectKind {
                project_id: pid,
                kind: ProviderKind::Apns,
            },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap();
    assert_eq!(out.targeted, 3);
    assert_eq!(out.sent, 3);
    assert_eq!(mock.send_calls(), 3);
}

#[tokio::test]
async fn dispatch_project_user_fans_across_providers() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let apns = Arc::new(MockProvider::new(
        ProviderKind::Apns,
        SendOutcome::Sent,
        "MOCK_APNS_OK",
    ));
    let fcm = Arc::new(MockProvider::new(
        ProviderKind::Fcm,
        SendOutcome::Sent,
        "MOCK_FCM_OK",
    ));
    let mut reg = ProviderRegistry::new();
    reg.register(ProviderKind::Apns, apns.clone());
    reg.register(ProviderKind::Fcm, fcm.clone());
    let disp = PushDispatcher::new(pool, reg, vault(), RateLimits::default());
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    seed_credential(&disp, pid, ProviderKind::Fcm).await;
    seed_token(&disp, pid, ProviderKind::Apns, "iphone", Some("u1")).await;
    seed_token(&disp, pid, ProviderKind::Fcm, "android", Some("u1")).await;

    let out = disp
        .dispatch(
            DispatchTarget::ProjectUser {
                project_id: pid,
                app_user_id: "u1".into(),
            },
            NativeMessage::simple("x", "y"),
        )
        .await
        .unwrap();
    assert_eq!(out.targeted, 2);
    assert_eq!(out.sent, 2);
    assert_eq!(apns.send_calls(), 1);
    assert_eq!(fcm.send_calls(), 1);
}

// ── dispatch error paths ─────────────────────────────────────

#[tokio::test]
async fn dispatch_missing_credentials_errors() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let disp = build_dispatcher(pool, mock, ProviderKind::Apns);
    let token_id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    let err = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PushError::CredentialsMissing { .. }));
}

#[tokio::test]
async fn dispatch_unregistered_provider_errors() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    // Register APNs but seed a Fcm token.
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let disp = build_dispatcher(pool, mock, ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Fcm).await;
    let token_id = seed_token(&disp, pid, ProviderKind::Fcm, "T", None).await;
    let err = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PushError::ProviderNotRegistered(_)));
}

#[tokio::test]
async fn dispatch_single_token_quarantined_errors() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let disp = build_dispatcher(pool, mock, ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    let token_id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    disp.tokens().quarantine(token_id, "manual").await.unwrap();
    let err = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, PushError::TokenNotFound(_)));
}

// ── quarantine on send outcome ───────────────────────────────

#[tokio::test]
async fn permanently_invalid_quarantines_row() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::PermanentlyInvalidToken));
    let disp = build_dispatcher(pool, mock.clone(), ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    let token_id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    let out = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap();
    assert_eq!(out.sent, 0);
    assert_eq!(out.newly_quarantined(), 1);
    let row = disp.tokens().find(token_id).await.unwrap().unwrap();
    assert!(row.is_quarantined());
    assert!(
        row.quarantine_reason
            .as_deref()
            .unwrap_or("")
            .contains("PermanentlyInvalidToken")
    );
    // Token now appears as quarantined; subsequent dispatch
    // skips it (TokenNotFound for SingleToken; nothing in
    // ProjectKind list).
    let live = disp
        .tokens()
        .list_live(pid, ProviderKind::Apns)
        .await
        .unwrap();
    assert!(live.is_empty());
}

#[tokio::test]
async fn transient_outcome_does_not_quarantine() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Transient {
        retry_after_secs: Some(30),
    }));
    let disp = build_dispatcher(pool, mock, ProviderKind::Apns);
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    let token_id = seed_token(&disp, pid, ProviderKind::Apns, "T", None).await;
    let _ = disp
        .dispatch(
            DispatchTarget::SingleToken { token_id },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap();
    let row = disp.tokens().find(token_id).await.unwrap().unwrap();
    assert!(!row.is_quarantined());
}

// ── rate-limit smoke ────────────────────────────────────────

#[tokio::test]
async fn rate_limit_skips_after_burst() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let mock = Arc::new(MockProvider::always(SendOutcome::Sent));
    let mut reg = ProviderRegistry::new();
    reg.register(ProviderKind::Apns, mock.clone());
    // Tight L1: 2/min — third dispatch in the same minute
    // must hit the limiter.
    let disp = PushDispatcher::new(
        pool,
        reg,
        vault(),
        RateLimits {
            sends_per_minute_per_project_provider: 2,
        },
    );
    seed_credential(&disp, pid, ProviderKind::Apns).await;
    for n in 0..3 {
        seed_token(&disp, pid, ProviderKind::Apns, &format!("T{n}"), None).await;
    }
    let out = disp
        .dispatch(
            DispatchTarget::ProjectKind {
                project_id: pid,
                kind: ProviderKind::Apns,
            },
            NativeMessage::simple("a", "b"),
        )
        .await
        .unwrap();
    assert_eq!(out.targeted, 3);
    assert_eq!(out.sent, 2);
    assert_eq!(out.skipped_rate_limited, 1);
    assert_eq!(mock.send_calls(), 2);
    // Verify per_token outcomes match.
    let limited = out
        .per_token
        .iter()
        .filter(|o| matches!(o, PerTokenOutcome::SkippedRateLimited { .. }))
        .count();
    assert_eq!(limited, 1);
}
