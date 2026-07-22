//! Best-effort alert dispatcher fired by ingest paths when a new
//! issue is created or an existing one regresses.
//!
//! Finds enabled alert_rules with matching trigger_kind that have
//! cleared their throttle window, fires their `webhook`/`slack`
//! channels via crate::webhook::deliver, and stamps last_fired_at.
//!
//! Runs in a tokio::spawn so the ingest hot-path is not blocked.

use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum TriggerKind {
    IssueNew,
    Regression,
    EventCount,
}

impl TriggerKind {
    fn as_str(self) -> &'static str {
        match self {
            TriggerKind::IssueNew => "new_issue",
            TriggerKind::Regression => "regression",
            TriggerKind::EventCount => "event_count",
        }
    }
}

pub fn fire_async(
    pool: PgPool,
    workspace_id: Uuid,
    project_id: Uuid,
    issue_id: Uuid,
    kind: TriggerKind,
) {
    tokio::spawn(async move {
        if let Err(e) = fire(&pool, workspace_id, project_id, issue_id, kind).await {
            warn!(error = %e, "alert auto-fire failed");
        }
    });
}

// Splitting this would mean threading the notification state through
// new signatures; deferred rather than done inside a lint pass.
#[allow(clippy::too_many_lines)]
async fn fire(
    pool: &PgPool,
    workspace_id: Uuid,
    project_id: Uuid,
    issue_id: Uuid,
    kind: TriggerKind,
) -> Result<(), sqlx::Error> {
    // Pull the issue title for the payload.
    let issue_title: String = sqlx::query(
        "SELECT COALESCE(error_type, message_sample, 'Issue') AS title \
         FROM issues WHERE id = $1",
    )
    .bind(issue_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.try_get::<String, _>("title").ok())
    .unwrap_or_else(|| "Issue".into());
    // Pull matching rules. Throttle: last_fired_at + throttle_minutes
    // ago < now() OR last_fired_at IS NULL.
    let rows = sqlx::query(
        "SELECT id, name, channels, throttle_minutes, trigger_config \
         FROM alert_rules \
         WHERE workspace_id = $1 \
           AND enabled = TRUE \
           AND COALESCE(muted, FALSE) = FALSE \
           AND trigger_kind = $2 \
           AND ( \
                last_fired_at IS NULL \
                OR last_fired_at + (throttle_minutes || ' minutes')::interval <= now() \
           )",
    )
    .bind(workspace_id)
    .bind(kind.as_str())
    .fetch_all(pool)
    .await?;

    for r in &rows {
        let alert_id: Uuid = r.get("id");
        let name: String = r.get("name");
        let channels: Value = r.get("channels");
        let trigger_config: Value = r.try_get("trigger_config").unwrap_or(Value::Null);

        // event_count rule: only fire if issue's total event count >= threshold
        // (defaults to 100). For new_issue / regression rules: no extra check.
        if matches!(kind, TriggerKind::EventCount) {
            let threshold = trigger_config
                .get("count")
                .or_else(|| trigger_config.get("threshold"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(100);
            let cur: Option<(i64,)> =
                sqlx::query_as("SELECT event_count FROM issues WHERE id = $1")
                    .bind(issue_id)
                    .fetch_optional(pool)
                    .await?;
            let count = cur.map_or(0, |t| t.0);
            if count < threshold {
                continue;
            }
        }

        let arr = channels.as_array().cloned().unwrap_or_default();
        let mut delivered = 0usize;
        let mut per_channel = Vec::<Value>::new();
        for ch in &arr {
            let ch_kind = ch.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if ch_kind != "webhook" && ch_kind != "slack" {
                continue;
            }
            let Some(url) = ch.get("url").and_then(|v| v.as_str()) else {
                continue;
            };
            let secret = ch.get("secret").and_then(|v| v.as_str());
            let payload = json!({
                "type": kind.as_str(),
                "alert_id": alert_id.to_string(),
                "alert_name": name,
                "workspace_id": workspace_id.to_string(),
                "project_id": project_id.to_string(),
                "issue_id": issue_id.to_string(),
                "issue_title": issue_title,
            });
            // Truncate URL for the log entry (don't leak secrets via
            // query-string-tokenized Slack URLs).
            let host_only = url
                .split("://")
                .nth(1)
                .and_then(|h| h.split('/').next())
                .unwrap_or(url);
            match crate::webhook::deliver(url, secret, &payload).await {
                Ok(status) => {
                    delivered += 1;
                    per_channel.push(json!({
                        "host": host_only,
                        "ok": true,
                        "status": status,
                    }));
                }
                Err(e) => {
                    per_channel.push(json!({
                        "host": host_only,
                        "ok": false,
                        "error": e.to_string().chars().take(120).collect::<String>(),
                    }));
                }
            }
        }
        let _ = sqlx::query("UPDATE alert_rules SET last_fired_at = now() WHERE id = $1")
            .bind(alert_id)
            .execute(pool)
            .await;
        crate::notify::audit(
            pool,
            workspace_id,
            Some(project_id),
            None,
            &format!("alert.fire.{}", kind.as_str()),
            Some("alert"),
            Some(&alert_id.to_string()),
            json!({
                "delivered": delivered,
                "issue_id": issue_id.to_string(),
                "channels": per_channel,
            }),
        )
        .await;
    }
    Ok(())
}
