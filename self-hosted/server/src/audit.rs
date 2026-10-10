//! Append-only admin audit writer.
//!
//! Legacy mutations use `record` after committing and log audit failures.
//! Mutations requiring atomic audit use `record_in_transaction`; an audit
//! failure then rolls back both records in the caller's transaction.

use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

pub async fn record(
    pool: &PgPool,
    project_id: Option<Uuid>,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: &str,
    payload: Value,
) {
    let r = sqlx::query(
        "INSERT INTO audit_logs (id, project_id, actor_user_id, action, target_type, target_id, payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(payload)
    .execute(pool)
    .await;
    if let Err(e) = r {
        warn!(error = %e, action, "audit write failed");
    }
}

/// Record a mutation in its transaction so an audit failure rolls back the change.
pub async fn record_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Option<Uuid>,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: &str,
    payload: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_logs (id, project_id, actor_user_id, action, target_type, target_id, payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
