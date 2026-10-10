//! Transactional event capture and durable fan-out. No network call occurs in a database transaction.
use crate::state::AppState;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub struct Event {
    pub project: Uuid,
    pub key: String,
    pub kind: String,
    pub payload: Value,
    pub issue: Option<Uuid>,
    pub actor: Option<Uuid>,
    pub target: Option<Uuid>,
}
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    event: Event,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO notification_events(id,project_id,source_key,kind,payload,issue_id,actor_user_id,target_integration_id,generation) SELECT $1,$2,$3,$4,$5,$6,$7,$8,generation FROM instance_settings WHERE singleton ON CONFLICT(project_id,source_key) DO UPDATE SET source_key=EXCLUDED.source_key RETURNING id")
        .bind(Uuid::now_v7()).bind(event.project).bind(event.key).bind(event.kind).bind(event.payload).bind(event.issue).bind(event.actor).bind(event.target).fetch_one(&mut **tx).await
}
pub async fn issue(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
    issue: Uuid,
    event: Uuid,
    new: bool,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query(
        "SELECT kind,group_title,message_sample FROM issues WHERE id=$1 AND project_id=$2",
    )
    .bind(issue)
    .bind(project)
    .fetch_one(&mut **tx)
    .await?;
    let kind = if new { "issue.new" } else { "issue.regressed" };
    let title = format!(
        "[sentori] {}: {}",
        row.get::<String, _>("kind"),
        row.get::<String, _>("group_title")
    );
    let body: String = row
        .get::<String, _>("message_sample")
        .chars()
        .take(4000)
        .collect();
    enqueue(tx,Event{project,key:format!("{kind}:{issue}:{event}"),kind:kind.into(),payload:json!({"event":kind,"title":title,"body":body,"route":format!("/issues/{issue}?project={project}")}),issue:Some(issue),actor:None,target:None}).await?;
    Ok(())
}
pub async fn task(
    tx: &mut Transaction<'_, Postgres>,
    task: Uuid,
    event: Uuid,
    actor: Option<Uuid>,
    assigned: bool,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query("SELECT project_id,title,stage FROM tasks WHERE id=$1")
        .bind(task)
        .fetch_one(&mut **tx)
        .await?;
    let project: Uuid = row.get("project_id");
    let kind = if assigned {
        "task.assigned"
    } else {
        "task.updated"
    };
    enqueue(tx,Event{project,key:format!("task:{event}"),kind:kind.into(),payload:json!({"event":kind,"title":row.get::<String,_>("title"),"body":row.get::<String,_>("stage"),"route":format!("/tasks?project={project}&id={task}")}),issue:None,actor,target:None}).await?;
    Ok(())
}

pub async fn route(state: &AppState) -> Result<bool, sqlx::Error> {
    let mut tx = state.pool.begin().await?;
    let row=sqlx::query("SELECT * FROM notification_events WHERE routed_at IS NULL ORDER BY created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED").fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(false) };
    let id: Uuid = row.get("id");
    // Configuration, membership and preference checks are repeated immediately before send.
    sqlx::query("INSERT INTO notification_deliveries(id,project_id,event_id,integration_id,destination,config_version) SELECT gen_random_uuid(),e.project_id,e.id,i.id,'integration',i.version FROM notification_events e JOIN instance_settings s ON s.singleton AND s.generation=e.generation JOIN outbound_integrations i ON i.project_id=e.project_id AND i.enabled WHERE e.id=$1 AND ((e.target_integration_id=i.id) OR (e.target_integration_id IS NULL AND e.kind=ANY(i.events) AND i.created_at<=e.created_at)) ON CONFLICT DO NOTHING")
        .bind(id).execute(&mut *tx).await?;
    if state.mailer.transport().is_some() {
        sqlx::query("INSERT INTO notification_deliveries(id,project_id,event_id,user_id,destination,config_version) SELECT gen_random_uuid(),e.project_id,e.id,u.id,'user_email',0 FROM notification_events e JOIN instance_settings s ON s.singleton AND s.generation=e.generation JOIN users u ON (u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.user_id=u.id AND a.project_id=e.project_id)) LEFT JOIN notification_prefs p ON p.user_id=u.id AND p.project_id=e.project_id WHERE e.id=$1 AND e.kind IN ('issue.new','issue.regressed') AND CASE WHEN e.kind='issue.new' THEN COALESCE(p.on_new_issue,true) ELSE COALESCE(p.on_regression,true) END ON CONFLICT DO NOTHING")
            .bind(id).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO notification_deliveries(id,project_id,event_id,destination,config_version) SELECT gen_random_uuid(),e.project_id,e.id,'legacy_webhook',p.version FROM notification_events e JOIN instance_settings s ON s.singleton AND s.generation=e.generation JOIN projects p ON p.id=e.project_id WHERE e.id=$1 AND e.kind IN ('issue.new','issue.regressed') AND (p.webhook_secret_blob IS NOT NULL OR p.webhook_url IS NOT NULL) ON CONFLICT DO NOTHING")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE notification_events SET routed_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Upgrade legacy webhook URLs in bounded transactions. The key is never persisted here.
pub async fn migrate_legacy(state: &AppState) -> Result<(), sqlx::Error> {
    let Some(vault) = &state.vault else {
        return Ok(());
    };
    let mut tx = state.pool.begin().await?;
    let rows=sqlx::query("SELECT id,webhook_url FROM projects WHERE webhook_url IS NOT NULL LIMIT 32 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in rows {
        let project: Uuid = row.get("id");
        let endpoint: String = row.get("webhook_url");
        let settings = crate::integrations::Settings {
            endpoint,
            ..crate::integrations::Settings::default()
        };
        let Ok(blob) = crate::integrations::seal(vault, project, "legacy_webhook", &settings)
        else {
            continue;
        };
        sqlx::query("UPDATE projects SET webhook_secret_blob=COALESCE(webhook_secret_blob,$2),webhook_url=NULL WHERE id=$1").bind(project).bind(blob).execute(&mut *tx).await?;
        sqlx::query("UPDATE delivery_log SET recipient='legacy_webhook',error=CASE WHEN error IS NULL THEN NULL ELSE 'legacy_delivery_error' END WHERE project_id=$1 AND channel='webhook'")
            .bind(project).execute(&mut *tx).await?;
    }
    tx.commit().await
}
