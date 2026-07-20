//! Integration tests for `sentori-alert-rule`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

use std::sync::OnceLock;

use sentori_alert_rule::{
    AlertRuleDraft, AlertRuleError, AlertRulePatch, AlertRuleService, EventContext, TriggerKind,
};
use sentori_workspace_identity::{Identity, ProjectId, WorkspaceId, bootstrap_workspace};
use serde_json::json;
use sqlx::{Executor, PgPool};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use time::OffsetDateTime;
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
        include_str!("../../../migrations/0013_alert_rules.sql"),
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

fn ctx_for(project_id: ProjectId, regression: bool) -> EventContext {
    EventContext {
        project_id,
        issue_id: Uuid::now_v7(),
        error_type: "TypeError".into(),
        environment: "production".into(),
        release: "app@1.0.0".into(),
        is_regression: regression,
    }
}

// ── create / find / list ────────────────────────────────────

#[tokio::test]
async fn create_and_find_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "New issues", TriggerKind::NewIssue)
                .for_project(pid)
                .with_channels(json!([{"type": "email", "to": ["a@b.com"]}]))
                .with_throttle(15),
        )
        .await
        .unwrap();
    let r = svc.find(id).await.unwrap().unwrap();
    assert_eq!(r.name, "New issues");
    assert_eq!(r.trigger_kind, TriggerKind::NewIssue);
    assert_eq!(r.throttle_minutes, 15);
    assert_eq!(r.project_id, Some(pid));
    assert!(r.enabled);
    assert!(!r.muted);
}

#[tokio::test]
async fn create_workspace_wide() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "global",
            TriggerKind::Regression,
        ))
        .await
        .unwrap();
    let r = svc.find(id).await.unwrap().unwrap();
    assert!(r.project_id.is_none());
}

#[tokio::test]
async fn create_rejects_empty_name() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let err = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "  ",
            TriggerKind::NewIssue,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, AlertRuleError::InvalidInput(_)));
}

#[tokio::test]
async fn create_rejects_non_array_channels() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let err = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "x", TriggerKind::NewIssue)
                .with_channels(json!({"oops": true})),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AlertRuleError::InvalidInput(_)));
}

#[tokio::test]
async fn create_unknown_project_fk() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let err = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "x", TriggerKind::NewIssue)
                .for_project(ProjectId::new()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AlertRuleError::ProjectNotFound(_)));
}

#[tokio::test]
async fn list_for_project_includes_workspace_wide() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "scoped", TriggerKind::NewIssue).for_project(pid),
    )
    .await
    .unwrap();
    svc.create_rule(AlertRuleDraft::new(
        workspace_id,
        "global",
        TriggerKind::NewIssue,
    ))
    .await
    .unwrap();
    let list = svc.list_for_project(pid).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn list_workspace_wide_excludes_scoped() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "scoped", TriggerKind::NewIssue).for_project(pid),
    )
    .await
    .unwrap();
    svc.create_rule(AlertRuleDraft::new(
        workspace_id,
        "global",
        TriggerKind::NewIssue,
    ))
    .await
    .unwrap();
    let list = svc.list_workspace_wide().await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].project_id.is_none());
}

#[tokio::test]
async fn list_active_by_kind_filters() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let enabled = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "ok",
            TriggerKind::EventCount,
        ))
        .await
        .unwrap();
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "disabled", TriggerKind::EventCount).disabled(),
    )
    .await
    .unwrap();
    let muted = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "muted",
            TriggerKind::EventCount,
        ))
        .await
        .unwrap();
    svc.set_muted(muted, true).await.unwrap();
    svc.create_rule(AlertRuleDraft::new(
        workspace_id,
        "wrong-kind",
        TriggerKind::NewIssue,
    ))
    .await
    .unwrap();

    let active = svc
        .list_active_by_kind(TriggerKind::EventCount)
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, enabled);
}

// ── update / set_enabled / set_muted / snooze / delete ──────

#[tokio::test]
async fn update_patches_only_set_fields() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "orig", TriggerKind::NewIssue).with_throttle(10),
        )
        .await
        .unwrap();
    svc.update(
        id,
        AlertRulePatch {
            name: Some("renamed".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let r = svc.find(id).await.unwrap().unwrap();
    assert_eq!(r.name, "renamed");
    assert_eq!(r.throttle_minutes, 10, "throttle unchanged");
}

#[tokio::test]
async fn update_unknown_rule_errors() {
    let (pool, _workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let err = svc
        .update(Uuid::now_v7(), AlertRulePatch::default())
        .await
        .unwrap_err();
    assert!(matches!(err, AlertRuleError::RuleNotFound(_)));
}

#[tokio::test]
async fn set_enabled_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "x",
            TriggerKind::NewIssue,
        ))
        .await
        .unwrap();
    svc.set_enabled(id, false).await.unwrap();
    assert!(!svc.find(id).await.unwrap().unwrap().enabled);
    svc.set_enabled(id, true).await.unwrap();
    assert!(svc.find(id).await.unwrap().unwrap().enabled);
}

#[tokio::test]
async fn snooze_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "x",
            TriggerKind::NewIssue,
        ))
        .await
        .unwrap();
    let until = OffsetDateTime::now_utc() + time::Duration::hours(1);
    svc.snooze(id, Some(until)).await.unwrap();
    assert!(svc.find(id).await.unwrap().unwrap().snoozed_until.is_some());
    svc.snooze(id, None).await.unwrap();
    assert!(svc.find(id).await.unwrap().unwrap().snoozed_until.is_none());
}

#[tokio::test]
async fn delete_round_trip() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(AlertRuleDraft::new(
            workspace_id,
            "x",
            TriggerKind::NewIssue,
        ))
        .await
        .unwrap();
    svc.delete(id).await.unwrap();
    assert!(svc.find(id).await.unwrap().is_none());
    // Idempotent.
    svc.delete(id).await.unwrap();
}

// ── try_fire_for_event ──────────────────────────────────────

#[tokio::test]
async fn try_fire_returns_matched_rules() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "project-scoped", TriggerKind::NewIssue).for_project(pid),
    )
    .await
    .unwrap();
    svc.create_rule(AlertRuleDraft::new(
        workspace_id,
        "global",
        TriggerKind::NewIssue,
    ))
    .await
    .unwrap();

    let matched = svc.try_fire_for_event(&ctx_for(pid, false)).await.unwrap();
    assert_eq!(matched.len(), 2);
    for m in &matched {
        assert!(m.summary.contains("TypeError"));
        assert!(m.body.contains("Trigger: new_issue"));
        assert!(m.rule.last_fired_at.is_some());
    }
}

#[tokio::test]
async fn try_fire_filters_environment() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "prod-only", TriggerKind::NewIssue)
            .for_project(pid)
            .with_filter(json!({"environment": "staging"})),
    )
    .await
    .unwrap();
    let matched = svc.try_fire_for_event(&ctx_for(pid, false)).await.unwrap();
    assert!(matched.is_empty(), "filter prevents match");
}

#[tokio::test]
async fn try_fire_skips_disabled_muted_snoozed() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    let disabled = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "dis", TriggerKind::NewIssue)
                .for_project(pid)
                .disabled(),
        )
        .await
        .unwrap();
    let muted = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "mut", TriggerKind::NewIssue).for_project(pid),
        )
        .await
        .unwrap();
    svc.set_muted(muted, true).await.unwrap();
    let snoozed = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "snz", TriggerKind::NewIssue).for_project(pid),
        )
        .await
        .unwrap();
    svc.snooze(
        snoozed,
        Some(OffsetDateTime::now_utc() + time::Duration::hours(1)),
    )
    .await
    .unwrap();
    let matched = svc.try_fire_for_event(&ctx_for(pid, false)).await.unwrap();
    assert!(matched.is_empty(), "all three silenced");
    // sanity: ids exist + not fired.
    for id in [disabled, muted, snoozed] {
        assert!(svc.find(id).await.unwrap().unwrap().last_fired_at.is_none());
    }
}

#[tokio::test]
async fn try_fire_regression_only_matches_regression_kind() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "reg", TriggerKind::Regression).for_project(pid),
    )
    .await
    .unwrap();
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "ni", TriggerKind::NewIssue).for_project(pid),
    )
    .await
    .unwrap();
    let matched = svc.try_fire_for_event(&ctx_for(pid, true)).await.unwrap();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].rule.trigger_kind, TriggerKind::Regression);
}

// ── throttle / atomic claim ─────────────────────────────────

#[tokio::test]
async fn try_fire_throttle_blocks_second_fire() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "p1").await;
    let svc = AlertRuleService::new(pool);
    svc.create_rule(
        AlertRuleDraft::new(workspace_id, "x", TriggerKind::NewIssue)
            .for_project(pid)
            .with_throttle(60),
    )
    .await
    .unwrap();
    let first = svc.try_fire_for_event(&ctx_for(pid, false)).await.unwrap();
    assert_eq!(first.len(), 1);
    // Same event again within throttle window — should NOT
    // re-fire.
    let second = svc.try_fire_for_event(&ctx_for(pid, false)).await.unwrap();
    assert!(second.is_empty());
}

#[tokio::test]
async fn try_claim_first_caller_wins_race() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "x", TriggerKind::EventCount).with_throttle(60),
        )
        .await
        .unwrap();
    let svc2 = svc.clone();
    let svc3 = svc.clone();
    let (a, b, c) = tokio::join!(
        svc.try_claim(id, 60),
        svc2.try_claim(id, 60),
        svc3.try_claim(id, 60),
    );
    let winners = [a.unwrap(), b.unwrap(), c.unwrap()]
        .iter()
        .filter(|x| **x)
        .count();
    assert_eq!(winners, 1, "exactly one caller wins the throttle slot");
}

#[tokio::test]
async fn try_claim_zero_throttle_always_succeeds() {
    let (pool, workspace_id) = fresh_pool().await;
    let svc = AlertRuleService::new(pool);
    let id = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "x", TriggerKind::EventCount).with_throttle(0),
        )
        .await
        .unwrap();
    assert!(svc.try_claim(id, 0).await.unwrap());
    assert!(svc.try_claim(id, 0).await.unwrap(), "throttle=0 ≠ block");
}

// ── delete on project cascade ───────────────────────────────

#[tokio::test]
async fn project_cascade_drops_scoped_rules() {
    let (pool, workspace_id) = fresh_pool().await;
    let pid = seed_project(&pool, workspace_id, "doomed").await;
    let svc = AlertRuleService::new(pool.clone());
    let id = svc
        .create_rule(
            AlertRuleDraft::new(workspace_id, "scoped", TriggerKind::NewIssue).for_project(pid),
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(pid.into_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(svc.find(id).await.unwrap().is_none(), "FK cascade");
}
