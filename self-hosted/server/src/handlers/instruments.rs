//! GET /admin/api/projects/{id}/instruments — the devices panel
//! (design.md §11): everything the developer deliberately planted,
//! in one response. Asserts answer "is it alive and passing", probes
//! answer "is the fix holding", traces answer "did the code path
//! run". A panel of instruments, not a data browser.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::admin::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }

    let asserts = sqlx::query(
        "SELECT name, release, pass_count, fail_count, last_pass_at, last_fail_at \
         FROM assert_stats WHERE project_id = $1 \
         ORDER BY fail_count DESC, name, release DESC LIMIT 200",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let probes = sqlx::query(
        "SELECT ref, issue_id, first_registered_release, last_seen_release, \
                registered_at, last_fired_at, fire_count \
         FROM probes WHERE project_id = $1 ORDER BY ref LIMIT 200",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    // Trace observation points: the trace-kind issues themselves are
    // the aggregate ("did it run, how often, when last").
    let traces = sqlx::query(
        "SELECT group_title, event_count, users_count, last_seen \
         FROM issues WHERE project_id = $1 AND kind = 'trace' \
         ORDER BY last_seen DESC LIMIT 200",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    // Launch percentiles (QIP-8 #3): p50/p90/p95 of app.launch
    // totalMs per release, last 30 days. Pre-warmed phantom samples
    // are excluded from the percentiles but counted, so "median got
    // worse" and "more background starts this week" stay separable.
    let launch = sqlx::query(
        "SELECT e.release, \
                count(*) AS samples, \
                count(*) FILTER (WHERE e.payload->'data'->>'prewarmed' = 'true') \
                  AS prewarmed, \
                percentile_cont(0.5) WITHIN GROUP \
                  (ORDER BY (e.payload->'data'->>'totalMs')::float8) \
                  FILTER (WHERE e.payload->'data'->>'prewarmed' IS DISTINCT FROM 'true') \
                  AS p50, \
                percentile_cont(0.9) WITHIN GROUP \
                  (ORDER BY (e.payload->'data'->>'totalMs')::float8) \
                  FILTER (WHERE e.payload->'data'->>'prewarmed' IS DISTINCT FROM 'true') \
                  AS p90, \
                percentile_cont(0.95) WITHIN GROUP \
                  (ORDER BY (e.payload->'data'->>'totalMs')::float8) \
                  FILTER (WHERE e.payload->'data'->>'prewarmed' IS DISTINCT FROM 'true') \
                  AS p95 \
         FROM events e JOIN issues i ON i.id = e.issue_id \
         WHERE e.project_id = $1 AND e.kind = 'trace' \
           AND i.group_title = 'app.launch' \
           AND e.received_at > now() - interval '30 days' \
           AND e.payload->'data'->>'totalMs' ~ '^[0-9]+(\\.[0-9]+)?$' \
         GROUP BY e.release ORDER BY max(e.received_at) DESC LIMIT 30",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({
            "asserts": asserts.iter().map(|r| json!({
                "name": r.get::<String, _>("name"),
                "release": r.get::<String, _>("release"),
                "passCount": r.get::<i64, _>("pass_count"),
                "failCount": r.get::<i64, _>("fail_count"),
                "lastPassAt": crate::wire_time::rfc3339_opt(r.get("last_pass_at")),
                "lastFailAt": crate::wire_time::rfc3339_opt(r.get("last_fail_at")),
            })).collect::<Vec<_>>(),
            "probes": probes.iter().map(|r| json!({
                "ref": r.get::<String, _>("ref"),
                "issueId": r.get::<Option<Uuid>, _>("issue_id"),
                "firstRegisteredRelease": r.get::<Option<String>, _>("first_registered_release"),
                "lastSeenRelease": r.get::<Option<String>, _>("last_seen_release"),
                "registeredAt": crate::wire_time::rfc3339(r.get("registered_at")),
                "lastFiredAt": crate::wire_time::rfc3339_opt(r.get("last_fired_at")),
                "fireCount": r.get::<i64, _>("fire_count"),
            })).collect::<Vec<_>>(),
            "traces": traces.iter().map(|r| json!({
                "name": r.get::<String, _>("group_title"),
                "eventCount": r.get::<i64, _>("event_count"),
                "usersCount": r.get::<i64, _>("users_count"),
                "lastSeen": crate::wire_time::rfc3339(r.get("last_seen")),
            })).collect::<Vec<_>>(),
            "launch": launch.iter().map(|r| json!({
                "release": r.get::<String, _>("release"),
                "samples": r.get::<i64, _>("samples"),
                "prewarmed": r.get::<i64, _>("prewarmed"),
                "p50": r.get::<Option<f64>, _>("p50"),
                "p90": r.get::<Option<f64>, _>("p90"),
                "p95": r.get::<Option<f64>, _>("p95"),
            })).collect::<Vec<_>>(),
        })),
    )
}
