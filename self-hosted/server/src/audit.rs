//! Append-only admin audit writer.
//!
//! Best-effort by design: an audit insert failure is logged, never
//! propagated — the mutation it records already happened, and
//! failing the request after the fact would leave the caller
//! believing it didn't.

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
