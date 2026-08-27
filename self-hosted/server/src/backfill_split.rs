//! One-shot backfill: split pre-2.9.0 mixed issues by
//! environment × platform (`sentori-server backfill-split`).
//!
//! 2.9.0 put environment + platform into the fingerprint; issues
//! created before that aggregated across both and carry NULL in the
//! new columns. This tool re-homes their events onto issues keyed by
//! the REAL post-split fingerprint (the same `compute_fingerprint`
//! ingest uses — a synthetic key would leave the backfilled case and
//! all future events in two different issues forever), rebuilds the
//! breadth×depth substrate, re-points probes, and deletes the empty
//! mixed shells. Activity history on the mixed shells goes with them
//! (a mixed case's history is itself mixed).
//!
//! Idempotent: a second run finds no NULL-column issues and exits.

use serde_json::Value;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::pipeline::{IncomingEvent, Kind, compute_fingerprint, group_identity};

// One transaction per mixed issue, one narrative — same rationale
// as `ingest`.
#[allow(clippy::too_many_lines)]
pub async fn run(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await?;

    let mixed: Vec<(Uuid, Uuid, String, String, String, Value)> = sqlx::query_as(
        "SELECT id, project_id, kind, group_title, message_sample, surface \
         FROM issues WHERE environment IS NULL",
    )
    .fetch_all(&pool)
    .await?;
    println!("mixed issues to split: {}", mixed.len());

    let mut migrated_events = 0_u64;
    let mut new_issues = 0_u64;

    for (old_id, project_id, kind_s, group_title, message_sample, surface) in &mixed {
        let mut tx = pool.begin().await?;

        let combos: Vec<(String, String)> =
            sqlx::query_as("SELECT DISTINCT environment, platform FROM events WHERE issue_id = $1")
                .bind(old_id)
                .fetch_all(&mut *tx)
                .await?;

        let mut touched: Vec<Uuid> = Vec::new();
        for (env, plat) in &combos {
            // One representative event supplies the payload the
            // error-kind fingerprint reads; the other kinds derive
            // from the issue's own identity (name = group_title).
            let payload: Value = sqlx::query_scalar(
                "SELECT payload FROM events \
                 WHERE issue_id = $1 AND environment = $2 AND platform = $3 \
                 ORDER BY occurred_at DESC LIMIT 1",
            )
            .bind(old_id)
            .bind(env)
            .bind(plat)
            .fetch_one(&mut *tx)
            .await?;

            let kind = Kind::from_db_str(kind_s).ok_or_else(|| format!("unknown kind {kind_s}"))?;
            let pseudo = IncomingEvent {
                id: Uuid::now_v7(),
                project_id: *project_id,
                kind,
                platform: plat.clone(),
                occurred_at: OffsetDateTime::now_utc(),
                release: String::new(),
                environment: env.clone(),
                name: Some(group_title.clone()),
                surface: surface.clone(),
                user_key: None,
                payload,
            };
            let (_, _, fp_input) = group_identity(&pseudo)?;
            let fp = compute_fingerprint(env, plat, &fp_input);

            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM issues WHERE project_id = $1 AND fingerprint = $2",
            )
            .bind(project_id)
            .bind(&fp)
            .fetch_optional(&mut *tx)
            .await?;
            let target = if let Some(id) = existing {
                id
            } else {
                {
                    let id = Uuid::now_v7();
                    sqlx::query(
                        "INSERT INTO issues \
                         (id, project_id, fingerprint, kind, group_title, message_sample, \
                          surface, status, first_seen, last_seen, event_count, \
                          environment, platform) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, 'open', now(), now(), 0, $8, $9)",
                    )
                    .bind(id)
                    .bind(project_id)
                    .bind(&fp)
                    .bind(kind_s)
                    .bind(group_title)
                    .bind(message_sample)
                    .bind(surface)
                    .bind(env)
                    .bind(plat)
                    .execute(&mut *tx)
                    .await?;
                    new_issues += 1;
                    id
                }
            };

            let moved = sqlx::query(
                "UPDATE events SET issue_id = $1 \
                 WHERE issue_id = $2 AND environment = $3 AND platform = $4",
            )
            .bind(target)
            .bind(old_id)
            .bind(env)
            .bind(plat)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            migrated_events += moved;
            touched.push(target);
        }

        // Re-point probes at the busiest surviving split before the
        // shell (and its SET NULL) goes.
        if kind_s == "probe" && !touched.is_empty() {
            sqlx::query(
                "UPDATE probes SET issue_id = ( \
                     SELECT i.id FROM issues i WHERE i.id = ANY($1) \
                     ORDER BY (SELECT max(e.occurred_at) FROM events e \
                               WHERE e.issue_id = i.id) DESC NULLS LAST LIMIT 1) \
                 WHERE issue_id = $2",
            )
            .bind(&touched)
            .bind(old_id)
            .execute(&mut *tx)
            .await?;
        }

        // The mixed shell has no events left; hits + activity cascade.
        sqlx::query("DELETE FROM issues WHERE id = $1")
            .bind(old_id)
            .execute(&mut *tx)
            .await?;

        // Rebuild stats for every split target from its events.
        for id in &touched {
            sqlx::query("DELETE FROM issue_user_hits WHERE issue_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO issue_user_hits (issue_id, user_key, hit_count, last_hit) \
                 SELECT $1, user_key, count(*), max(occurred_at) \
                 FROM events WHERE issue_id = $1 AND user_key IS NOT NULL \
                 GROUP BY user_key",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE issues SET \
                   event_count = (SELECT count(*) FROM events WHERE issue_id = $1), \
                   first_seen = (SELECT min(occurred_at) FROM events WHERE issue_id = $1), \
                   last_seen  = (SELECT max(occurred_at) FROM events WHERE issue_id = $1), \
                   last_release = (SELECT release FROM events WHERE issue_id = $1 \
                                   ORDER BY occurred_at DESC LIMIT 1), \
                   last_environment = environment, \
                   users_count = (SELECT count(*) FROM issue_user_hits WHERE issue_id = $1), \
                   max_per_user = COALESCE((SELECT max(hit_count) \
                                            FROM issue_user_hits WHERE issue_id = $1), 0) \
                 WHERE id = $1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        println!(
            "split {old_id} ({kind_s} \"{group_title}\") into {} combo(s)",
            combos.len()
        );
    }

    println!(
        "done: {} mixed issues split, {migrated_events} events migrated, {new_issues} issues created",
        mixed.len()
    );
    let _ = pool_close(&pool).await;
    Ok(())
}

async fn pool_close(pool: &PgPool) -> Result<(), sqlx::Error> {
    pool.close().await;
    Ok(())
}
