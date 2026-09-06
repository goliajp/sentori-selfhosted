//! Bundle assembly — the product's actual deliverable (design.md §1,
//! bundle-schema.md).
//!
//! A bundle is one issue rendered as a "read it and fix it" report:
//! markdown for LLM consumption, with a JSON twin for tool-driven
//! agents. Everything here is assembled from data already stored;
//! the optional LLM-written sections (Summary / For the AI) only
//! render when an Anthropic key is configured and are skipped
//! silently otherwise — a bundle with hard evidence and no prose
//! beats no bundle.

// Markdown assembly reads best as a linear sequence of push_str +
// format!; splitting it or switching to write! would obscure the
// document order the sections appear in.
#![allow(clippy::format_push_string, clippy::too_many_lines)]

use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct Bundle {
    pub markdown: String,
    pub json: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("issue not found")]
    NotFound,
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

pub async fn assemble(pool: &PgPool, issue_id: Uuid) -> Result<Bundle, BundleError> {
    let issue = sqlx::query("SELECT * FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_optional(pool)
        .await?
        .ok_or(BundleError::NotFound)?;

    let kind: String = issue.get("kind");
    let title: String = issue.get("group_title");
    let message: String = issue.get("message_sample");
    let status: String = issue.get("status");
    let event_count: i64 = issue.get("event_count");
    let users_count: i64 = issue.get("users_count");
    let max_per_user: i64 = issue.get("max_per_user");
    let first_seen: time::OffsetDateTime = issue.get("first_seen");
    let last_seen: time::OffsetDateTime = issue.get("last_seen");
    let last_release: String = issue.get("last_release");
    let resolved_in: Option<String> = issue.get("resolved_in_release");
    let regressed_in: Option<String> = issue.get("regressed_in_release");
    let surface: Value = issue.get("surface");
    let project_id: Uuid = issue.get("project_id");

    // The most recent occurrence carries the payload the evidence
    // sections read from.
    let latest = sqlx::query(
        "SELECT id, platform, release, environment, occurred_at, payload \
         FROM events WHERE issue_id = $1 ORDER BY received_at DESC LIMIT 1",
    )
    .bind(issue_id)
    .fetch_optional(pool)
    .await?;

    // Release distribution — is this localized to one build?
    let releases: Vec<(String, i64)> = sqlx::query_as(
        "SELECT release, COUNT(*) FROM events WHERE issue_id = $1 \
         GROUP BY release ORDER BY COUNT(*) DESC LIMIT 10",
    )
    .bind(issue_id)
    .fetch_all(pool)
    .await?;

    // Platform/OS narrow-distribution hints (diagnostic signals).
    let platforms: Vec<(String, i64)> = sqlx::query_as(
        "SELECT platform, COUNT(*) FROM events WHERE issue_id = $1 \
         GROUP BY platform ORDER BY COUNT(*) DESC",
    )
    .bind(issue_id)
    .fetch_all(pool)
    .await?;

    let activity: Vec<(String, Value, time::OffsetDateTime)> = sqlx::query_as(
        "SELECT kind, body, at FROM issue_activity WHERE issue_id = $1 \
         ORDER BY at DESC LIMIT 20",
    )
    .bind(issue_id)
    .fetch_all(pool)
    .await?;

    // Guarding probe, if any.
    let probe: Option<(String, Option<time::OffsetDateTime>, i64)> = sqlx::query_as(
        "SELECT ref, last_fired_at, fire_count FROM probes \
         WHERE project_id = $1 AND issue_id = $2",
    )
    .bind(project_id)
    .bind(issue_id)
    .fetch_optional(pool)
    .await?;

    // ── markdown ──
    let mut md = String::with_capacity(8 * 1024);
    let breadth_depth =
        format!("{users_count} users × up to {max_per_user} hits each · {event_count} events");
    md.push_str(&format!("# {kind}: {title}\n\n"));
    if !message.is_empty() {
        md.push_str(&format!("> {message}\n\n"));
    }
    md.push_str(&format!(
        "- Status: **{status}**{}\n- Impact: {breadth_depth}\n- First seen: {} · Last seen: {} · Last release: {}\n",
        regressed_in
            .as_deref()
            .map(|r| format!(" (REGRESSED in {r})"))
            .unwrap_or_default(),
        fmt_ts(first_seen),
        fmt_ts(last_seen),
        if last_release.is_empty() { "unknown" } else { &last_release },
    ));
    if let Some(r) = &resolved_in {
        md.push_str(&format!("- Previously resolved in: {r}\n"));
    }
    if surface != json!({}) {
        md.push_str(&format!("- Surface: `{surface}`\n"));
    }
    md.push('\n');

    if let Some(ev) = &latest {
        let payload: Value = ev.get("payload");
        // Stack trace (already symbolicated at ingest where a map
        // existed).
        if let Some(stack) = payload
            .get("error")
            .and_then(|e| e.get("stack"))
            .and_then(Value::as_array)
        {
            md.push_str("## Stack trace\n\n```\n");
            for (i, f) in stack.iter().enumerate().take(40) {
                let func = f.get("function").and_then(Value::as_str).unwrap_or("?");
                let file = f.get("file").and_then(Value::as_str).unwrap_or("?");
                let line = f.get("line").and_then(Value::as_i64).unwrap_or(0);
                let in_app = f.get("inApp").and_then(Value::as_bool).unwrap_or(false);
                let marker = if in_app { "→" } else { " " };
                md.push_str(&format!("{marker} {i:2}. {func}  ({file}:{line})\n"));
            }
            md.push_str("```\n\n(→ = in-app frame)\n\n");

            // Source windows for the in-app frames — the bundle's
            // whole point is that an AI (or a human) can read the
            // failing code without cloning anything. Top 3 in-app
            // frames keeps the bundle readable.
            let mut shown = 0usize;
            for f in stack {
                if shown >= 3 {
                    break;
                }
                if !f.get("inApp").and_then(Value::as_bool).unwrap_or(false) {
                    continue;
                }
                let Some(ctx_line) = f.get("contextLine").and_then(Value::as_str) else {
                    continue;
                };
                let file = f.get("file").and_then(Value::as_str).unwrap_or("?");
                let line = f.get("line").and_then(Value::as_i64).unwrap_or(0);
                let func = f.get("function").and_then(Value::as_str).unwrap_or("?");
                md.push_str(&format!("### {func} — {file}:{line}\n\n```\n"));
                let pre_len = f
                    .get("preContext")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let start = line - i64::try_from(pre_len).unwrap_or(0);
                let mut n = start;
                if let Some(pre) = f.get("preContext").and_then(Value::as_array) {
                    for l in pre {
                        md.push_str(&format!("  {n:4} | {}\n", l.as_str().unwrap_or("")));
                        n += 1;
                    }
                }
                md.push_str(&format!("> {n:4} | {ctx_line}\n"));
                n += 1;
                if let Some(post) = f.get("postContext").and_then(Value::as_array) {
                    for l in post {
                        md.push_str(&format!("  {n:4} | {}\n", l.as_str().unwrap_or("")));
                        n += 1;
                    }
                }
                md.push_str("```\n\n");
                shown += 1;
            }
        }
        // What the user was doing — the signal ring shipped with the
        // event.
        if let Some(signals) = payload.get("signals").and_then(Value::as_array)
            && !signals.is_empty()
        {
            md.push_str("## What the user was doing\n\n");
            for s in signals.iter().take(30) {
                md.push_str(&format!("- `{s}`\n"));
            }
            md.push('\n');
        }
        // Environment.
        if let Some(device) = payload.get("device") {
            md.push_str(&format!("## Environment\n\n```json\n{device:#}\n```\n\n"));
        }
    }

    md.push_str("## Distribution\n\n");
    md.push_str("| release | events |\n|---|---|\n");
    for (r, c) in &releases {
        let name = if r.is_empty() { "(none)" } else { r };
        md.push_str(&format!("| {name} | {c} |\n"));
    }
    md.push('\n');
    if platforms.len() == 1 {
        md.push_str(&format!(
            "**Narrow distribution: 100% {}** — treat as a platform-specific lead.\n\n",
            platforms[0].0
        ));
    }

    if let Some((r, fired, count)) = &probe {
        md.push_str(&format!(
            "## Guard\n\nProbe `{r}` — fired {count} times{}\n\n",
            fired.map_or_else(
                || " (silent — fix holding)".to_string(),
                |t| format!(", last {}", fmt_ts(t))
            ),
        ));
    }

    if !activity.is_empty() {
        md.push_str("## Activity\n\n");
        for (k, body, at) in &activity {
            md.push_str(&format!("- {} `{k}` {body}\n", fmt_ts(*at)));
        }
        md.push('\n');
    }

    md.push_str(
        "---\n\nWhen you fix this: plant `sentori.probe('<issue-ref>')` in the branch \
         that used to break, note the fix here (`POST /api/issues/{id}/notes`), and \
         resolve with the release that carries it (`POST /api/issues/{id}/resolve`).\n",
    );

    let json_twin = json!({
        "issue": {
            "id": issue_id,
            "kind": kind,
            "title": title,
            "messageSample": message,
            "status": status,
            "eventCount": event_count,
            "usersCount": users_count,
            "maxPerUser": max_per_user,
            "lastRelease": last_release,
            "resolvedInRelease": resolved_in,
            "regressedInRelease": regressed_in,
            "surface": surface,
        },
        "latestEvent": latest.as_ref().map(|ev| json!({
            "id": ev.get::<Uuid, _>("id"),
            "platform": ev.get::<String, _>("platform"),
            "release": ev.get::<String, _>("release"),
            "environment": ev.get::<String, _>("environment"),
            "payload": ev.get::<Value, _>("payload"),
        })),
        "releases": releases.iter().map(|(r, c)| json!({"release": r, "events": c})).collect::<Vec<_>>(),
        "platforms": platforms.iter().map(|(p, c)| json!({"platform": p, "events": c})).collect::<Vec<_>>(),
        "probe": probe.as_ref().map(|(r, fired, count)| json!({
            "ref": r,
            "fireCount": count,
            "lastFiredAt": fired.map(time::OffsetDateTime::unix_timestamp),
        })),
    });

    Ok(Bundle {
        markdown: md,
        json: json_twin,
    })
}

fn fmt_ts(t: time::OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| t.unix_timestamp().to_string())
}
