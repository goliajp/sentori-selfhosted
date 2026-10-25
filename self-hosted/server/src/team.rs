//! Transactional collaboration events and their database-only inbox consumer.
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

/// The already locked fields needed by a state transition.
pub struct TaskSnapshot {
    pub stage: String,
    pub assignee: Option<Uuid>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Typed event fields share one transaction"
)]
pub async fn event(
    tx: &mut Transaction<'_, Postgres>,
    task: Uuid,
    actor: Option<Uuid>,
    action: &str,
    body: &str,
    recipient: Option<Uuid>,
    category: &str,
) -> Result<(), sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO task_events(id,task_id,actor_user_id,action,body,notify_user_id,category) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(id).bind(task).bind(actor).bind(action).bind(body).bind(recipient.filter(|u| Some(*u)!=actor)).bind(category).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO outbox_events(id,task_event_id,event_type) VALUES($1,$2,'task.changed')",
    )
    .bind(Uuid::now_v7())
    .bind(id)
    .execute(&mut **tx)
    .await?;
    crate::notification_queue::task(
        tx,
        task,
        id,
        actor,
        category == "assignment" && recipient.is_some(),
    )
    .await?;
    Ok(())
}

/// Called under the ingest transaction's issue lock: a rollback also rolls back the task.
pub async fn regress(tx: &mut Transaction<'_, Postgres>, issue: Uuid) -> Result<(), sqlx::Error> {
    let rows=sqlx::query("UPDATE tasks SET stage='inProgress',verified=false,fix_release='',automation=jsonb_build_object('regressionAt',clock_timestamp()),version=version+1,updated_at=now() WHERE issue_id=$1 AND stage<>'cancelled' RETURNING id,project_id,assignee_user_id")
        .bind(issue).fetch_all(&mut **tx).await?;
    for row in rows {
        let id: Uuid = row.get("id");
        event(
            tx,
            id,
            None,
            "issue.regressed",
            "",
            row.get("assignee_user_id"),
            "task",
        )
        .await?;
        sqlx::query("INSERT INTO audit_logs(id,project_id,action,target_type,target_id,payload,ip) VALUES($1,$2,'task.regressed','task',$3,$4,$5)")
            .bind(Uuid::now_v7()).bind(row.get::<Uuid,_>("project_id")).bind(id.to_string()).bind(json!({"issueId":issue})).bind(sentori_request_origin::current()).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn dispatch(pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id:Option<Uuid>=sqlx::query_scalar("SELECT id FROM outbox_events WHERE event_type='task.changed' AND delivered_at IS NULL ORDER BY created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED")
        .fetch_optional(&mut *tx).await?;
    let Some(id) = id else { return Ok(false) };
    // Every retry rechecks membership and preferences. INSERT and acknowledgement are atomic.
    sqlx::query("INSERT INTO inbox_items(id,user_id,project_id,task_id,source_event_id,category,title,body) SELECT $2,u.id,t.project_id,t.id,e.id,e.category,t.title,e.body FROM outbox_events o JOIN task_events e ON e.id=o.task_event_id JOIN tasks t ON t.id=e.task_id JOIN users u ON u.id=e.notify_user_id LEFT JOIN inbox_preferences p ON p.user_id=u.id WHERE o.id=$1 AND (u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.project_id=t.project_id AND a.user_id=u.id)) AND CASE e.category WHEN 'assignment' THEN COALESCE(p.assignments,true) WHEN 'mention' THEN COALESCE(p.mentions,true) ELSE COALESCE(p.task_updates,true) END ON CONFLICT(user_id,source_event_id) DO NOTHING")
        .bind(id).bind(Uuid::now_v7()).execute(&mut *tx).await?;
    sqlx::query("UPDATE outbox_events SET delivered_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
pub fn spawn(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        loop {
            match dispatch(&pool).await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
                Err(error) => {
                    tracing::warn!(%error,"inbox dispatch failed");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    });
}
