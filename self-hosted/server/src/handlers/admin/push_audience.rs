//! POST /admin/api/projects/:project_id/push/audience/preview
//!
//! How many devices an audience selects, and a few of them.
//!
//! Without this the only way to find out what an expression matches is
//! to send to it, which is not a thing anyone can undo. A condition
//! editor that cannot answer "how many?" is a text box that fires
//! notifications at strangers.
//!
//! The count comes from the same compiler the send uses, so a preview
//! that says 412 is not an estimate of what a send would do — it is
//! the same query with `count(*)` in front of it.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Row};
use tracing::warn;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewBody {
    #[serde(default)]
    pub app_user_id: Option<String>,
    #[serde(default)]
    pub traits: Option<Value>,
    #[serde(default)]
    pub audience: Option<Value>,
}

/// How many of the matched devices to show.
///
/// Enough to tell "this is the group I meant" from "this is every
/// device I have", which is the question a sample answers. More than
/// that is a device list, and there is already one of those.
const SAMPLE: i64 = 8;

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<PreviewBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let audience = crate::audience::from_request(
        body.app_user_id.as_deref(),
        body.traits.as_ref(),
        body.audience.as_ref(),
    )
    .map_err(|detail| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "bad_audience", "detail": detail })),
        )
    })?;

    let Some(audience) = audience else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "bad_audience",
                "detail": "give appUserId, traits or audience",
            })),
        ));
    };

    // `$1` is the project; the audience numbers from `$2`.
    // `$1` is the project; the audience numbers from `$2`.
    let (frag, binds) = audience.to_sql(2);
    let where_clause = format!("dt.project_id = $1 AND dt.revoked_at IS NULL AND ({frag})");

    let count_sql = format!("SELECT count(*) AS n FROM device_tokens dt WHERE {where_clause}");
    // Audited for injection: the fragment `to_sql` returns contains only
    // `$n` placeholders and column names from a two-variant enum
    // (`Source::column` returns `&'static str`). Every value an
    // operator supplied travels as a bind — see `audience.rs`, where
    // this is the property the unit tests are about.
    let mut q = sqlx::query(AssertSqlSafe(count_sql.clone())).bind(project_id);
    for b in &binds {
        q = b.attach(q);
    }
    let matched = q
        .fetch_one(&state.pool)
        .await
        .map(|r| r.get::<i64, _>("n"))
        .map_err(|e| {
            // The compiler produces valid SQL for anything it accepted, so
            // a failure here is ours rather than the caller's — and saying
            // "no devices" would read as an answer.
            warn!(error = %e, "push.audience preview_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        })?;

    // The token itself never comes back — the device list is careful
    // about that and a preview must not be the way around it.
    let sample_sql = format!(
        "SELECT dt.id, dt.provider, dt.traits, dt.metadata, \
                dt.user_key IS NOT NULL AS addressable, \
                right(dt.user_key, 6) AS user_key_tail \
         FROM device_tokens dt WHERE {where_clause} \
         ORDER BY dt.last_seen_at DESC LIMIT {SAMPLE}"
    );
    // Audited for injection: the fragment `to_sql` returns contains only
    // `$n` placeholders and column names from a two-variant enum
    // (`Source::column` returns `&'static str`). Every value an
    // operator supplied travels as a bind — see `audience.rs`, where
    // this is the property the unit tests are about.
    let mut q = sqlx::query(AssertSqlSafe(sample_sql.clone())).bind(project_id);
    for b in &binds {
        q = b.attach(q);
    }
    let rows = q.fetch_all(&state.pool).await.unwrap_or_default();

    let sample: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id").to_string(),
                "provider": r.get::<String, _>("provider"),
                "traits": r.get::<Value, _>("traits"),
                "metadata": r.get::<Value, _>("metadata"),
                "addressable": r.get::<bool, _>("addressable"),
                "userKeyTail": r.get::<Option<String>, _>("user_key_tail"),
            })
        })
        .collect();

    Ok(Json(json!({ "matched": matched, "sample": sample })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBody {
    #[serde(default)]
    pub app_user_id: Option<String>,
    #[serde(default)]
    pub traits: Option<Value>,
    #[serde(default)]
    pub audience: Option<Value>,
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// What the preview said, from the operator who is looking at it.
    ///
    /// Required, and the send is refused if it no longer holds. An
    /// audience is live data: devices register between reading a
    /// number and pressing a button, and "it said 12" is the only
    /// thing standing between a careful operator and a notification
    /// to everyone. Nothing here can be undone, so the console is not
    /// allowed to send to a number nobody read.
    pub expected_matched: i64,
    /// Minted by the console when it took the count, so pressing send
    /// twice queues once.
    ///
    /// The count guard does not catch this: sending does not change
    /// the audience, so a second press finds the same number and
    /// passes. A key does — the unique index behind it is per device
    /// per key, so the second insert is skipped and `queued` comes
    /// back zero.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Queue a send to everyone an audience selects.
pub async fn send(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<SendBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let bad = |detail: String| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "bad_audience", "detail": detail })),
        )
    };

    if body.title.trim().is_empty() {
        return Err(bad("a notification with no title shows nothing".to_string()));
    }

    let audience = crate::audience::from_request(
        body.app_user_id.as_deref(),
        body.traits.as_ref(),
        body.audience.as_ref(),
    )
    .map_err(bad)?
    .ok_or_else(|| bad("give appUserId, traits or audience".to_string()))?;

    let (frag, binds) = audience.to_sql(2);
    let where_clause = format!("dt.project_id = $1 AND dt.revoked_at IS NULL AND ({frag})");

    // Counted first, and compared with what the operator read. Doing
    // it in the same transaction as the insert would be tighter, and
    // it is not what this guards against: the gap that matters is the
    // one between a human reading a number and clicking, not the
    // milliseconds inside the request.
    let count_sql = format!("SELECT count(*) AS n FROM device_tokens dt WHERE {where_clause}");
    // Audited for injection: the fragment `to_sql` returns contains only
    // `$n` placeholders and column names from a two-variant enum
    // (`Source::column` returns `&'static str`). Every value an
    // operator supplied travels as a bind — see `audience.rs`, where
    // this is the property the unit tests are about.
    let mut q = sqlx::query(AssertSqlSafe(count_sql.clone())).bind(project_id);
    for b in &binds {
        q = b.attach(q);
    }
    let matched = q
        .fetch_one(&state.pool)
        .await
        .map(|r| r.get::<i64, _>("n"))
        .map_err(|e| {
            warn!(error = %e, "push.audience send_count_failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        })?;

    if matched != body.expected_matched {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "audience_changed",
                "matched": matched,
                "expected": body.expected_matched,
                "detail": "this audience does not select what it did when you \
                           previewed it — look again before sending",
            })),
        ));
    }

    // Compiled a second time, at the offset this statement needs,
    // rather than renumbering the first one. The renumbering pass
    // moved every placeholder above `$1` — including the payload's,
    // which this statement writes by hand — so the payload and the
    // audience's first bind collided on one number, and every queued
    // row went out carrying a fragment of the condition as its
    // message. Counting the rows had said nothing about what was in
    // them.
    //
    // Two calls on the same value give the same clause with different
    // numbers and the same bind order, so the count and the send
    // cannot come to disagree about what they select.
    //
    // `$1` project, `$2` payload, `$3` the key, so the audience starts
    // at `$4` and is told so.
    let (send_frag, send_binds) = audience.to_sql(4);
    let send_where = format!("dt.project_id = $1 AND dt.revoked_at IS NULL AND ({send_frag})");
    let insert_sql = format!(
        "INSERT INTO push_sends \
           (id, project_id, token_id, provider, payload, status, idempotency_key) \
         SELECT gen_random_uuid(), $1, dt.id, dt.provider, $2, 'queued', $3 \
         FROM device_tokens dt WHERE {send_where} \
         ON CONFLICT (project_id, token_id, idempotency_key) \
           WHERE idempotency_key IS NOT NULL DO NOTHING \
         RETURNING id"
    );
    // Audited for injection: the fragment `to_sql` returns contains only
    // `$n` placeholders and column names from a two-variant enum
    // (`Source::column` returns `&'static str`). Every value an
    // operator supplied travels as a bind — see `audience.rs`, where
    // this is the property the unit tests are about.
    let mut q = sqlx::query(AssertSqlSafe(insert_sql.clone()))
        .bind(project_id)
        .bind(json!({ "title": body.title, "body": body.body }))
        .bind(body.idempotency_key.as_deref());
    for b in &send_binds {
        q = b.attach(q);
    }
    let rows = q.fetch_all(&state.pool).await.map_err(|e| {
        warn!(error = %e, "push.audience send_failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    })?;

    // Zero rows against a non-empty audience means every one of them
    // was already queued under this key — a second press of the
    // button, not an empty send. The console has to tell those apart,
    // so it is said here rather than inferred from a number that reads
    // like failure.
    Ok(Json(json!({
        "queued": rows.len(),
        "matched": matched,
        "alreadySent": rows.is_empty() && matched > 0,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same audience, compiled at two offsets, selects the same
    /// thing and binds the same values in the same order.
    ///
    /// This is what lets the count and the send be two statements
    /// rather than one renumbered one — and renumbering is what put a
    /// fragment of the condition into the notification's payload.
    #[test]
    fn two_offsets_of_one_audience_agree_on_everything_but_the_numbers() {
        let v = json!({ "all": [
            { "trait": "plan", "is": "pro" },
            { "device": "appVersion", "versionGte": "4.2" } ] });
        let parsed = crate::audience::from_request(None, None, Some(&v));
        assert!(parsed.is_ok(), "the audience did not parse");
        let Ok(Some(a)) = parsed else { return };
        let (at2, b2) = a.to_sql(2);
        let (at4, b4) = a.to_sql(4);
        assert_eq!(
            b2.len(),
            b4.len(),
            "the two offsets bound different amounts"
        );
        assert_eq!(
            format!("{b2:?}"),
            format!("{b4:?}"),
            "the two offsets bound different values, or in a different order"
        );
        let renumbered: String = at2
            .split('$')
            .enumerate()
            .map(|(i, part)| {
                if i == 0 {
                    return part.to_string();
                }
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                match digits.parse::<usize>() {
                    Ok(n) => format!("${}{}", n + 2, &part[digits.len()..]),
                    Err(_) => format!("${part}"),
                }
            })
            .collect();
        assert_eq!(
            renumbered, at4,
            "the two offsets produced different clauses"
        );
    }
}
