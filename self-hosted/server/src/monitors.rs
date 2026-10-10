//! Four bounded probe workers. Leases survive crashes; configuration/generation fence late results.
use sqlx::{PgPool, Row};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn health(ok: bool, failures: i32, threshold: i32) -> (&'static str, i32) {
    if ok {
        ("operational", 0)
    } else {
        let failures = (failures + 1).min(10);
        (
            if failures >= threshold {
                "down"
            } else {
                "degraded"
            },
            failures,
        )
    }
}

async fn cycle(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let token = Uuid::now_v7();
    let row = sqlx::query("WITH due AS (SELECT m.id FROM monitors m JOIN instance_settings s ON s.singleton WHERE m.enabled AND m.deleted_at IS NULL AND m.generation=s.generation AND m.next_check_at<=now() AND (m.lease_until IS NULL OR m.lease_until<now()) ORDER BY m.next_check_at,m.id LIMIT 1 FOR UPDATE OF m SKIP LOCKED) UPDATE monitors m SET lease_owner=$1,lease_until=now()+interval '45 seconds' FROM due WHERE m.id=due.id RETURNING m.*")
        .bind(token).fetch_optional(pool).await?;
    let Some(row) = row else { return Ok(false) };
    let id: Uuid = row.get("id");
    let started = Instant::now();
    let timeout: i32 = row.get("timeout_seconds");
    let result = crate::public_http::probe(
        &row.get::<String, _>("url"),
        Duration::from_secs(u64::try_from(timeout).unwrap_or(30)),
    )
    .await;
    let latency = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
    finish(pool, &row, token, result, latency).await?;
    tracing::debug!(%id, "monitor check complete");
    Ok(true)
}

async fn finish(
    pool: &PgPool,
    claimed: &sqlx::postgres::PgRow,
    token: Uuid,
    result: Result<u16, &'static str>,
    latency: i32,
) -> Result<(), sqlx::Error> {
    let id: Uuid = claimed.get("id");
    let version: i64 = claimed.get("version");
    let mut tx = pool.begin().await?;
    let current=sqlx::query("SELECT m.* FROM monitors m JOIN instance_settings s ON s.singleton WHERE m.id=$1 AND m.version=$2 AND m.lease_owner=$3 AND m.lease_until>now() AND m.generation=s.generation AND m.enabled AND m.deleted_at IS NULL FOR UPDATE OF m")
        .bind(id).bind(version).bind(token).fetch_optional(&mut *tx).await?;
    let Some(row) = current else { return Ok(()) };
    let ok = result.is_ok_and(|code| i32::from(code) == row.get::<i32, _>("expected_status"));
    let (status, failures) = health(
        ok,
        row.get("consecutive_failures"),
        row.get("failure_threshold"),
    );
    let previous: Option<Uuid> = row.get("incident_id");
    let incident = if status == "down" {
        Some(previous.unwrap_or_else(Uuid::now_v7))
    } else if ok {
        None
    } else {
        previous
    };
    sqlx::query("INSERT INTO monitor_samples(id,monitor_id,config_version,ok,status_code,latency_ms,error_code) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(Uuid::now_v7()).bind(id).bind(version).bind(ok).bind(result.ok().map(i32::from)).bind(latency).bind(result.err().or(if ok {None} else {Some("unexpected_status")})).execute(&mut *tx).await?;
    sqlx::query("UPDATE monitors SET status=$2,consecutive_failures=$3,incident_id=$4,lease_owner=NULL,lease_until=NULL,next_check_at=now()+interval_seconds*interval '1 second' WHERE id=$1")
        .bind(id).bind(status).bind(failures).bind(incident).execute(&mut *tx).await?;
    let transition = if previous.is_none() {
        incident.map(|i| (i, "monitor.down"))
    } else if ok {
        previous.map(|i| (i, "monitor.recovered"))
    } else {
        None
    };
    if let Some((incident, kind)) = transition {
        let event = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO monitor_events(id,monitor_id,incident_id,kind) VALUES($1,$2,$3,$4)",
        )
        .bind(event)
        .bind(id)
        .bind(incident)
        .bind(kind)
        .execute(&mut *tx)
        .await?;
        let project: Uuid = row.get("project_id");
        crate::notification_queue::enqueue(&mut tx,crate::notification_queue::Event{
            project,key:format!("monitor:{event}"),kind:kind.into(),payload:serde_json::json!({"event":kind,"title":row.get::<String,_>("name"),"body":status,"route":format!("/status?project={project}&id={id}")}),issue:None,actor:None,target:None,
        }).await?;
        // Both the event and each personal inbox item commit with the measured state.
        sqlx::query("INSERT INTO inbox_items(id,user_id,project_id,monitor_id,monitor_event_id,category,title,body) SELECT gen_random_uuid(),u.id,m.project_id,m.id,$2,'monitor',m.name,$3 FROM monitors m JOIN users u ON (u.role='superadmin' OR EXISTS(SELECT 1 FROM project_assignments a WHERE a.user_id=u.id AND a.project_id=m.project_id)) LEFT JOIN inbox_preferences p ON p.user_id=u.id WHERE m.id=$1 AND COALESCE(p.monitors,true) ON CONFLICT(user_id,monitor_event_id) DO NOTHING")
            .bind(id).bind(event).bind(status).execute(&mut *tx).await?;
    }
    tx.commit().await
}

pub fn spawn(pool: PgPool) {
    for _ in 0..4 {
        let pool = pool.clone();
        tokio::spawn(async move {
            loop {
                match cycle(&pool).await {
                    Ok(true) => {}
                    Ok(false) => tokio::time::sleep(Duration::from_secs(1)).await,
                    Err(error) => {
                        tracing::warn!(%error,"monitor worker failed");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }
    tokio::spawn(async move {
        loop {
            // Bounded cleanup, including deleted configurations. Incidents remain auditable.
            if let Err(error)=sqlx::query("DELETE FROM monitor_samples WHERE id IN (SELECT id FROM monitor_samples WHERE checked_at<now()-interval '30 days' ORDER BY checked_at LIMIT 10000)").execute(&pool).await {
                tracing::warn!(%error,"monitor retention failed");
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::health;
    #[test]
    fn threshold_recovery_and_saturation() {
        assert_eq!(health(false, 0, 2), ("degraded", 1));
        assert_eq!(health(false, 1, 2), ("down", 2));
        assert_eq!(health(false, 10, 2), ("down", 10));
        assert_eq!(health(true, 10, 2), ("operational", 0));
        assert_eq!(health(false, 0, 1), ("down", 1));
    }
}
