//! Integration tests for `sentori-replay-store`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_attachment_store::MemoryBlobStore;
use sentori_event_pipeline::{Event, IngestOptions, IngestService, Platform};
use sentori_replay_store::{Cursor, ReplayStore, ReplayStoreError, Scrubber};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId};
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
        include_str!("../../../migrations/0003_event_pipeline.sql"),
        include_str!("../../../migrations/0007_replay_sessions.sql"),
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

async fn seed_event(pool: &PgPool, pid: ProjectId) -> Uuid {
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let ev = Event::exception(
        Uuid::now_v7(),
        OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap(),
        Platform::Ios,
        "app@1.0.0",
        "production",
        "T",
        "boom",
    );
    svc.ingest(pid, ev).await.expect("ingest").event_id
}

fn build_store(pool: PgPool) -> ReplayStore<MemoryBlobStore> {
    ReplayStore::new(pool, MemoryBlobStore::new(), Scrubber::owasp_default())
}

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn sample_ndjson() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(br#"{"ts":1,"kind":"key","width":390,"height":844,"nodes":[]}"#);
    v.push(b'\n');
    v.extend_from_slice(
        br#"{"ts":2,"kind":"delta","added":[{"text":"hello"}],"changed":[],"removed":[]}"#,
    );
    v.push(b'\n');
    v
}

// ── store happy paths ────────────────────────────────────────

#[tokio::test]
async fn store_round_trip() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    let session = store
        .store(pid, eid, &sample_ndjson(), now(), now())
        .await
        .expect("store");
    assert_eq!(session.event_id, eid);
    assert_eq!(session.project_id, pid);
    assert_eq!(session.frame_count, 2);
    assert_eq!(session.scrubbed_count, 0); // no PII in sample
    assert!(session.byte_count > 0);
    assert!(!session.blob_hash.is_empty());

    let raw = store.fetch(session.id).await.expect("fetch");
    let raw_str = String::from_utf8(raw).expect("utf8");
    assert!(raw_str.contains("\"key\""));
    assert!(raw_str.contains("\"delta\""));
}

#[tokio::test]
async fn store_redacts_pii_on_write() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    let mut ndjson = Vec::new();
    ndjson.extend_from_slice(
        br#"{"ts":1,"kind":"key","nodes":[{"text":"contact alice@example.com"}]}"#,
    );
    ndjson.push(b'\n');
    let session = store.store(pid, eid, &ndjson, now(), now()).await.unwrap();
    assert!(session.had_pii());
    assert_eq!(session.scrubbed_count, 1);

    // fetched bytes must NOT contain the raw email.
    let raw = store.fetch(session.id).await.unwrap();
    let s = String::from_utf8(raw).unwrap();
    assert!(!s.contains("alice@example.com"));
    assert!(s.contains("[REDACTED]"));
}

#[tokio::test]
async fn store_invalid_window_rejected() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    let later = OffsetDateTime::now_utc() + time::Duration::minutes(1);
    let earlier = OffsetDateTime::now_utc();
    let err = store
        .store(pid, eid, &sample_ndjson(), later, earlier)
        .await
        .unwrap_err();
    assert!(matches!(err, ReplayStoreError::InvalidInput(_)));
}

#[tokio::test]
async fn store_unknown_event_typed_error() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let store = build_store(pool);
    let phantom_event = Uuid::now_v7();
    let err = store
        .store(pid, phantom_event, &sample_ndjson(), now(), now())
        .await
        .unwrap_err();
    // FK error — could route to EventNotFound or ProjectNotFound
    // depending on which constraint fires first. Both are typed.
    assert!(matches!(
        err,
        ReplayStoreError::EventNotFound(_) | ReplayStoreError::ProjectNotFound(_)
    ));
}

// ── fetch / find ─────────────────────────────────────────────

#[tokio::test]
async fn fetch_missing_session_errors() {
    let (pool, _ws) = fresh_pool().await;
    let store = build_store(pool);
    let err = store.fetch(Uuid::now_v7()).await.unwrap_err();
    assert!(matches!(err, ReplayStoreError::SessionNotFound(_)));
}

#[tokio::test]
async fn find_missing_returns_none() {
    let (pool, _ws) = fresh_pool().await;
    let store = build_store(pool);
    let result = store.find(Uuid::now_v7()).await.unwrap();
    assert!(result.is_none());
}

// ── list_for_event ───────────────────────────────────────────

#[tokio::test]
async fn list_for_event_returns_all_attached() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    for _ in 0..3 {
        store
            .store(pid, eid, &sample_ndjson(), now(), now())
            .await
            .unwrap();
    }
    let list = store.list_for_event(eid).await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn list_for_event_empty_for_unknown() {
    let (pool, _ws) = fresh_pool().await;
    let store = build_store(pool);
    let list = store.list_for_event(Uuid::now_v7()).await.unwrap();
    assert!(list.is_empty());
}

// ── list_for_project (cursor) ────────────────────────────────

#[tokio::test]
async fn list_for_project_cursor_paginates() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    for _ in 0..5 {
        store
            .store(pid, eid, &sample_ndjson(), now(), now())
            .await
            .unwrap();
    }
    let p1 = store.list_for_project(pid, Cursor::start(2)).await.unwrap();
    assert_eq!(p1.items.len(), 2);
    assert!(p1.next.is_some());

    let p2 = store.list_for_project(pid, p1.next.unwrap()).await.unwrap();
    assert_eq!(p2.items.len(), 2);

    let p3 = store.list_for_project(pid, p2.next.unwrap()).await.unwrap();
    assert_eq!(p3.items.len(), 1);
    assert!(p3.next.is_none());

    // No duplicates across pages.
    let mut all: Vec<Uuid> = p1
        .items
        .iter()
        .chain(p2.items.iter())
        .chain(p3.items.iter())
        .map(|s| s.id)
        .collect();
    let pre_dedup = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), pre_dedup);
}

// ── delete ───────────────────────────────────────────────────

#[tokio::test]
async fn delete_removes_metadata_but_leaves_blob() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    let session = store
        .store(pid, eid, &sample_ndjson(), now(), now())
        .await
        .unwrap();
    store.delete(session.id).await.unwrap();
    assert!(store.find(session.id).await.unwrap().is_none());
    // delete is idempotent.
    store.delete(session.id).await.unwrap();
}

#[tokio::test]
async fn event_cascade_drops_replays() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool.clone());
    let session = store
        .store(pid, eid, &sample_ndjson(), now(), now())
        .await
        .unwrap();

    // Drop the event row directly — cascade should clean up
    // replay_sessions.
    sqlx::query("DELETE FROM events WHERE id = $1")
        .bind(eid)
        .execute(&pool)
        .await
        .unwrap();
    let after = store.find(session.id).await.unwrap();
    assert!(after.is_none(), "FK cascade should drop the session row");
}

// ── dedup across sessions ────────────────────────────────────

#[tokio::test]
async fn identical_payloads_share_blob_hash() {
    let (pool, ws) = fresh_pool().await;
    let pid = seed_project(&pool, ws, "p1").await;
    let eid = seed_event(&pool, pid).await;
    let store = build_store(pool);
    let s1 = store
        .store(pid, eid, &sample_ndjson(), now(), now())
        .await
        .unwrap();
    let s2 = store
        .store(pid, eid, &sample_ndjson(), now(), now())
        .await
        .unwrap();
    assert_ne!(s1.id, s2.id, "different session ids");
    assert_eq!(
        s1.blob_hash, s2.blob_hash,
        "K3 content-address means same bytes → same blob hash"
    );
}
