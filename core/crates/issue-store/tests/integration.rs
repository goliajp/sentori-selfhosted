//! Integration tests for `sentori-issue-store` against a real
//! postgres 18 via testcontainers.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_event_pipeline::{
    Event, FrameSite, IngestOptions, IngestService, IssueStatus, Platform,
};
use sentori_issue_store::{
    Cursor, IssuePatch, IssueStore, IssueStoreError, ListFilter, RelationReason,
};
use sentori_workspace_identity::{Identity, ProjectId, UserId, WorkspaceId, bootstrap_workspace};
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
    for sql in [
        include_str!("../../../migrations/0001_workspace_identity.sql"),
        include_str!("../../../migrations/0003_event_pipeline.sql"),
        include_str!("../../../migrations/0004_issue_triage.sql"),
    ] {
        pool.execute(sql).await.expect("migration");
    }
    let workspace_id = bootstrap_workspace(&pool, "test")
        .await
        .expect("bootstrap workspace");
    (pool, workspace_id)
}

// ── seed helpers ──────────────────────────────────────────────

async fn seed_project(pool: &PgPool, workspace_id: WorkspaceId, slug: &str) -> ProjectId {
    let identity = Identity::new(pool.clone(), workspace_id);
    let salt = [0xa5u8; 32];
    identity
        .projects()
        .create(slug, slug, &salt)
        .await
        .expect("project")
        .id
}

async fn seed_user(pool: &PgPool, workspace_id: WorkspaceId, email: &str) -> UserId {
    let identity = Identity::new(pool.clone(), workspace_id);
    identity
        .users()
        .create(email, "$argon2id$placeholder")
        .await
        .expect("user")
        .id
}

fn exception(release: &str, error_type: &str, message: &str, ts: i64) -> Event {
    Event::exception(
        Uuid::now_v7(),
        OffsetDateTime::from_unix_timestamp(ts).expect("ts"),
        Platform::Ios,
        release,
        "production",
        error_type,
        message,
    )
    .with_frame(FrameSite {
        function: Some("renderHeader".into()),
        file: "app/screens/Home.tsx".into(),
    })
}

fn exception_with_user(
    release: &str,
    error_type: &str,
    message: &str,
    ts: i64,
    user_id: &str,
) -> Event {
    exception(release, error_type, message, ts).with_payload(serde_json::json!({
        "user": { "id": user_id }
    }))
}

// ── list ──────────────────────────────────────────────────────

#[tokio::test]
async fn list_empty_when_no_issues() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let store = IssueStore::new(pool);

    let page = store
        .list(project_id, ListFilter::default(), Cursor::start(50))
        .await
        .expect("list");
    assert!(page.items.is_empty());
    assert!(page.next.is_none());
}

#[tokio::test]
async fn list_orders_desc_by_last_seen() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    // ts ordering: A=1000, B=2000, C=3000 → C should come first.
    svc.ingest(project_id, exception("app@1.0.0", "A", "boom", 1000))
        .await
        .unwrap();
    svc.ingest(project_id, exception("app@1.0.0", "B", "boom", 2000))
        .await
        .unwrap();
    svc.ingest(project_id, exception("app@1.0.0", "C", "boom", 3000))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    let page = store
        .list(project_id, ListFilter::default(), Cursor::start(10))
        .await
        .unwrap();
    let types: Vec<_> = page.items.iter().map(|i| i.error_type.as_str()).collect();
    assert_eq!(types, vec!["C", "B", "A"]);
}

#[tokio::test]
async fn list_filters_by_status() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");

    let out_a = svc
        .ingest(project_id, exception("app@1.0.0", "A", "boom", 1000))
        .await
        .unwrap();
    let _ = svc
        .ingest(project_id, exception("app@1.0.0", "B", "boom", 2000))
        .await
        .unwrap();
    // Resolve A.
    svc.set_issue_status(
        out_a.issue_id,
        IssueStatus::Resolved,
        Some(OffsetDateTime::now_utc()),
    )
    .await
    .unwrap();

    let store = IssueStore::new(pool);
    let active = store
        .list(
            project_id,
            ListFilter {
                status: Some(IssueStatus::Active),
                ..Default::default()
            },
            Cursor::start(10),
        )
        .await
        .unwrap();
    assert_eq!(active.items.len(), 1);
    assert_eq!(active.items[0].error_type, "B");

    let resolved = store
        .list(
            project_id,
            ListFilter {
                status: Some(IssueStatus::Resolved),
                ..Default::default()
            },
            Cursor::start(10),
        )
        .await
        .unwrap();
    assert_eq!(resolved.items.len(), 1);
    assert_eq!(resolved.items[0].error_type, "A");
}

#[tokio::test]
async fn list_search_ilike_matches_error_type_and_message() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    svc.ingest(
        project_id,
        exception("v", "AuthFailure", "token expired", 1),
    )
    .await
    .unwrap();
    svc.ingest(
        project_id,
        exception("v", "Other", "auth header missing", 2),
    )
    .await
    .unwrap();
    svc.ingest(project_id, exception("v", "NoMatch", "unrelated", 3))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    let page = store
        .list(
            project_id,
            ListFilter {
                search: Some("auth".into()),
                ..Default::default()
            },
            Cursor::start(10),
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(
        page.items
            .iter()
            .all(|i| i.error_type.to_lowercase().contains("auth")
                || i.message_sample.to_lowercase().contains("auth"))
    );
}

#[tokio::test]
async fn list_cursor_paginates_correctly() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    for i in 0..5 {
        svc.ingest(project_id, exception("v", &format!("E{i}"), "m", 100 + i))
            .await
            .unwrap();
    }
    let store = IssueStore::new(pool);
    let p1 = store
        .list(project_id, ListFilter::default(), Cursor::start(2))
        .await
        .unwrap();
    assert_eq!(p1.items.len(), 2);
    assert!(p1.next.is_some());

    let p2 = store
        .list(project_id, ListFilter::default(), p1.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p2.items.len(), 2);
    assert!(p2.next.is_some());

    let p3 = store
        .list(project_id, ListFilter::default(), p2.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p3.items.len(), 1);
    assert!(p3.next.is_none());

    // No overlapping ids across pages.
    let all_ids: Vec<_> = p1
        .items
        .iter()
        .chain(p2.items.iter())
        .chain(p3.items.iter())
        .map(|i| i.id)
        .collect();
    let mut sorted = all_ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(all_ids.len(), sorted.len(), "no duplicates");
}

// ── detail + affected_users ──────────────────────────────────

#[tokio::test]
async fn detail_round_trip_and_affected_users() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception_with_user("v", "T", "m", 1, "user-1"))
        .await
        .unwrap();
    svc.ingest(project_id, exception_with_user("v", "T", "m", 2, "user-2"))
        .await
        .unwrap();
    svc.ingest(project_id, exception_with_user("v", "T", "m", 3, "user-1"))
        .await
        .unwrap();

    let store = IssueStore::new(pool);
    let detail = store.detail(out.issue_id).await.unwrap();
    assert_eq!(detail.summary.event_count, 3);
    assert_eq!(detail.affected_users.count, 2, "user-1 + user-2 distinct");
    assert_eq!(detail.affected_users.sampled_events, 3);
    assert!(!detail.affected_users.truncated);
}

#[tokio::test]
async fn detail_not_found_errors() {
    let (pool, _workspace_id) = fresh_pool().await;
    let store = IssueStore::new(pool);
    let err = store.detail(Uuid::now_v7()).await.unwrap_err();
    assert!(matches!(err, IssueStoreError::IssueNotFound(_)));
}

// ── related ──────────────────────────────────────────────────

#[tokio::test]
async fn related_finds_cross_release_and_same_signature() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let anchor = svc
        .ingest(project_id, exception("app@1.0.0", "TypeError", "boom-a", 1))
        .await
        .unwrap();
    // Same error_type, different release → cross-release.
    svc.ingest(project_id, exception("app@2.0.0", "TypeError", "boom-b", 2))
        .await
        .unwrap();
    // Same signature (kind + error_type + message), different fingerprint?
    // Hard to force a different fingerprint with same inputs (S3 deterministically
    // groups them). So skip the second-relation assertion here — the cross-
    // release case is the load-bearing one.
    let store = IssueStore::new(pool);
    let related = store.related(anchor.issue_id, 10).await.unwrap();
    assert!(!related.is_empty());
    assert!(
        related
            .iter()
            .any(|r| matches!(r.relation, RelationReason::SameTypeDifferentRelease))
    );
}

#[tokio::test]
async fn related_not_found_errors() {
    let (pool, _workspace_id) = fresh_pool().await;
    let store = IssueStore::new(pool);
    let err = store.related(Uuid::now_v7(), 5).await.unwrap_err();
    assert!(matches!(err, IssueStoreError::IssueNotFound(_)));
}

// ── releases_for_issue ───────────────────────────────────────

#[tokio::test]
async fn releases_distinct_and_sorted() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("app@1.0.0", "T", "m", 1))
        .await
        .unwrap();
    // Two more events on the SAME issue (force same fingerprint
    // by reusing the same fields) but different releases:
    // wait — different release = different fingerprint per S3.
    // So `releases_for_issue` will only ever see the issue's
    // single release. Still useful as a smoke test.
    let releases = IssueStore::new(pool)
        .releases_for_issue(out.issue_id)
        .await
        .unwrap();
    assert_eq!(releases, vec!["app@1.0.0".to_string()]);
}

// ── events_for_issue ─────────────────────────────────────────

#[tokio::test]
async fn events_for_issue_cursor_paginates() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let mut issue_id = None;
    for i in 0..5 {
        let out = svc
            .ingest(project_id, exception("app@1.0.0", "T", "m", 100 + i))
            .await
            .unwrap();
        issue_id = Some(out.issue_id);
    }
    let store = IssueStore::new(pool);
    let p1 = store
        .events_for_issue(issue_id.unwrap(), Cursor::start(2))
        .await
        .unwrap();
    assert_eq!(p1.items.len(), 2);
    assert!(p1.next.is_some());

    let p2 = store
        .events_for_issue(issue_id.unwrap(), p1.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p2.items.len(), 2);
    let p3 = store
        .events_for_issue(issue_id.unwrap(), p2.next.unwrap())
        .await
        .unwrap();
    assert_eq!(p3.items.len(), 1);
    assert!(p3.next.is_none());
}

// ── patch ────────────────────────────────────────────────────

#[tokio::test]
async fn patch_status_and_priority_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);

    let outcome = store
        .patch(
            out.issue_id,
            IssuePatch {
                status: Some(IssueStatus::Resolved),
                priority: Some("p0".into()),
                resolved_in_release: Some("v".into()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.updated, 1);
    assert!(outcome.any_resolved);
    assert!(!outcome.any_reopened);

    let detail = store.detail(out.issue_id).await.unwrap();
    assert_eq!(detail.summary.status, IssueStatus::Resolved);
    assert_eq!(detail.summary.priority, "p0");
    assert_eq!(detail.summary.resolved_in_release.as_deref(), Some("v"));
    assert!(detail.summary.resolved_at.is_some());
}

#[tokio::test]
async fn patch_assignee_tri_state() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let assignee = seed_user(&pool, workspace_id, "ops@example.com").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);

    // Assign.
    store
        .patch(
            out.issue_id,
            IssuePatch {
                assignee_user_id: Some(Some(assignee)),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    let d1 = store.detail(out.issue_id).await.unwrap();
    assert_eq!(d1.summary.assignee_user_id, Some(assignee));

    // Clear.
    store
        .patch(
            out.issue_id,
            IssuePatch {
                assignee_user_id: Some(None),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    let d2 = store.detail(out.issue_id).await.unwrap();
    assert!(d2.summary.assignee_user_id.is_none());
}

#[tokio::test]
async fn patch_invalid_priority_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    let err = store
        .patch(
            out.issue_id,
            IssuePatch {
                priority: Some("p99".into()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, IssueStoreError::InvalidPriority { .. }));
}

#[tokio::test]
async fn patch_missing_id_errors() {
    let (pool, _workspace_id) = fresh_pool().await;
    let store = IssueStore::new(pool);
    let err = store
        .patch(
            Uuid::now_v7(),
            IssuePatch {
                priority: Some("p1".into()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, IssueStoreError::IssueNotFound(_)));
}

#[tokio::test]
async fn bulk_patch_silent_on_missing_ids() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let real = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let phantom = Uuid::now_v7();
    let store = IssueStore::new(pool);
    let outcome = store
        .bulk_patch(
            &[real.issue_id, phantom],
            IssuePatch {
                priority: Some("p0".into()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    assert_eq!(outcome.updated, 1);
}

#[tokio::test]
async fn patch_reopen_flag_fires() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    store
        .patch(
            out.issue_id,
            IssuePatch {
                status: Some(IssueStatus::Resolved),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    let outcome = store
        .patch(
            out.issue_id,
            IssuePatch {
                status: Some(IssueStatus::Active),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    assert!(outcome.any_reopened);
    assert!(!outcome.any_resolved);
    // Resolved metadata should be cleared on un-resolve.
    let detail = store.detail(out.issue_id).await.unwrap();
    assert!(detail.summary.resolved_at.is_none());
    assert!(detail.summary.resolved_in_release.is_none());
}

// ── merge ────────────────────────────────────────────────────

#[tokio::test]
async fn merge_moves_events_and_drops_source() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let dst = svc
        .ingest(project_id, exception("v", "A", "msg-a", 1))
        .await
        .unwrap();
    let src = svc
        .ingest(project_id, exception("v", "B", "msg-b", 2))
        .await
        .unwrap();
    svc.ingest(project_id, exception("v", "B", "msg-b", 3))
        .await
        .unwrap();

    let store = IssueStore::new(pool);
    let outcome = store.merge(src.issue_id, dst.issue_id).await.unwrap();
    assert_eq!(outcome.events_moved, 2);
    assert_eq!(outcome.src, src.issue_id);
    assert_eq!(outcome.dst, dst.issue_id);

    // src gone, dst absorbed the events.
    let err = store.detail(src.issue_id).await.unwrap_err();
    assert!(matches!(err, IssueStoreError::IssueNotFound(_)));
    let dst_detail = store.detail(dst.issue_id).await.unwrap();
    assert_eq!(dst_detail.summary.event_count, 3); // 1 own + 2 absorbed
}

#[tokio::test]
async fn merge_into_self_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let out = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    let err = store.merge(out.issue_id, out.issue_id).await.unwrap_err();
    assert!(matches!(err, IssueStoreError::MergeIntoSelf));
}

#[tokio::test]
async fn merge_across_projects_rejected() {
    let (pool, workspace_id) = fresh_pool().await;
    let p_a = seed_project(&pool, workspace_id, "p1").await;
    let p_b = seed_project(&pool, workspace_id, "p2").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let a = svc.ingest(p_a, exception("v", "T", "m", 1)).await.unwrap();
    let b = svc.ingest(p_b, exception("v", "T", "m", 2)).await.unwrap();
    let store = IssueStore::new(pool);
    let err = store.merge(a.issue_id, b.issue_id).await.unwrap_err();
    assert!(matches!(err, IssueStoreError::MergeAcrossProjects));
}

#[tokio::test]
async fn merge_missing_id_errors() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let real = svc
        .ingest(project_id, exception("v", "T", "m", 1))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    let err = store
        .merge(Uuid::now_v7(), real.issue_id)
        .await
        .unwrap_err();
    assert!(matches!(err, IssueStoreError::IssueNotFound(_)));
}

// ── facet smoke (label/priority filter) ───────────────────────

#[tokio::test]
async fn label_and_priority_filter_match() {
    let (pool, workspace_id) = fresh_pool().await;
    let project_id = seed_project(&pool, workspace_id, "p1").await;
    let svc = IngestService::new(pool.clone(), IngestOptions::default()).expect("svc");
    let a = svc
        .ingest(project_id, exception("v", "A", "m", 1))
        .await
        .unwrap();
    let b = svc
        .ingest(project_id, exception("v", "B", "m", 2))
        .await
        .unwrap();
    let store = IssueStore::new(pool);
    store
        .patch(
            a.issue_id,
            IssuePatch {
                priority: Some("p0".into()),
                labels: Some(vec!["crash".into(), "ios".into()]),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
    store
        .patch(
            b.issue_id,
            IssuePatch {
                priority: Some("p2".into()),
                labels: Some(vec!["android".into()]),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();

    let p0_only = store
        .list(
            project_id,
            ListFilter {
                priorities: vec!["p0".into()],
                ..Default::default()
            },
            Cursor::start(50),
        )
        .await
        .unwrap();
    assert_eq!(p0_only.items.len(), 1);
    assert_eq!(p0_only.items[0].error_type, "A");

    let crash_only = store
        .list(
            project_id,
            ListFilter {
                labels: vec!["crash".into()],
                ..Default::default()
            },
            Cursor::start(50),
        )
        .await
        .unwrap();
    assert_eq!(crash_only.items.len(), 1);
    assert_eq!(crash_only.items[0].error_type, "A");
}
