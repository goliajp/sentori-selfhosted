//! Backend availability probe.
//!
//! The integrator writes `backendHealthUrl` into `sentori.init()`;
//! the SDK's batch envelope carries it here and the project row
//! remembers it. This worker GETs each configured URL once a
//! minute, records ok / status / latency, and keeps one rolling
//! day of results — the project health endpoint turns that into an
//! uptime figure.
//!
//! The URL is owner-configured on a self-hosted, single-tenant
//! instance; the probe is a plain GET with a hard timeout and no
//! body retention.

use sqlx::{PgPool, Row};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

const INTERVAL: Duration = Duration::from_mins(1);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(PROBE_TIMEOUT)
            .user_agent("sentori-backend-check")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "backend check client build failed — probes disabled");
                return;
            }
        };
        loop {
            run_cycle(&pool, &client).await;
            tokio::time::sleep(INTERVAL).await;
        }
    });
}

async fn run_cycle(pool: &PgPool, client: &reqwest::Client) {
    let rows = sqlx::query(
        "SELECT id, backend_health_url FROM projects WHERE backend_health_url IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for r in rows {
        let project_id: Uuid = r.get("id");
        let url: String = r.get("backend_health_url");
        let started = Instant::now();
        let outcome = client.get(&url).send().await;
        let latency = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
        let (ok, status) = match outcome {
            Ok(resp) => {
                let code = resp.status();
                (code.is_success(), Some(i32::from(code.as_u16())))
            }
            Err(_) => (false, None),
        };
        let write = sqlx::query(
            "INSERT INTO backend_checks (project_id, ok, status_code, latency_ms) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(project_id)
        .bind(ok)
        .bind(status)
        .bind(latency)
        .execute(pool)
        .await;
        if let Err(e) = write {
            warn!(%project_id, error = %e, "backend check write failed");
        }
        debug!(%project_id, ok, latency, "backend check");
    }
    // One rolling day is the whole retention story.
    let _ =
        sqlx::query("DELETE FROM backend_checks WHERE checked_at < now() - interval '24 hours'")
            .execute(pool)
            .await;
}
