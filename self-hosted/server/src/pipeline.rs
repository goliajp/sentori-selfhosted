//! The five-kind ingest pipeline (design.md §2).
//!
//! One event comes in; this module decides which issue it belongs
//! to, whether that issue just regressed, and what the objective
//! importance counters become. Cement-first: this logic lives in
//! the server (not a crate) until a second consumer exists.
//!
//! ## Fingerprint rules (per kind)
//!
//! - `error`  — exception type + the in-app frames of the
//!   symbolicated stack (file:function). Symbolication runs BEFORE
//!   fingerprinting, which is what makes groups stable across
//!   releases — a minified column shift must not mint a new issue.
//! - `warn`   — name (scenario for detected, free name for
//!   hand-written) + surface.screen + surface.element.
//! - `trace` / `assert` — name.
//! - `probe`  — ref.
//!
//! ## Regression (release-anchored)
//!
//! A resolved issue reopens only if the recurrence is in a release
//! at least as new as `resolved_in_release` (design.md §11). "As
//! new as" orders by `releases.created_at` when both sides are
//! registered releases; when either side is unregistered the
//! fallback is `occurred_at > resolved_at` — time is a weaker
//! anchor than release identity, but it never lets a stale build's
//! long tail reopen a fix.
//!
//! ## Breadth × depth
//!
//! `issue_user_hits` gets one upsert per (issue, user); the issues
//! row denormalizes `users_count` (breadth) and `max_per_user`
//! (depth) so the Inbox never aggregates. Events without a user key
//! count only toward `event_count`.

use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// The five kinds. The enum IS the concept model — no severity
/// dimension anywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Error,
    Warn,
    Trace,
    Assert,
    Probe,
}

impl Kind {
    #[must_use]
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "trace" => Some(Self::Trace),
            "assert" => Some(Self::Assert),
            "probe" => Some(Self::Probe),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Trace => "trace",
            Self::Assert => "assert",
            Self::Probe => "probe",
        }
    }
}

/// One event after wire parsing + symbolication, ready to persist.
#[derive(Debug)]
pub struct IncomingEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: Kind,
    pub platform: String,
    pub occurred_at: OffsetDateTime,
    pub release: String,
    pub environment: String,
    /// warn/trace/assert name; probe ref; ignored for error.
    pub name: Option<String>,
    /// warn surface ({screen, element, ...}); empty object otherwise.
    pub surface: Value,
    /// Salted identity hash — drives breadth × depth.
    pub user_key: Option<String>,
    /// The full payload as stored (already symbolicated).
    pub payload: Value,
}

pub struct IngestOutcome {
    pub event_id: Uuid,
    pub issue_id: Uuid,
    pub is_new_issue: bool,
    pub regressed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid event: {0}")]
    Invalid(&'static str),
}

/// Group title + fingerprint input, derived per kind.
pub fn group_identity(ev: &IncomingEvent) -> Result<(String, String, String), IngestError> {
    match ev.kind {
        Kind::Error => {
            let err = ev.payload.get("error");
            let etype = err
                .and_then(|e| e.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("Error");
            let message = err
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("");
            // In-app frames of the (already symbolicated) stack.
            let frames = err.and_then(|e| e.get("stack")).and_then(Value::as_array);
            let mut sig = String::new();
            if let Some(frames) = frames {
                for f in frames {
                    if f.get("inApp").and_then(Value::as_bool) == Some(true) {
                        let file = f.get("file").and_then(Value::as_str).unwrap_or("?");
                        let func = f.get("function").and_then(Value::as_str).unwrap_or("?");
                        sig.push_str(file);
                        sig.push(':');
                        sig.push_str(func);
                        sig.push('\n');
                    }
                }
            }
            // No in-app frames (dev-client bundles mark everything
            // out-of-app): function names survive bundle line drift
            // where file URLs and line numbers don't.
            if sig.is_empty()
                && let Some(frames) = frames
            {
                for f in frames {
                    if let Some(func) = f.get("function").and_then(Value::as_str) {
                        sig.push_str(func);
                        sig.push('\n');
                    }
                }
            }
            // Last resort: type + message — with volatile fragments
            // (timestamps, ids, counts) collapsed, or a message that
            // embeds a timestamp opens a fresh issue per occurrence.
            let fp_input = if sig.is_empty() {
                format!("error\x1f{etype}\x1f{}", collapse_numbers(message))
            } else {
                format!("error\x1f{etype}\x1f{sig}")
            };
            Ok((etype.to_string(), message.to_string(), fp_input))
        }
        Kind::Warn => {
            let name = ev
                .name
                .as_deref()
                .ok_or(IngestError::Invalid("warn requires name"))?;
            let screen = ev
                .surface
                .get("screen")
                .and_then(Value::as_str)
                .unwrap_or("");
            let element = ev
                .surface
                .get("element")
                .and_then(Value::as_str)
                .unwrap_or("");
            Ok((
                name.to_string(),
                String::new(),
                format!("warn\x1f{name}\x1f{screen}\x1f{element}"),
            ))
        }
        Kind::Trace | Kind::Assert => {
            let name = ev
                .name
                .as_deref()
                .ok_or(IngestError::Invalid("trace/assert requires name"))?;
            Ok((
                name.to_string(),
                String::new(),
                format!("{}\x1f{name}", ev.kind.as_db_str()),
            ))
        }
        Kind::Probe => {
            let r = ev
                .name
                .as_deref()
                .ok_or(IngestError::Invalid("probe requires ref"))?;
            Ok((r.to_string(), String::new(), format!("probe\x1f{r}")))
        }
    }
}

/// The grouping key. Environment and platform split the
/// aggregation: a staging occurrence is not the production case,
/// and an iOS error is not the Android one. Release deliberately
/// does NOT — the resolve/regression narrative is cross-release.
/// Shared by ingest and the backfill-split tool so both derive the
/// exact same key.
pub fn compute_fingerprint(environment: &str, platform: &str, fp_input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(environment.as_bytes());
    h.update(b"\x1f");
    h.update(platform.as_bytes());
    h.update(b"\x1f");
    h.update(fp_input.as_bytes());
    // 16 bytes / 32 hex chars — grouping key, not a secret.
    hex::encode(&h.finalize()[..16])
}

/// Replace every digit run with `#` so messages that differ only in
/// volatile fragments (timestamps, ids, counts) group together.
fn collapse_numbers(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut in_digits = false;
    for c in msg.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
                in_digits = true;
            }
        } else {
            in_digits = false;
            out.push(c);
        }
    }
    out
}

/// Persist one event: find-or-create its issue (with atomic
/// regression detection), bump the importance counters, insert the
/// event row, and — for probes — update the tripwire registry.
// One transaction, one narrative: splitting the steps into helpers
// would hide the ordering the row locks depend on.
#[allow(clippy::too_many_lines)]
pub async fn ingest(pool: &PgPool, ev: IncomingEvent) -> Result<IngestOutcome, IngestError> {
    let (group_title, message_sample, fp_input) = group_identity(&ev)?;
    let fingerprint = compute_fingerprint(&ev.environment, &ev.platform, &fp_input);

    let mut tx = pool.begin().await?;

    // Lock-or-create the issue row.
    let existing: Option<(Uuid, String, Option<String>, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, status, resolved_in_release, resolved_at FROM issues \
             WHERE project_id = $1 AND fingerprint = $2 FOR UPDATE",
    )
    .bind(ev.project_id)
    .bind(&fingerprint)
    .fetch_optional(&mut *tx)
    .await?;

    let (issue_id, is_new_issue, regressed) = match existing {
        None => {
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO issues \
                 (id, project_id, fingerprint, kind, group_title, message_sample, surface, \
                  status, first_seen, last_seen, event_count, last_environment, last_release, \
                  environment, platform) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 'open', $8, $8, 0, $9, $10, $9, $11)",
            )
            .bind(id)
            .bind(ev.project_id)
            .bind(&fingerprint)
            .bind(ev.kind.as_db_str())
            .bind(&group_title)
            .bind(&message_sample)
            .bind(&ev.surface)
            .bind(ev.occurred_at)
            .bind(&ev.environment)
            .bind(&ev.release)
            .bind(&ev.platform)
            .execute(&mut *tx)
            .await?;
            (id, true, false)
        }
        Some((id, status, resolved_in, resolved_at)) => {
            let regress = status == "resolved"
                && is_regression(
                    &mut tx,
                    ev.project_id,
                    &ev.release,
                    resolved_in.as_deref(),
                    resolved_at,
                    ev.occurred_at,
                )
                .await?;
            if regress {
                sqlx::query(
                    "UPDATE issues SET status = 'open', regressed_at = now(), \
                     regressed_in_release = $2, last_seen = $3, \
                     last_environment = $4, last_release = $5 WHERE id = $1",
                )
                .bind(id)
                .bind(&ev.release)
                .bind(ev.occurred_at)
                .bind(&ev.environment)
                .bind(&ev.release)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO issue_activity (id, issue_id, kind, body) \
                     VALUES ($1, $2, 'regression', $3)",
                )
                .bind(Uuid::now_v7())
                .bind(id)
                .bind(serde_json::json!({
                    "in_release": ev.release,
                    "trigger": if ev.kind == Kind::Probe { "probe" } else { "recurrence" },
                }))
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "UPDATE issues SET last_seen = GREATEST(last_seen, $2), \
                     last_environment = $3, last_release = $4 WHERE id = $1",
                )
                .bind(id)
                .bind(ev.occurred_at)
                .bind(&ev.environment)
                .bind(&ev.release)
                .execute(&mut *tx)
                .await?;
            }
            (id, false, regress)
        }
    };

    // Importance counters. event_count always; breadth/depth only
    // with a user key.
    sqlx::query("UPDATE issues SET event_count = event_count + 1 WHERE id = $1")
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
    if let Some(user_key) = &ev.user_key {
        let (hits,): (i64,) = sqlx::query_as(
            "INSERT INTO issue_user_hits (issue_id, user_key, hit_count, last_hit) \
             VALUES ($1, $2, 1, now()) \
             ON CONFLICT (issue_id, user_key) \
             DO UPDATE SET hit_count = issue_user_hits.hit_count + 1, last_hit = now() \
             RETURNING hit_count",
        )
        .bind(issue_id)
        .bind(user_key)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issues SET \
             users_count = (SELECT COUNT(*) FROM issue_user_hits WHERE issue_id = $1), \
             max_per_user = GREATEST(max_per_user, $2) \
             WHERE id = $1",
        )
        .bind(issue_id)
        .bind(hits)
        .execute(&mut *tx)
        .await?;
    }

    // The event row itself.
    //
    // `DO NOTHING` rather than a plain insert, because a mobile SDK
    // retrying a request whose response was lost sends the same event
    // id again — and that is the ordinary case, not the pathological
    // one. A primary-key violation surfaced as `500 ingest_failed`,
    // which our own contract tells the SDK to retry with backoff: it
    // failed identically three more times and logged a dropped batch,
    // for an event the server had already stored. Every counter this
    // transaction touched is rolled back below when the row was
    // already there.
    let inserted = sqlx::query(
        "INSERT INTO events \
         (id, project_id, issue_id, kind, platform, occurred_at, release, environment, \
          user_key, payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(ev.id)
    .bind(ev.project_id)
    .bind(issue_id)
    .bind(ev.kind.as_db_str())
    .bind(&ev.platform)
    .bind(ev.occurred_at)
    .bind(&ev.release)
    .bind(&ev.environment)
    .bind(ev.user_key.as_deref())
    .bind(&ev.payload)
    .execute(&mut *tx)
    .await?;

    if inserted.rows_affected() == 0 {
        // Already have it. Drop everything this transaction did — the
        // issue's counters, the user hit, the assert tally — and
        // report the event as accepted, because it is.
        tx.rollback().await?;
        // Scoped to the project. `events.id` is globally unique, so a
        // client-chosen id that happens to belong to a DIFFERENT
        // project also lands in DO NOTHING — and this read, when it
        // carried no project predicate, handed that other project's
        // `issue_id` back in a 202 while the caller's event was never
        // stored. Silent data loss for them, and a probe for anyone
        // willing to guess uuids.
        let issue_id: Option<Uuid> =
            sqlx::query_scalar("SELECT issue_id FROM events WHERE id = $1 AND project_id = $2")
                .bind(ev.id)
                .bind(ev.project_id)
                .fetch_optional(pool)
                .await?;
        let Some(issue_id) = issue_id else {
            // The id exists, but not here. Refusing is the honest
            // answer: we did not store it, so we must not answer 202.
            // The message names no project and confirms nothing about
            // what the id belongs to — a caller who guessed learns
            // only that this id is unusable, which is also true of an
            // id it collided with inside its own project.
            return Err(IngestError::Invalid(
                "event id is already in use; omit `id` and let the server assign one",
            ));
        };
        return Ok(IngestOutcome {
            event_id: ev.id,
            issue_id,
            is_new_issue: false,
            regressed: false,
        });
    }

    // Probe registry: firing updates the tripwire row (creating it
    // if the CLI scan never saw this ref), and links the guarded
    // issue if one is attached.
    if ev.kind == Kind::Probe
        && let Some(r) = &ev.name
    {
        sqlx::query(
            "INSERT INTO probes (id, project_id, ref, last_seen_release, last_fired_at, fire_count) \
             VALUES ($1, $2, $3, $4, now(), 1) \
             ON CONFLICT (project_id, ref) \
             DO UPDATE SET last_fired_at = now(), fire_count = probes.fire_count + 1, \
                           last_seen_release = EXCLUDED.last_seen_release",
        )
        .bind(Uuid::now_v7())
        .bind(ev.project_id)
        .bind(r)
        .bind(&ev.release)
        .execute(&mut *tx)
        .await?;
        // A probe guarding a specific issue reopens THAT issue too
        // (the probe's own group already reopened above if resolved).
        let guarded: Option<(Uuid,)> = sqlx::query_as(
            "SELECT issue_id FROM probes WHERE project_id = $1 AND ref = $2 AND issue_id IS NOT NULL",
        )
        .bind(ev.project_id)
        .bind(r)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((guarded_issue,)) = guarded
            && guarded_issue != issue_id
        {
            let flipped = sqlx::query(
                "UPDATE issues SET status = 'open', regressed_at = now(), \
                 regressed_in_release = $2, last_seen = now() \
                 WHERE id = $1 AND status = 'resolved'",
            )
            .bind(guarded_issue)
            .bind(&ev.release)
            .execute(&mut *tx)
            .await?;
            if flipped.rows_affected() > 0 {
                sqlx::query(
                    "INSERT INTO issue_activity (id, issue_id, kind, body) \
                     VALUES ($1, $2, 'regression', $3)",
                )
                .bind(Uuid::now_v7())
                .bind(guarded_issue)
                .bind(serde_json::json!({
                    "in_release": ev.release,
                    "trigger": "probe",
                    "probe_ref": r,
                }))
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(IngestOutcome {
        event_id: ev.id,
        issue_id,
        is_new_issue,
        regressed,
    })
}

/// Release-anchored regression check (see module docs).
async fn is_regression(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    event_release: &str,
    resolved_in: Option<&str>,
    resolved_at: Option<OffsetDateTime>,
    occurred_at: OffsetDateTime,
) -> Result<bool, sqlx::Error> {
    if let Some(resolved_release) = resolved_in
        && !event_release.is_empty()
    {
        let pair: Vec<(String, OffsetDateTime)> = sqlx::query_as(
            "SELECT name, created_at FROM releases \
             WHERE project_id = $1 AND name IN ($2, $3)",
        )
        .bind(project_id)
        .bind(event_release)
        .bind(resolved_release)
        .fetch_all(&mut **tx)
        .await?;
        let ev_at = pair
            .iter()
            .find(|(n, _)| n == event_release)
            .map(|(_, t)| *t);
        let res_at = pair
            .iter()
            .find(|(n, _)| n == resolved_release)
            .map(|(_, t)| *t);
        if let (Some(ev_at), Some(res_at)) = (ev_at, res_at) {
            // Both registered: the release axis decides. Same
            // release counts — "fixed in X" broken by X itself is
            // exactly the regression the anchor exists to catch.
            return Ok(ev_at >= res_at);
        }
    }
    // Unregistered release(s): weak time anchor.
    Ok(resolved_at.is_some_and(|t| occurred_at > t))
}

/// Batch-envelope assert aggregates (design.md §2): passes are
/// counted client-side and shipped as totals, never as events.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssertStat {
    pub name: String,
    #[serde(default)]
    pub release: String,
    pub pass_delta: i64,
    #[serde(default)]
    pub fail_delta: i64,
}

pub async fn record_assert_stats(
    pool: &PgPool,
    project_id: Uuid,
    stats: &[AssertStat],
) -> Result<(), sqlx::Error> {
    for s in stats {
        sqlx::query(
            // The casts are not fixing a bug — they are removing a
            // dependency on the driver.
            //
            // `$4` appears both as a `bigint` column value and inside
            // `$4 > 0`, where the literal is an `integer`. Asked to
            // *deduce* the parameter, PostgreSQL refuses the
            // statement outright: "inconsistent types deduced …
            // integer versus bigint". Asked to *accept* a declared
            // type it is fine, and sqlx declares one — it sends
            // `int8` in Parse because the bound value is an `i64`. So
            // this has always worked here, and a bare `PREPARE` of the
            // same text has always failed.
            //
            // Made explicit because that difference is invisible from
            // the statement. Anything that reads it without binding —
            // psql, a migration tool, a schema linter, another
            // driver's inference — sees a statement PostgreSQL will
            // not take. spg's harness Described all 211 of ours
            // against PG18 and this was one of two it flagged; the
            // flag was right about the text even though nothing was
            // broken.
            "INSERT INTO assert_stats \
             (project_id, name, release, pass_count, fail_count, last_pass_at, last_fail_at) \
             VALUES ($1, $2, $3, $4::bigint, $5::bigint, \
                     CASE WHEN $4::bigint > 0 THEN now() END, \
                     CASE WHEN $5::bigint > 0 THEN now() END) \
             ON CONFLICT (project_id, name, release) DO UPDATE SET \
             pass_count = assert_stats.pass_count + $4::bigint, \
             fail_count = assert_stats.fail_count + $5::bigint, \
             last_pass_at = CASE WHEN $4::bigint > 0 THEN now() \
                                 ELSE assert_stats.last_pass_at END, \
             last_fail_at = CASE WHEN $5::bigint > 0 THEN now() \
                                 ELSE assert_stats.last_fail_at END",
        )
        .bind(project_id)
        .bind(&s.name)
        .bind(&s.release)
        .bind(s.pass_delta.max(0))
        .bind(s.fail_delta.max(0))
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn error_event(message: &str, in_app: bool) -> IncomingEvent {
        IncomingEvent {
            id: uuid::Uuid::now_v7(),
            project_id: uuid::Uuid::now_v7(),
            kind: Kind::Error,
            occurred_at: time::OffsetDateTime::now_utc(),
            platform: "ios".into(),
            release: "t@1".into(),
            environment: "test".into(),
            name: None,
            surface: json!({}),
            user_key: None,
            payload: json!({
                "error": {
                    "type": "Error",
                    "message": message,
                    "stack": [
                        { "file": "http://192.168.0.2:8081/bundle", "function": "boom", "line": 721_422, "inApp": in_app },
                        { "file": "http://192.168.0.2:8081/bundle", "function": "press", "line": 721_001, "inApp": in_app }
                    ]
                }
            }),
        }
    }

    #[test]
    fn dev_bundle_stacks_group_despite_timestamped_messages() -> Result<(), IngestError> {
        let a = group_identity(&error_event("smoke @ 2026-07-31T16:24:08.939Z", false))?;
        let b = group_identity(&error_event("smoke @ 2026-07-31T16:25:47.573Z", false))?;
        assert_eq!(a.2, b.2, "same stack shape must share a fingerprint");
        Ok(())
    }

    #[test]
    fn stackless_messages_collapse_volatile_numbers() -> Result<(), IngestError> {
        let mk = |msg: &str| IncomingEvent {
            payload: json!({ "error": { "type": "Error", "message": msg } }),
            ..error_event(msg, false)
        };
        let a = group_identity(&mk("timeout after 1500ms"))?;
        let b = group_identity(&mk("timeout after 3200ms"))?;
        assert_eq!(a.2, b.2);
        Ok(())
    }

    #[test]
    fn in_app_frames_still_take_priority() -> Result<(), IngestError> {
        let a = group_identity(&error_event("m1", true))?;
        let b = group_identity(&error_event("m2", true))?;
        assert_eq!(a.2, b.2);
        Ok(())
    }
}
