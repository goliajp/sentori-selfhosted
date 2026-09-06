//! GET /admin/api/projects/:project_id/push/readiness
//!
//! What is configured, what is missing, and what that costs.
//!
//! Push has a lot of ways to be almost set up, and every one of them
//! looks the same from the console: sends queue, nothing arrives, and
//! the reason is somewhere else. A project can have three hundred FCM
//! devices and no FCM credential. It can have devices that never
//! called `user()`, so every send aimed at a person reaches nobody and
//! reports success. It can have a credential nothing uses.
//!
//! Each of those was found by a person asking us. They are all
//! answerable from two tables, so they are answered here.
//!
//! ## Codes, not sentences
//!
//! Every check is an id and some numbers. The console owns the words,
//! because the console is where the words are translated and where the
//! gate that keeps them translated runs. A server that returns English
//! prose is a server that has quietly opted the dashboard out of two
//! of its three languages.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// How long a queued send may sit before the queue counts as stuck.
///
/// The worker drains every five seconds. Ten minutes is not a slow
/// queue; it is a queue nothing is reading.
const STALLED_MINUTES: i64 = 10;

/// Everything the checks are decided from, in one place.
///
/// Gathering and deciding are split so the deciding is a pure
/// function: the rules are where the mistakes are, and a rule that
/// needs a database to test is a rule that gets tested by hand once.
pub struct Facts {
    pub live: i64,
    /// Live devices per provider.
    pub per_provider: Vec<(String, i64)>,
    pub identified: i64,
    pub with_traits: i64,
    pub with_metadata: i64,
    pub sandbox: i64,
    pub quarantined: i64,
    pub credentials: Vec<String>,
    pub queued: i64,
    pub oldest_queued_minutes: i64,
    pub sent24h: i64,
    pub failed24h: i64,
    pub top_reason: Option<String>,
}

fn check(id: &str, level: &str, data: &Value) -> Value {
    json!({ "id": id, "level": level, "data": data })
}

/// The rules. Blocked first, because that is the order they bite in.
#[must_use]
pub fn checks_for(f: &Facts) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();

    // ── blocked: nothing can arrive until this is dealt with ──
    if f.live == 0 {
        out.push(check(
            "no-device",
            "blocked",
            &json!({ "quarantined": f.quarantined }),
        ));
    }
    for (provider, n) in &f.per_provider {
        if !f.credentials.contains(provider) {
            out.push(check(
                "no-credential",
                "blocked",
                &json!({ "provider": provider, "devices": n }),
            ));
        }
    }
    if f.queued > 0 && f.oldest_queued_minutes >= STALLED_MINUTES {
        out.push(check(
            "queue-stalled",
            "blocked",
            &json!({ "queued": f.queued, "minutes": f.oldest_queued_minutes }),
        ));
    }

    // ── warn: it can send, but some way of aiming reaches nobody ──
    if f.live > 0 && f.identified == 0 {
        out.push(check("no-identity", "warn", &json!({ "live": f.live })));
    }
    if f.live > 0 && f.with_traits == 0 {
        out.push(check("no-traits", "warn", &json!({ "live": f.live })));
    }
    if f.live > 0 && f.with_metadata == 0 {
        out.push(check("no-metadata", "warn", &json!({ "live": f.live })));
    }
    for kind in &f.credentials {
        if !f.per_provider.iter().any(|(p, _)| p == kind) {
            out.push(check(
                "credential-unused",
                "warn",
                &json!({ "provider": kind }),
            ));
        }
    }
    // More retired than alive is not attrition, it is a setting.
    if f.quarantined >= 5 && f.quarantined > f.live {
        out.push(check(
            "mass-quarantine",
            "warn",
            &json!({ "quarantined": f.quarantined, "live": f.live }),
        ));
    }
    if f.failed24h > 0 && f.sent24h == 0 {
        out.push(check(
            "all-failing",
            "warn",
            &json!({ "failed": f.failed24h, "reason": f.top_reason }),
        ));
    }

    // Not a fault — a fact worth knowing, because the two halves of an
    // Apple fleet are on different hosts.
    if f.sandbox > 0 && f.live > f.sandbox {
        out.push(check(
            "apns-mixed-env",
            "info",
            &json!({ "sandbox": f.sandbox, "live": f.live }),
        ));
    }

    out
}

#[allow(clippy::too_many_lines)] // one query per fact, each named
pub async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let internal = |e: sqlx::Error| {
        tracing::warn!(error = %e, "push.readiness query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal" })),
        )
    };

    // Live devices per provider, and how many carry each of the three
    // things a condition can select on.
    let fleet = sqlx::query(
        "SELECT provider, \
                count(*) AS live, \
                count(*) FILTER (WHERE user_key IS NOT NULL) AS identified, \
                count(*) FILTER (WHERE coalesce(traits, '{}'::jsonb) <> '{}'::jsonb) \
                  AS with_traits, \
                count(*) FILTER (WHERE coalesce(metadata, '{}'::jsonb) <> '{}'::jsonb) \
                  AS with_metadata, \
                count(*) FILTER (WHERE provider = 'apns' AND env = 'sandbox') AS sandbox \
         FROM device_tokens \
         WHERE project_id = $1 AND revoked_at IS NULL \
         GROUP BY provider",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await
    .map_err(internal)?;

    let quarantined: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM device_tokens \
         WHERE project_id = $1 AND revoked_at IS NOT NULL",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(internal)?;

    let credentials: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM push_credentials WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(&state.pool)
            .await
            .map_err(internal)?;

    let queue = sqlx::query(
        "SELECT count(*) AS queued, \
                coalesce(extract(epoch FROM now() - min(created_at)) / 60, 0)::bigint \
                  AS oldest_minutes \
         FROM push_sends WHERE project_id = $1 AND status = 'queued'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(internal)?;

    let recent = sqlx::query(
        "SELECT count(*) FILTER (WHERE status = 'sent') AS sent, \
                count(*) FILTER (WHERE status = 'failed') AS failed, \
                (SELECT coalesce(nullif(error, ''), provider_outcome, 'unknown') \
                 FROM push_sends \
                 WHERE project_id = $1 AND status = 'failed' \
                   AND created_at > now() - interval '24 hours' \
                 GROUP BY 1 ORDER BY count(*) DESC LIMIT 1) AS top_reason \
         FROM push_sends \
         WHERE project_id = $1 AND created_at > now() - interval '24 hours'",
    )
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .map_err(internal)?;

    // `try_get`, and a missing column is an error rather than a zero.
    //
    // `Row::get` panics on a column the result does not carry, which
    // is how this endpoint took a tokio worker down against a backend
    // whose Describe under-reported a SELECT list containing a
    // subquery. Falling back to `0` would have been worse than the
    // panic in one way: readiness would then report "nothing sent in
    // 24h" — a claim about the customer's push traffic that we could
    // not support — and the console would draw it as a healthy zero.
    let col =
        |r: &sqlx::postgres::PgRow, name: &'static str| -> Result<i64, (StatusCode, Json<Value>)> {
            r.try_get::<i64, _>(name).map_err(|e| {
                tracing::warn!(column = name, error = %e,
                "push readiness: column missing from the result");
                internal(e)
            })
        };
    let facts = Facts {
        live: fleet
            .iter()
            .filter_map(|r| r.try_get::<i64, _>("live").ok())
            .sum(),
        per_provider: fleet
            .iter()
            .filter_map(|r| {
                Some((
                    r.try_get::<String, _>("provider").ok()?,
                    r.try_get::<i64, _>("live").ok()?,
                ))
            })
            .collect(),
        identified: fleet
            .iter()
            .filter_map(|r| r.try_get::<i64, _>("identified").ok())
            .sum(),
        with_traits: fleet
            .iter()
            .filter_map(|r| r.try_get::<i64, _>("with_traits").ok())
            .sum(),
        with_metadata: fleet
            .iter()
            .filter_map(|r| r.try_get::<i64, _>("with_metadata").ok())
            .sum(),
        sandbox: fleet
            .iter()
            .filter_map(|r| r.try_get::<i64, _>("sandbox").ok())
            .sum(),
        quarantined,
        credentials,
        queued: col(&queue, "queued")?,
        oldest_queued_minutes: col(&queue, "oldest_minutes")?,
        sent24h: col(&recent, "sent")?,
        failed24h: col(&recent, "failed")?,
        top_reason: recent
            .try_get::<Option<String>, _>("top_reason")
            .ok()
            .flatten(),
    };

    let checks = checks_for(&facts);
    Ok(Json(json!({
        "checks": checks,
        "live": facts.live,
        "ready": !checks.iter().any(|c| c["level"] == "blocked"),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Facts {
        Facts {
            live: 10,
            per_provider: vec![("apns".into(), 10)],
            identified: 10,
            with_traits: 10,
            with_metadata: 10,
            sandbox: 0,
            quarantined: 0,
            credentials: vec!["apns".into()],
            queued: 0,
            oldest_queued_minutes: 0,
            sent24h: 5,
            failed24h: 0,
            top_reason: None,
        }
    }

    fn ids(f: &Facts) -> Vec<String> {
        checks_for(f)
            .iter()
            .filter_map(|c| c["id"].as_str().map(str::to_string))
            .collect()
    }

    /// A project that is set up says nothing. A checklist that always
    /// has something on it is a checklist nobody reads.
    #[test]
    fn a_working_project_raises_nothing() {
        assert_eq!(ids(&healthy()), Vec::<String>::new());
    }

    /// The one that started this: devices on a provider with no
    /// credential. Nothing can ever arrive, and the console showed a
    /// growing device count and a growing queue.
    #[test]
    fn devices_with_no_credential_for_them_is_blocking() {
        let mut f = healthy();
        f.per_provider = vec![("apns".into(), 10), ("fcm".into(), 300)];
        f.live = 310;
        let out = checks_for(&f);
        let blocked: Vec<&Value> = out.iter().filter(|c| c["level"] == "blocked").collect();
        assert_eq!(
            blocked.len(),
            1,
            "expected exactly one blocking check: {out:?}"
        );
        assert_eq!(blocked[0]["id"], "no-credential");
        assert_eq!(blocked[0]["data"]["provider"], "fcm");
        assert_eq!(blocked[0]["data"]["devices"], 300);
    }

    /// Every way of aiming at a person is reported separately, because
    /// each one is a different call the host is not making.
    #[test]
    fn each_missing_selector_is_its_own_line() {
        let mut f = healthy();
        f.identified = 0;
        f.with_traits = 0;
        f.with_metadata = 0;
        let out = ids(&f);
        assert!(out.contains(&"no-identity".to_string()));
        assert!(out.contains(&"no-traits".to_string()));
        assert!(out.contains(&"no-metadata".to_string()));
    }

    /// A queue nothing is draining is not a slow queue.
    #[test]
    fn a_stalled_queue_is_blocking_but_a_fresh_one_is_not() {
        let mut f = healthy();
        f.queued = 12;
        f.oldest_queued_minutes = 1;
        assert_eq!(ids(&f), Vec::<String>::new());
        f.oldest_queued_minutes = STALLED_MINUTES;
        assert_eq!(ids(&f), vec!["queue-stalled".to_string()]);
    }

    /// More devices retired than alive is a setting, not attrition —
    /// but a project with three devices and five retirements is a
    /// project that is still starting up.
    #[test]
    fn mass_quarantine_needs_both_a_ratio_and_a_number() {
        let mut f = healthy();
        f.quarantined = 4;
        f.live = 1;
        f.per_provider = vec![("apns".into(), 1)];
        assert!(!ids(&f).contains(&"mass-quarantine".to_string()));
        f.quarantined = 40;
        assert!(ids(&f).contains(&"mass-quarantine".to_string()));
    }

    /// An all-Apple-sandbox fleet is a dev project, not a mixed one.
    #[test]
    fn a_fleet_that_is_entirely_sandbox_is_not_mixed() {
        let mut f = healthy();
        f.sandbox = 10;
        assert!(!ids(&f).contains(&"apns-mixed-env".to_string()));
        f.sandbox = 4;
        assert!(ids(&f).contains(&"apns-mixed-env".to_string()));
    }
}
