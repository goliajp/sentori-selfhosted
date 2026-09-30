//! Notification admin surface:
//!
//! - `GET  /admin/api/smtp`               — SMTP channel status
//! - `POST /admin/api/smtp/test`          — send a test mail to the caller
//! - `GET  /admin/api/notification-prefs` — the caller's per-project switches
//! - `PUT  /admin/api/notification-prefs` — upsert one project's switches
//!
//! Prefs are per-user: every admin tunes their own inbox. No row
//! means both occasions on (0006's contract), so the list endpoint
//! synthesizes defaults for projects without a row.

use std::sync::Arc;
use time::OffsetDateTime;

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_notifier::{Channel, Notification, Notifier};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// Whether mail is configured, and whether it is getting through.
///
/// This answered `configured: true` and nothing else. On 2026-09-30 the
/// certificate on the SMTP host had been expired for two days, 110
/// notifications had failed in a row, and this endpoint — and the
/// screen that reads it — said mail was fine. Configured and working
/// are two facts, and only the second one matters to someone asking
/// why they stopped hearing about crashes.
///
/// The second fact comes from `delivery_log` rather than from a probe
/// connection: what happened to the mail we actually sent is a better
/// answer than whether a test connection opens now, and it costs one
/// indexed query instead of an SMTP round trip on every page load.
pub async fn smtp_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Delivery history is reported either way. An instance with no
    // SMTP still has a log — of webhook sends, and of the mail it used
    // to send before someone unset the configuration — and "not
    // configured" is not an answer to "did the last notification get
    // through". The e2e stack runs without SMTP on purpose, which is
    // how the first version of this was found reporting nothing there.
    let smtp = state.mailer.smtp_info();

    let recent: Option<(i64, i64, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'delivered'), \
                count(*) FILTER (WHERE status = 'failed'), \
                max(created_at) FILTER (WHERE status = 'delivered') \
           FROM delivery_log \
          WHERE channel = 'email' AND created_at > now() - interval '7 days'",
    )
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let (delivered, failed, last_delivered) = recent.unwrap_or((0, 0, None));

    // The most recent attempt, whatever it was. A count of failures
    // says how bad last week was; this says whether mail works now,
    // and they disagree exactly when it matters — the week Sentori
    // delivered 52 and then failed 110 in a row reads as healthy by
    // any measure that only counts.
    let last: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT status, error FROM delivery_log \
          WHERE channel = 'email' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let (last_status, last_error) = match last {
        Some((s, e)) => (Some(s), e),
        None => (None, None),
    };

    Json(json!({
        "configured": smtp.is_some(),
        "host": smtp.map(|(h, _)| h),
        "from": smtp.map(|(_, f)| f),
        // Last seven days. A run of failures is the thing worth
        // seeing; a single one is usually one bad address.
        "delivered7d": delivered,
        "failed7d": failed,
        "lastError": last_error,
        "lastDeliveredAt": last_delivered.map(crate::wire_time::rfc3339),
        // What the screen branches on. Never having sent anything is
        // not unhealthy — a fresh instance has an empty log.
        "healthy": last_status.as_deref() != Some("failed"),
        "lastStatus": last_status,
    }))
}

pub async fn smtp_test(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<Value>) {
    let Some(transport) = state.mailer.transport() else {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": "smtp_unconfigured" })),
        );
    };
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(ctx.user_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    let Some(email) = email else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "user_not_found" })),
        );
    };
    let n = Notification::new(
        Channel::Email,
        email.clone(),
        "[sentori] Test email".to_string(),
        format!(
            "This is a test email from your Sentori instance.\n\n\
             If you can read this, SMTP delivery works.\n\n  {}\n",
            state.mailer.base_url()
        ),
    );
    match transport.send(&n).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "to": email }))),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "smtp_send_failed", "detail": e.to_string() })),
        ),
    }
}

pub async fn prefs_list(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
) -> (StatusCode, Json<Value>) {
    // Stable, not pretty. The console re-sorts this in the viewer's
    // language — locale-aware ordering belongs where the locale is
    // known, and a database never knows which of the three languages
    // is on screen. `COLLATE "C"` only keeps the fallback order
    // identical across deployments, because ours are not one thing:
    // the shipped compose is postgres:18-alpine, which is musl and
    // behaves as C while declaring en_US.utf8.
    let rows = sqlx::query(
        "SELECT p.id AS project_id, p.name, \
                COALESCE(np.on_new_issue, TRUE) AS on_new_issue, \
                COALESCE(np.on_regression, TRUE) AS on_regression \
         FROM projects p \
         LEFT JOIN notification_prefs np \
                ON np.project_id = p.id AND np.user_id = $1 \
         ORDER BY p.name COLLATE \"C\"",
    )
    .bind(ctx.user_id)
    .fetch_all(&state.pool)
    .await;
    match rows {
        Ok(rows) => {
            let out: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "projectId": r.get::<uuid::Uuid, _>("project_id"),
                        "projectName": r.get::<String, _>("name"),
                        "onNewIssue": r.get::<bool, _>("on_new_issue"),
                        "onRegression": r.get::<bool, _>("on_regression"),
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({ "prefs": out })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "db", "detail": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefUpdate {
    pub project_id: uuid::Uuid,
    pub on_new_issue: bool,
    pub on_regression: bool,
}

pub async fn prefs_put(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Json(body): Json<PrefUpdate>,
) -> (StatusCode, Json<Value>) {
    let res = sqlx::query(
        "INSERT INTO notification_prefs (user_id, project_id, on_new_issue, on_regression) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, project_id) \
         DO UPDATE SET on_new_issue = $3, on_regression = $4",
    )
    .bind(ctx.user_id)
    .bind(body.project_id)
    .bind(body.on_new_issue)
    .bind(body.on_regression)
    .execute(&state.pool)
    .await;
    match res {
        Ok(_) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(sqlx::Error::Database(e)) if e.constraint().is_some() => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "project_not_found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "db", "detail": e.to_string() })),
        ),
    }
}
