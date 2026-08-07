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

use axum::{Extension, Json, extract::State, http::StatusCode};
use sentori_notifier::{Channel, Notification, Notifier};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn smtp_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    match state.mailer.smtp_info() {
        Some((host, from)) => Json(json!({
            "configured": true,
            "host": host,
            "from": from,
        })),
        None => Json(json!({ "configured": false })),
    }
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
    let rows = sqlx::query(
        "SELECT p.id AS project_id, p.name, \
                COALESCE(np.on_new_issue, TRUE) AS on_new_issue, \
                COALESCE(np.on_regression, TRUE) AS on_regression \
         FROM projects p \
         LEFT JOIN notification_prefs np \
                ON np.project_id = p.id AND np.user_id = $1 \
         ORDER BY p.name",
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
