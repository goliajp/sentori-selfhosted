//! Crash-free rate — the first number a mobile team is asked for.
//!
//! The product counted what went wrong and nothing counted what went
//! right, so "18 errors" had no denominator. Two reviewers evaluating
//! this against Crashlytics named that as the reason not to adopt.
//!
//! Everything here is computed from `sessions`, which is append-only.
//! Nothing is cached: dropping every number and recomputing gives the
//! same answer, which is the point of keeping the facts separate from
//! what is derived from them.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    pub project_id: Uuid,
    /// How far back, in hours. The default is the window a release
    /// decision is made in.
    pub hours: Option<i64>,
    pub environment: Option<String>,
}

/// GET /admin/api/sessions/crash-free
///
/// Answers per release, newest first, and once for the whole window.
/// Per release because the question is never "are we healthy" — it is
/// "is the build we shipped on Tuesday worse than the one before it".
pub async fn crash_free(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Query(params): Query<Params>,
) -> (StatusCode, Json<Value>) {
    if let Err(e) =
        super::admin::tokens::ensure_project_access(&state, &ctx, params.project_id).await
    {
        return e;
    }
    let hours = params.hours.unwrap_or(24).clamp(1, 24 * 90);

    // `crashed` is the numerator everyone means by "crash-free"; an
    // `errored` session is one the app survived, and folding it in
    // here would make the number disagree with every other tool the
    // reader has used.
    let rows = sqlx::query(
        "SELECT release, platform, \
                COUNT(*)::bigint AS total, \
                COUNT(*) FILTER (WHERE status = 'crashed')::bigint AS crashed, \
                COUNT(DISTINCT user_key)::bigint AS users, \
                COUNT(DISTINCT user_key) FILTER (WHERE status = 'crashed')::bigint \
                  AS crashed_users, \
                MAX(started_at) AS last_at \
         FROM sessions \
         WHERE project_id = $1 \
           AND started_at > now() - make_interval(hours => $2::int) \
           AND ($3::text IS NULL OR environment = $3) \
         GROUP BY release, platform \
         ORDER BY MAX(started_at) DESC",
    )
    .bind(params.project_id)
    .bind(i32::try_from(hours).unwrap_or(24))
    .bind(params.environment.as_deref())
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut total: i64 = 0;
    let mut crashed: i64 = 0;
    let releases: Vec<Value> = rows
        .iter()
        .map(|r| {
            let t: i64 = r.get("total");
            let c: i64 = r.get("crashed");
            let u: i64 = r.get("users");
            let cu: i64 = r.get("crashed_users");
            total += t;
            crashed += c;
            json!({
                "release": r.get::<String, _>("release"),
                "platform": r.get::<String, _>("platform"),
                "sessions": t,
                "crashedSessions": c,
                "crashFreeSessions": rate(t, c),
                "users": u,
                "crashedUsers": cu,
                // Null rather than 100% when nobody was identified.
                // A rate over an empty set is not 100, it is unknown,
                // and rendering it as perfect health is the kind of
                // lie this product exists to not tell.
                "crashFreeUsers": rate(u, cu),
                "lastAt": crate::wire_time::rfc3339(r.get("last_at")),
            })
        })
        .collect();

    // Not a sum of the per-release user counts: someone who ran two
    // releases in the window is one person, and adding the groups
    // would report them twice. The distinct count has to be taken
    // across the whole window in its own pass.
    let users: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(DISTINCT user_key)::bigint, \
                COUNT(DISTINCT user_key) FILTER (WHERE status = 'crashed')::bigint \
         FROM sessions \
         WHERE project_id = $1 \
           AND started_at > now() - make_interval(hours => $2::int) \
           AND ($3::text IS NULL OR environment = $3) \
           AND user_key IS NOT NULL",
    )
    .bind(params.project_id)
    .bind(i32::try_from(hours).unwrap_or(24))
    .bind(params.environment.as_deref())
    .fetch_one(&state.pool)
    .await
    .unwrap_or((0, 0));

    // The shape of the window, not just its total.
    //
    // A single number answers "are we healthy" and hides "did Tuesday's
    // build make it worse" — which is the question the number is
    // actually consulted for. Per release already answers part of it;
    // this answers the rest, for a project that ships one release and
    // watches it.
    //
    // Bucketed server-side because the client must not have to hold a
    // window's sessions to draw a line over it, and because the bucket
    // boundaries then agree with the totals above rather than being a
    // second opinion computed from different rows.
    let buckets = trend(&state, &params, hours).await;

    (
        StatusCode::OK,
        Json(json!({
            "windowHours": hours,
            "trend": buckets,
            "sessions": total,
            "crashedSessions": crashed,
            "crashFreeSessions": rate(total, crashed),
            // Sessions and users are different populations, and the
            // console labels them as such: an app can be 99% crash-free
            // by session and have hit a third of its users. Both are
            // over the same window and the same `user_key` the issue
            // breadth counts, so the two numbers are comparable — which
            // they were not while this endpoint answered only sessions.
            "users": users.0,
            "crashedUsers": users.1,
            "crashFreeUsers": rate(users.0, users.1),
            "releases": releases,
        })),
    )
}

/// The window split into equal buckets, oldest first.
///
/// Empty buckets are present with a null rate rather than absent: a
/// gap in a line is a period with no sessions, and dropping the point
/// would draw a straight line across it as if the rate had held.
async fn trend(state: &Arc<AppState>, params: &Params, hours: i64) -> Vec<Value> {
    // A day's window reads by the hour, a quarter's by the day. Fixing
    // the count instead would make a 24h line of 1.6-hour buckets,
    // which no reader thinks in.
    let bucket_minutes: i64 = match hours {
        0..=24 => 60,
        25..=168 => 60 * 6,
        _ => 60 * 24,
    };
    // `generate_series` first, then a LEFT JOIN, so a bucket with no
    // sessions comes back as a row with a null rate. Grouping the
    // sessions alone would simply omit it, and a line drawn through
    // the remaining points runs straight across the gap as though the
    // rate had held there — the reader cannot tell "fine" from "nobody
    // opened the app".
    let rows = sqlx::query(
        "WITH bounds AS ( \
             SELECT now() - make_interval(hours => $2::int) AS from_at, \
                    make_interval(mins => $4::int) AS step \
         ), \
         slots AS ( \
             SELECT g AS at, g + bounds.step AS until \
             FROM bounds, \
                  generate_series(bounds.from_at, now() - bounds.step, bounds.step) AS g \
         ) \
         SELECT slots.at, \
                COUNT(s.id)::bigint AS total, \
                COUNT(s.id) FILTER (WHERE s.status = 'crashed')::bigint AS crashed \
         FROM slots \
         LEFT JOIN sessions s \
                ON s.project_id = $1 \
               AND s.started_at >= slots.at \
               AND s.started_at < slots.until \
               AND ($3::text IS NULL OR s.environment = $3) \
         GROUP BY slots.at \
         ORDER BY slots.at",
    )
    .bind(params.project_id)
    .bind(i32::try_from(hours).unwrap_or(24))
    .bind(params.environment.as_deref())
    .bind(i32::try_from(bucket_minutes).unwrap_or(60))
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            let t: i64 = r.get("total");
            let c: i64 = r.get("crashed");
            json!({
                "at": crate::wire_time::rfc3339(r.get("at")),
                "sessions": t,
                "crashedSessions": c,
                "crashFreeSessions": rate(t, c),
            })
        })
        .collect()
}

/// The share that did **not** crash, as a percentage with two
/// decimals — or null when there is nothing to divide by.
///
/// Null, not 100: "no sessions yet" and "every session was fine" are
/// different facts, and a dashboard that shows 100% for an app nobody
/// has run is telling its reader the opposite of the truth.
fn rate(total: i64, bad: i64) -> Option<f64> {
    if total <= 0 {
        return None;
    }
    // f64 loses precision past 2^53 sessions. A project reaching that
    // has a bigger problem than two decimal places.
    let good = f64::from(i32::try_from(total - bad).unwrap_or(i32::MAX))
        / f64::from(i32::try_from(total).unwrap_or(i32::MAX))
        * 100.0;
    Some((good * 100.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::rate;

    #[test]
    fn an_empty_window_is_unknown_rather_than_perfect() {
        // The whole reason this returns an Option. A new project with
        // no traffic would otherwise read 100% crash-free, which is
        // the most flattering possible way to be wrong.
        assert_eq!(rate(0, 0), None);
    }

    #[test]
    fn the_rate_is_the_share_that_did_not_crash() {
        assert_eq!(rate(100, 1), Some(99.0));
        assert_eq!(rate(1000, 1), Some(99.9));
        assert_eq!(rate(10, 10), Some(0.0));
        assert_eq!(rate(10, 0), Some(100.0));
    }

    #[test]
    fn two_decimals_because_the_third_is_noise_and_the_first_is_not_enough() {
        // 99.9 and 99.95 are different products; 99.9501 and 99.9502
        // are the same one measured twice.
        assert_eq!(rate(10_000, 5), Some(99.95));
        assert_eq!(rate(3, 1), Some(66.67));
    }
}
