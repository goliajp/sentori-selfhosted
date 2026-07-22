//! Periodic alert evaluation worker.
//!
//! Evaluates trigger kinds that need a time-window scan (vs the
//! ingest-path fire used for new_issue / regression / event_count):
//!
//! - `crash_free_drop`: compare crash_free_rate in the current
//!   window vs the baseline window. Fire when the drop exceeds the
//!   rule's threshold.
//!
//! Runs every `SENTORI_PERIODIC_ALERT_INTERVAL_SEC` (default 300s).

use std::time::Duration;

use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::Row;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub fn spawn(pool: PgPool) {
    if !env_enabled() {
        info!("periodic alert worker disabled");
        return;
    }
    let interval = env_interval();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            "periodic alert worker started"
        );
        loop {
            match run_once(&pool).await {
                Ok(n) => {
                    if n > 0 {
                        info!(evaluated = n, "periodic alert pass");
                    } else {
                        debug!("periodic alert pass idle");
                    }
                }
                Err(e) => warn!(error = %e, "periodic alert pass failed"),
            }
            sleep(interval).await;
        }
    });
}

async fn run_once(pool: &PgPool) -> Result<usize, sqlx::Error> {
    // Pull crash_free_drop rules that have cleared their throttle.
    let rules = sqlx::query(
        "SELECT id, workspace_id, project_id, name, channels, trigger_config \
         FROM alert_rules \
         WHERE enabled = TRUE \
           AND COALESCE(muted, FALSE) = FALSE \
           AND trigger_kind = 'crash_free_drop' \
           AND ( \
                last_fired_at IS NULL \
                OR last_fired_at + (throttle_minutes || ' minutes')::interval <= now() \
           )",
    )
    .fetch_all(pool)
    .await?;
    let mut count = 0usize;
    for r in &rules {
        let alert_id: Uuid = r.get("id");
        let workspace_id: Uuid = r.get("workspace_id");
        let project_id: Option<Uuid> = r.try_get("project_id").ok();
        let name: String = r.get("name");
        let channels: Value = r.get("channels");
        let cfg: Value = r.try_get("trigger_config").unwrap_or(Value::Null);

        let window_min = cfg
            .get("windowMinutes")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(60);
        let baseline_min = cfg
            .get("baselineMinutes")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(window_min * 24);
        let drop_pct = cfg
            .get("dropPct")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(5.0);
        let min_sessions = cfg
            .get("minSessions")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(20);

        // Compute crash-free rate for current window + baseline window.
        let current = crash_free_rate(pool, project_id, window_min, 0).await?;
        let baseline = crash_free_rate(pool, project_id, baseline_min, window_min).await?;

        if current.total < min_sessions || baseline.total < min_sessions {
            continue;
        }
        let drop = baseline.rate - current.rate;
        if drop < drop_pct {
            continue;
        }

        // Drop exceeded — fire the rule.
        let payload = json!({
            "type": "crash_free_drop",
            "alert_id": alert_id.to_string(),
            "alert_name": name,
            "current_rate": current.rate,
            "baseline_rate": baseline.rate,
            "drop": drop,
            "windowMinutes": window_min,
        });
        let mut delivered = 0usize;
        let arr = channels.as_array().cloned().unwrap_or_default();
        for ch in &arr {
            let ch_kind = ch.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if ch_kind != "webhook" && ch_kind != "slack" {
                continue;
            }
            let Some(url) = ch.get("url").and_then(|v| v.as_str()) else {
                continue;
            };
            let secret = ch.get("secret").and_then(|v| v.as_str());
            if crate::webhook::deliver(url, secret, &payload).await.is_ok() {
                delivered += 1;
            }
        }
        let _ = sqlx::query("UPDATE alert_rules SET last_fired_at = now() WHERE id = $1")
            .bind(alert_id)
            .execute(pool)
            .await;
        crate::notify::audit(
            pool,
            workspace_id,
            project_id,
            None,
            "alert.fire.crash_free_drop",
            Some("alert"),
            Some(&alert_id.to_string()),
            json!({
                "delivered": delivered,
                "current_rate": current.rate,
                "baseline_rate": baseline.rate,
                "drop": drop,
            }),
        )
        .await;
        count += 1;
    }
    Ok(count)
}

struct WindowStats {
    rate: f64,
    total: i64,
}

/// Compute crash-free rate for the last `window_minutes` ending
/// `offset_minutes` before now (offset=0 → "current window").
async fn crash_free_rate(
    pool: &PgPool,
    project_id: Option<Uuid>,
    window_minutes: i64,
    offset_minutes: i64,
) -> Result<WindowStats, sqlx::Error> {
    let row = sqlx::query(
        "SELECT \
            COUNT(*) AS total, \
            COUNT(*) FILTER (WHERE status <> 'crashed') AS non_crashed \
         FROM sessions \
         WHERE ($1::uuid IS NULL OR project_id = $1) \
           AND received_at < now() - ($2 || ' minutes')::interval \
           AND received_at >= now() - (($2 + $3) || ' minutes')::interval",
    )
    .bind(project_id)
    .bind(offset_minutes)
    .bind(window_minutes)
    .fetch_one(pool)
    .await?;
    let total: i64 = row.get("total");
    let non_crashed: i64 = row.get("non_crashed");
    // Session counts within an alert window never approach 2^53, so
    // the f64 conversion is exact for every realistic input.
    #[allow(clippy::cast_precision_loss)]
    let rate = if total > 0 {
        (non_crashed as f64) / (total as f64) * 100.0
    } else {
        100.0
    };
    Ok(WindowStats { rate, total })
}

fn env_enabled() -> bool {
    matches!(
        std::env::var("SENTORI_PERIODIC_ALERT_WORKER_ENABLED")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase),
        Some(s) if s == "1" || s == "true"
    ) || std::env::var("SENTORI_PERIODIC_ALERT_WORKER_ENABLED").is_err()
}

fn env_interval() -> Duration {
    let secs = std::env::var("SENTORI_PERIODIC_ALERT_INTERVAL_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300);
    Duration::from_secs(secs)
}
