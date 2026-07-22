//! Background endpoint probe runner.
//!
//! Polls active endpoint_check rows whose last probe is older
//! than `interval_sec`, issues an HTTP request, records the
//! outcome in the `endpoint_probe` time-partitioned table.
//! Schema per migration 0029.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::Row;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub fn spawn(pool: PgPool) {
    if !env_enabled() {
        info!("probe worker disabled via SENTORI_PROBE_WORKER_ENABLED");
        return;
    }
    let interval = env_interval();
    let batch = env_batch();
    tokio::spawn(async move {
        info!(
            interval_sec = interval.as_secs(),
            batch, "probe worker started"
        );
        loop {
            match run_due(&pool, batch).await {
                Ok(0) => debug!("probe worker idle"),
                Ok(n) => info!(ran = n, "probe worker batch"),
                Err(e) => warn!(error = %e, "probe worker batch failed"),
            }
            sleep(interval).await;
        }
    });
}

async fn run_due(pool: &PgPool, batch: usize) -> Result<usize, sqlx::Error> {
    // Due = not paused AND latest endpoint_probe.ts older than now - interval
    let rows = sqlx::query(
        "WITH last AS ( \
            SELECT check_id, MAX(ts) AS last_ts FROM endpoint_probe GROUP BY check_id \
         ) \
         SELECT ec.id, ec.target_url, ec.method, ec.assertion_status_codes, \
                ec.assertion_max_latency_ms, ec.interval_sec \
         FROM endpoint_check ec LEFT JOIN last ON last.check_id = ec.id \
         WHERE NOT ec.paused \
           AND (last.last_ts IS NULL OR last.last_ts + (ec.interval_sec || ' seconds')::interval <= now()) \
         ORDER BY last.last_ts NULLS FIRST LIMIT $1",
    )
    // Batch size is a small operator-set constant; saturating is
    // unreachable and a clamped LIMIT is harmless regardless.
    .bind(i64::try_from(batch).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut ran = 0usize;
    for r in &rows {
        let check_id: Uuid = r.get("id");
        let target_url: String = r.get("target_url");
        let method: String = r.try_get("method").unwrap_or_else(|_| "GET".into());
        let allowed_codes: Vec<i32> = r
            .try_get::<Vec<i32>, _>("assertion_status_codes")
            .unwrap_or_else(|_| vec![200]);
        let max_latency = r
            .try_get::<Option<i32>, _>("assertion_max_latency_ms")
            .ok()
            .flatten();

        let outcome = probe_one(&target_url, &method, 5000).await;
        let (status_code, latency_ms, ok, error_kind) = match outcome {
            Ok((code, dur)) => {
                let code_ok = allowed_codes.contains(&i32::from(code));
                let latency_ok = match max_latency {
                    Some(max) => dur <= max,
                    None => true,
                };
                (i32::from(code), dur, code_ok && latency_ok, None::<String>)
            }
            Err(e) => (0, 0, false, Some(e)),
        };

        let _ = sqlx::query(
            "INSERT INTO endpoint_probe (ts, check_id, status_code, latency_ms, ok, error_kind) \
             VALUES (now(), $1, $2, $3, $4, $5)",
        )
        .bind(check_id)
        .bind(status_code)
        .bind(latency_ms)
        .bind(ok)
        .bind(error_kind.as_deref())
        .execute(pool)
        .await;
        ran += 1;
    }
    Ok(ran)
}

async fn probe_one(url: &str, method: &str, timeout_ms: u64) -> Result<(u16, i32), String> {
    use std::time::Instant;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| e.to_string())?;
    let req = match method {
        "HEAD" => client.head(url),
        "POST" => client.post(url),
        _ => client.get(url),
    };
    let start = Instant::now();
    let resp = req.send().await.map_err(|e| e.to_string())?;
    // Elapsed millis of a single probe; saturates only after ~24
    // days, far beyond the client timeout.
    let dur = i32::try_from(start.elapsed().as_millis()).unwrap_or(i32::MAX);
    Ok((resp.status().as_u16(), dur))
}

fn env_enabled() -> bool {
    matches!(
        std::env::var("SENTORI_PROBE_WORKER_ENABLED")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase),
        Some(s) if s == "1" || s == "true"
    ) || std::env::var("SENTORI_PROBE_WORKER_ENABLED").is_err()
}

fn env_interval() -> Duration {
    let secs = std::env::var("SENTORI_PROBE_POLL_INTERVAL_SEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(10);
    Duration::from_secs(secs)
}

fn env_batch() -> usize {
    std::env::var("SENTORI_PROBE_BATCH")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
}
