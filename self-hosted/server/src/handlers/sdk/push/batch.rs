//! What happened to one call to `POST /v1/push/send`.
//!
//! ```text
//! GET /v1/push/sends/{sendId}              the aggregate
//! GET /v1/push/sends/{sendId}/deliveries   the rows behind it
//! ```
//!
//! A send writes one row per device. Before this, the caller was
//! handed every one of those ids and had to poll each — homework for
//! a hundred and twenty-eight devices, and a three-megabyte response
//! before it was anything else for a hundred thousand.
//!
//! The call is what an integrator has. It has an id; the id has a
//! state; the state aggregates its rows; the rows are listable when
//! the aggregate is not enough. One poll answers the common question
//! and the drill-down is there when it does not.
//!
//! ## Sent is not delivered
//!
//! `sent` means the vendor accepted it — Apple or Google took the
//! request and said yes. `delivered` means the device said so, which
//! only happens for apps whose SDK acks. They are different facts and
//! they are counted separately: reading `sent` as `delivered` is how
//! an integrator concludes a notification arrived when the phone was
//! off.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::state::AppState;

fn internal(e: &sqlx::Error) -> (StatusCode, Json<Value>) {
    warn!(error = %e, "push.batch query failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
}

fn not_found() -> (StatusCode, Json<Value>) {
    // A 404, not a 200 carrying the word. The per-delivery receipt
    // answered `{"status":"not_found"}` with a 200, so a typo in an id
    // and a send that failed looked the same to anything reading the
    // status field.
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "send_not_found" })),
    )
}

/// The aggregate.
pub async fn summary(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(send_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    crate::handlers::sdk::require_admin_token(&ctx)?;

    let row = sqlx::query(
        "SELECT count(*) AS total, \
                count(*) FILTER (WHERE status = 'queued') AS queued, \
                count(*) FILTER (WHERE status = 'sent')   AS sent, \
                count(*) FILTER (WHERE status = 'failed') AS failed, \
                count(*) FILTER (WHERE acked_at IS NOT NULL) AS delivered, \
                min(created_at) AS created_at, \
                max(sent_at)    AS last_sent_at \
         FROM push_sends WHERE project_id = $1 AND batch_id = $2",
    )
    .bind(ctx.project_id)
    .bind(send_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let total: i64 = row.get("total");
    if total == 0 {
        return Err(not_found());
    }

    let queued: i64 = row.get("queued");
    let sent: i64 = row.get("sent");
    let failed: i64 = row.get("failed");

    // Why the failures failed, which is the only part of a failure
    // worth polling for. Capped: a hundred thousand rows do not have a
    // hundred thousand distinct reasons, and if they did the first
    // eight would still be the answer.
    let reasons = sqlx::query(
        "SELECT coalesce(nullif(error, ''), provider_outcome, 'unknown') AS reason, \
                count(*) AS n \
         FROM push_sends \
         WHERE project_id = $1 AND batch_id = $2 AND status = 'failed' \
         GROUP BY 1 ORDER BY n DESC LIMIT 8",
    )
    .bind(ctx.project_id)
    .bind(send_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    Ok(Json(json!({
        "sendId": send_id.to_string(),
        // `done` means nothing is queued any more, not that everything
        // arrived. The counts say what happened; this says whether to
        // keep polling.
        "state": if queued > 0 { "in_flight" } else { "done" },
        "createdAt": crate::wire_time::rfc3339_opt(
            row.try_get::<Option<time::OffsetDateTime>, _>("created_at").ok().flatten()
        ),
        "lastSentAt": crate::wire_time::rfc3339_opt(
            row.try_get::<Option<time::OffsetDateTime>, _>("last_sent_at").ok().flatten()
        ),
        "counts": {
            "total": total,
            "queued": queued,
            // The vendor accepted it.
            "sent": sent,
            "failed": failed,
            // The device said so. A subset of `sent`, and only for
            // apps whose SDK acks — zero here is not evidence of
            // non-delivery.
            "delivered": row.get::<i64, _>("delivered"),
        },
        "reasons": reasons.iter().map(|r| json!({
            "reason": r.get::<String, _>("reason"),
            "count": r.get::<i64, _>("n"),
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// `queued` / `sent` / `failed`, or absent for all of them.
    pub status: Option<String>,
    pub limit: Option<u32>,
    /// The `nextCursor` from the previous page.
    pub cursor: Option<Uuid>,
}

/// The rows behind the aggregate.
pub async fn deliveries(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path(send_id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    crate::handlers::sdk::require_admin_token(&ctx)?;

    let limit = i64::from(q.limit.unwrap_or(100).clamp(1, 1000));

    // Keyset, not offset. A caller walking a hundred thousand rows
    // with OFFSET makes the last page cost a hundred thousand rows to
    // skip, and the ids are v7 so they order by time already.
    let rows = sqlx::query(
        "SELECT id, token_id, status, provider, provider_outcome, error, \
                sent_at, acked_at, retry_count \
         FROM push_sends \
         WHERE project_id = $1 AND batch_id = $2 \
           AND ($3::text IS NULL OR status = $3) \
           AND ($4::uuid IS NULL OR id > $4) \
         ORDER BY id LIMIT $5",
    )
    .bind(ctx.project_id)
    .bind(send_id)
    .bind(q.status.as_deref())
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| internal(&e))?;

    let deliveries: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "deliveryId": r.get::<Uuid, _>("id").to_string(),
                // The device this row is about, by the address the
                // caller sends to.
                "spToken": r.get::<Uuid, _>("token_id").to_string(),
                "status": r.get::<String, _>("status"),
                "provider": r.get::<String, _>("provider"),
                "providerOutcome": r.get::<Option<String>, _>("provider_outcome"),
                "error": r.get::<Option<String>, _>("error"),
                "sentAt": crate::wire_time::rfc3339_opt(
                    r.get::<Option<time::OffsetDateTime>, _>("sent_at")
                ),
                "deliveredAt": crate::wire_time::rfc3339_opt(
                    r.get::<Option<time::OffsetDateTime>, _>("acked_at")
                ),
                "retryCount": r.get::<i32, _>("retry_count"),
            })
        })
        .collect();

    // A cursor only when there may be more. Handing one back on a
    // short page makes a caller ask for an empty page to find out.
    let next = if i64::try_from(rows.len()).unwrap_or(0) == limit {
        rows.last().map(|r| r.get::<Uuid, _>("id").to_string())
    } else {
        None
    };

    Ok(Json(
        json!({ "deliveries": deliveries, "nextCursor": next }),
    ))
}
