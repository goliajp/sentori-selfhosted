//! Backend availability probe.
//!
//! The integrator writes `backendHealthUrl` into `sentori.init()`;
//! the SDK's batch envelope carries it here and the project row
//! remembers it. This worker GETs each configured URL once a
//! minute, records ok / status / latency, and keeps one rolling
//! day of results — the project health endpoint turns that into an
//! uptime figure.
//!
//! SDK-supplied URLs are untrusted. Public egress validation applies to
//! every DNS answer and redirect; no ambient proxy or private target is allowed.

use futures::{StreamExt, stream};
use sqlx::{PgPool, Row};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

const INTERVAL: Duration = Duration::from_mins(1);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            run_cycle(&pool).await;
            tokio::time::sleep(INTERVAL).await;
        }
    });
}

async fn run_cycle(pool: &PgPool) {
    let rows = sqlx::query(
        "SELECT p.id,p.backend_health_url FROM projects p LEFT JOIN LATERAL \
         (SELECT checked_at FROM backend_checks WHERE project_id=p.id ORDER BY checked_at DESC LIMIT 1) c ON true \
         WHERE p.backend_health_url IS NOT NULL AND (c.checked_at IS NULL OR c.checked_at < now()-interval '1 minute') \
         ORDER BY c.checked_at NULLS FIRST,p.id LIMIT 32",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    stream::iter(rows)
        .for_each_concurrent(4, |r| async move {
            let project_id: Uuid = r.get("id");
            let url: String = r.get("backend_health_url");
            let started = Instant::now();
            let outcome = crate::public_http::probe(&url, PROBE_TIMEOUT).await;
            let latency = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
            let (ok, status) = match outcome {
                Ok(code) => ((200..300).contains(&code), Some(i32::from(code))),
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
        })
        .await;
    // One rolling day is the whole retention story.
    let _ =
        sqlx::query("DELETE FROM backend_checks WHERE checked_at < now() - interval '24 hours'")
            .execute(pool)
            .await;
}
