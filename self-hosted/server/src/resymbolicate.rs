//! Re-reading stored crashes once the artifact that explains them
//! arrives.
//!
//! The upload CLI exits 0 when it fails, on purpose: Sentori must
//! never be the reason a release does not ship. That contract is only
//! honest if a late upload recovers what came in while the gap was
//! open — otherwise "the upload failed, carry on" quietly means "every
//! crash from this release is permanently unreadable".
//!
//! The docs, the CLI's own comments and a reply to a customer all said
//! the server did this. It did not. `rewrite_frame` was written to be
//! safe to run twice — it keeps `minifiedLine` / `minifiedColumn`
//! precisely so a second pass can re-resolve — and then nothing ever
//! called it a second time. The affordance existed; the pass did not.
//!
//! What this does and does not do:
//!
//! - It rewrites the **stored payload**: frames gain source paths,
//!   function names, `inApp` and the reading window. A stack that read
//!   as `index.android.bundle:1:289430` becomes the failing line.
//! - It does **not** re-group. The issue an event belongs to was
//!   decided at ingest from the stack as it looked then, and moving
//!   events between issues afterwards is a different operation with
//!   different risks (`backfill_split.rs` is what that looks like).
//!   So a release whose map arrived late keeps its crashes split
//!   across a pre-map and a post-map issue; both become readable.
//!   Saying so is better than a silent half-fix.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::state::AppState;

/// Ceiling on one pass. A release that has been live for a month can
/// hold a lot of events, and this runs behind an upload the operator
/// is waiting on — the newest are the ones someone is about to read.
const MAX_EVENTS: i64 = 20_000;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub scanned: usize,
    pub rewritten: usize,
    pub frames: usize,
}

/// Re-symbolicate the stored error events of one release.
///
/// Idempotent: a frame that already carries source context is skipped
/// by `rewrite_frame`, so re-running costs a read and writes nothing.
pub async fn one_release(
    state: &AppState,
    project_id: Uuid,
    release_name: &str,
) -> Result<Stats, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, platform, payload FROM events \
         WHERE project_id = $1 AND release = $2 AND kind = 'error' \
         ORDER BY occurred_at DESC LIMIT $3",
    )
    .bind(project_id)
    .bind(release_name)
    .bind(MAX_EVENTS)
    .fetch_all(&state.pool)
    .await?;

    let mut tally = Stats {
        scanned: rows.len(),
        ..Stats::default()
    };

    for row in rows {
        let id: Uuid = row.get("id");
        let platform: String = row.get("platform");
        let mut payload: serde_json::Value = row.get("payload");

        let mut n = crate::symbolicate::symbolicate_payload(
            &state.pool,
            &state.attachments,
            &state.source_maps,
            project_id,
            release_name,
            &mut payload,
        )
        .await;
        if platform == "android" || platform == "ios" {
            n += crate::native_symbolicate::symbolicate_native(
                &state.pool,
                &state.attachments,
                project_id,
                release_name,
                &platform,
                &mut payload,
            )
            .await;
        }
        if n == 0 {
            continue;
        }
        sqlx::query("UPDATE events SET payload = $1 WHERE id = $2")
            .bind(&payload)
            .bind(id)
            .execute(&state.pool)
            .await?;
        tally.rewritten += 1;
        tally.frames += n;
    }
    Ok(tally)
}

/// Fire the pass behind an artifact upload, without making the
/// uploader wait for it.
///
/// A build pipeline is holding this connection open with a 300 MB
/// dSYM behind it; the pass can take a while on a busy release and
/// nothing about the upload's success depends on its result. Failures
/// are logged, not surfaced: the artifact did land, which is what the
/// caller asked about.
pub fn spawn_for_release(state: &Arc<AppState>, project_id: Uuid, release_name: String) {
    let state = Arc::clone(state);
    tokio::spawn(async move {
        match one_release(&state, project_id, &release_name).await {
            Ok(s) if s.rewritten > 0 => tracing::info!(
                release = %release_name,
                scanned = s.scanned,
                rewritten = s.rewritten,
                frames = s.frames,
                "retro-symbolication rewrote stored events",
            ),
            Ok(s) => tracing::debug!(
                release = %release_name,
                scanned = s.scanned,
                "retro-symbolication found nothing to upgrade",
            ),
            Err(e) => tracing::warn!(
                release = %release_name,
                error = %e,
                "retro-symbolication failed; the artifact is stored and \
                 a later upload or `sentori-server resymbolicate` will retry",
            ),
        }
    });
}

/// `sentori-server resymbolicate [<release>]` — the operator path.
///
/// Auto-triggering covers the ordinary case (map arrives, stored
/// crashes become readable). This covers the ones it cannot: an
/// upload that predates this code, a pass that failed, or a map
/// replaced after the fact.
pub async fn run(database_url: &str, only: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect(database_url).await?;
    let attachments = crate::blob_store::AttachmentStore::from_env().await?;
    let state = Arc::new(AppState::new(pool.clone(), attachments));

    let releases: Vec<(Uuid, String)> = if let Some(name) = only {
        sqlx::query("SELECT project_id, name FROM releases WHERE name = $1")
            .bind(name)
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|r| (r.get("project_id"), r.get("name")))
            .collect()
    } else {
        sqlx::query("SELECT project_id, name FROM releases ORDER BY created_at DESC")
            .fetch_all(&pool)
            .await?
            .into_iter()
            .map(|r| (r.get("project_id"), r.get("name")))
            .collect()
    };

    if releases.is_empty() {
        println!("no matching release");
        return Ok(());
    }

    let mut total = Stats::default();
    for (project_id, name) in releases {
        let s = one_release(&state, project_id, &name).await?;
        println!(
            "{name}: scanned {}, rewrote {}, {} frame(s)",
            s.scanned, s.rewritten, s.frames
        );
        total.scanned += s.scanned;
        total.rewritten += s.rewritten;
        total.frames += s.frames;
    }
    println!(
        "done: scanned {}, rewrote {}, {} frame(s)",
        total.scanned, total.rewritten, total.frames
    );
    Ok(())
}
