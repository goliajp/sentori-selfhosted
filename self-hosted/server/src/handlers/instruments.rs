//! GET /admin/api/projects/{id}/instruments — the devices panel:
//! everything the developer deliberately planted, in one response.
//! Asserts answer "is it alive and passing", probes answer "is the
//! fix holding", traces answer "did the code path
//! run". A panel of instruments, not a data browser.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct InstrumentsQuery {
    pub release: Option<String>,
    pub q: Option<String>,
    pub hours: Option<i32>,
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<InstrumentsQuery>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) = super::admin::tokens::ensure_project_access(&state, &ctx, project_id).await {
        return e;
    }

    let hours = q.hours.unwrap_or(24);
    if ![1, 24, 168, 720].contains(&hours)
        || q.q.as_ref().is_some_and(|v| v.chars().count() > 200)
        || q.release.as_ref().is_some_and(|v| v.len() > 1000)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_filter"})),
        );
    }
    let query = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let asserts = sqlx::query(
        "SELECT name, release, pass_count, fail_count, last_pass_at, last_fail_at \
         FROM assert_stats WHERE project_id = $1 \
           AND ($2::text IS NULL OR release=$2) \
           AND ($3::text IS NULL OR strpos(lower(name || ' ' || release),lower($3))>0) \
         ORDER BY fail_count DESC, name COLLATE \"C\", release COLLATE \"C\" DESC \
         LIMIT 200",
    )
    .bind(project_id)
    .bind(q.release.as_deref())
    .bind(query)
    .fetch_all(&state.pool)
    .await;

    let probes = sqlx::query(
        "SELECT ref, issue_id, first_registered_release, last_seen_release, \
                registered_at, last_fired_at, fire_count \
         FROM probes WHERE project_id = $1 \
           AND ($2::text IS NULL OR last_seen_release=$2 OR first_registered_release=$2) \
           AND ($3::text IS NULL OR strpos(lower(ref || ' ' || coalesce(last_seen_release,'')),lower($3))>0) \
         ORDER BY ref COLLATE \"C\" LIMIT 200",
    )
    .bind(project_id).bind(q.release.as_deref()).bind(query)
    .fetch_all(&state.pool).await;

    let asserts = match asserts {
        Ok(rows) => rows,
        Err(error) => return super::jobs::database_error(error),
    };
    let probes = match probes {
        Ok(rows) => rows,
        Err(error) => return super::jobs::database_error(error),
    };

    // Trace observation points: the trace-kind issues themselves are
    // the aggregate ("did it run, how often, when last").
    let traces = sqlx::query(
        "SELECT i.group_title,count(*) AS event_count,count(DISTINCT e.user_key) AS users_count,
                max(e.occurred_at) AS last_seen
         FROM events e JOIN issues i ON i.id=e.issue_id
         WHERE e.project_id=$1 AND e.kind='trace' AND e.received_at>now()-make_interval(hours=>$4)
           AND ($2::text IS NULL OR e.release=$2)
           AND ($3::text IS NULL OR strpos(lower(i.group_title || ' ' || e.release),lower($3))>0)
         GROUP BY i.group_title ORDER BY event_count DESC,i.group_title COLLATE \"C\" LIMIT 200",
    )
    .bind(project_id)
    .bind(q.release.as_deref())
    .bind(query)
    .bind(hours)
    .fetch_all(&state.pool)
    .await;
    let traces = match traces {
        Ok(rows) => rows,
        Err(error) => return super::jobs::database_error(error),
    };

    // Launch percentiles (QIP-8 #3): p50/p90/p95 of app.launch
    // totalMs per release in the observation window. Pre-warmed samples
    // are excluded from the percentiles but counted, so "median got
    // worse" and "more background starts this week" stay separable.
    let launch = launch_rows(&state.pool, project_id, q.release.as_deref(), query, hours).await;
    let launch = match launch {
        Ok(rows) => rows,
        Err(error) => return super::jobs::database_error(error),
    };
    let activity = activity_rows(&state.pool, project_id, q.release.as_deref(), query, hours).await;
    let activity = match activity {
        Ok(rows) => rows,
        Err(error) => return super::jobs::database_error(error),
    };

    (
        StatusCode::OK,
        Json(instrument_json(
            hours, &activity, &asserts, &probes, &traces, &launch,
        )),
    )
}

async fn launch_rows(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    release: Option<&str>,
    query: Option<&str>,
    hours: i32,
) -> Result<Vec<PgRow>, sqlx::Error> {
    sqlx::query(
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
           AND e.received_at > now() - make_interval(hours=>$4) \
           AND ($2::text IS NULL OR e.release=$2) \
           AND ($3::text IS NULL OR strpos(lower(i.group_title || ' ' || e.release),lower($3))>0) \
           AND length(e.payload->'data'->>'totalMs')<=18 \
           AND e.payload->'data'->>'totalMs' ~ '^[0-9]+(\\.[0-9]+)?$' \
         GROUP BY e.release ORDER BY max(e.received_at) DESC LIMIT 30",
    )
    .bind(project_id)
    .bind(release)
    .bind(query)
    .bind(hours)
    .fetch_all(pool)
    .await
}

async fn activity_rows(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    release: Option<&str>,
    query: Option<&str>,
    hours: i32,
) -> Result<Vec<PgRow>, sqlx::Error> {
    sqlx::query(
        "WITH bounds AS (SELECT now() AS until,now()-make_interval(hours=>$4) AS since,
            CASE WHEN $4<=48 THEN 'hour' ELSE 'day' END AS grain),
         observed AS MATERIALIZED (
           SELECT e.received_at,e.kind,e.user_key FROM events e JOIN issues i ON i.id=e.issue_id,bounds b
           WHERE e.project_id=$1 AND e.received_at>=b.since AND e.received_at<=b.until
             AND ($2::text IS NULL OR e.release=$2)
             AND ($3::text IS NULL OR strpos(lower(i.group_title || ' ' || e.release),lower($3))>0)
         ), buckets AS (
           SELECT generate_series(date_trunc(grain,since),date_trunc(grain,until),
             CASE WHEN $4<=48 THEN interval '1 hour' ELSE interval '1 day' END) AS at FROM bounds
         ), counts AS (
           SELECT date_trunc(b.grain,e.received_at) AS at,count(*) AS events,
             count(*) FILTER (WHERE e.kind='error') AS errors,
             count(*) FILTER (WHERE e.kind='warn') AS warnings,
             count(*) FILTER (WHERE e.kind='assert') AS assertions,
             count(*) FILTER (WHERE e.kind='probe') AS probes,
             count(*) FILTER (WHERE e.kind='trace') AS traces
           FROM observed e,bounds b GROUP BY at
         ) SELECT b.at,coalesce(c.events,0) AS events,coalesce(c.errors,0) AS errors,
           coalesce(c.warnings,0) AS warnings,coalesce(c.assertions,0) AS assertions,
           coalesce(c.probes,0) AS probes,coalesce(c.traces,0) AS traces,
           (SELECT count(DISTINCT user_key) FROM observed) AS users
         FROM buckets b LEFT JOIN counts c USING(at) ORDER BY b.at",
    )
    .bind(project_id)
    .bind(release)
    .bind(query)
    .bind(hours)
    .fetch_all(pool)
    .await
}

fn instrument_json(
    hours: i32,
    activity: &[PgRow],
    asserts: &[PgRow],
    probes: &[PgRow],
    traces: &[PgRow],
    launch: &[PgRow],
) -> Value {
    json!({
        "windowHours":hours,
        "users":activity.first().map_or(0,|r|r.get::<i64,_>("users")),
        "activity":activity.iter().map(|r|json!({"at":crate::wire_time::rfc3339(r.get("at")),
            "events":r.get::<i64,_>("events"),"errors":r.get::<i64,_>("errors"),
            "warnings":r.get::<i64,_>("warnings"),"assertions":r.get::<i64,_>("assertions"),
            "probes":r.get::<i64,_>("probes"),"traces":r.get::<i64,_>("traces")})).collect::<Vec<_>>(),
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
    })
}
