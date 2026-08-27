//! Reading the artifacts that were stored before anyone read them.
//!
//! `release_artifacts.usable` is decided at upload from 2.15.0. Every
//! artifact older than that carries NULL — "never looked at", which
//! is deliberately not the same claim as "looked at and fine". On the
//! instance that prompted the column, all twenty-one of them were
//! NULL, including the two Hermes bytecode bundles filed as source
//! maps that started the whole thread.
//!
//! Leaving them NULL forever would mean the check only ever protects
//! uploads that come after it, and the ones that already went wrong
//! stay quietly lit. This reads each one once and records the answer.
//!
//! Read-only on the blob store; the only write is the flag.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::state::AppState;

/// `sentori-server verify-artifacts [--all]`
///
/// Without `--all`, only artifacts never checked. With it, every
/// artifact — for after a resolver learns a format it used to refuse,
/// which is exactly what happened to Hermes maps in 2.14.0.
pub async fn run(database_url: &str, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect(database_url).await?;
    let attachments = crate::blob_store::AttachmentStore::from_env().await?;
    let state = Arc::new(AppState::new(pool.clone(), attachments));

    // `COLLATE "C"` on the release name: byte order, so two
    // deployments print the same sequence. Without it this orders by
    // whatever collation the operator's image has, and those differ —
    // our own compose ships postgres:18-alpine, which is musl and
    // behaves as C while declaring en_US.utf8. This output gets diffed.
    let sql = if all {
        "SELECT a.id, a.kind, a.name, a.content_hash, r.name AS release \
         FROM release_artifacts a JOIN releases r ON r.id = a.release_id \
         ORDER BY r.name COLLATE \"C\", a.kind, a.name COLLATE \"C\""
    } else {
        "SELECT a.id, a.kind, a.name, a.content_hash, r.name AS release \
         FROM release_artifacts a JOIN releases r ON r.id = a.release_id \
         WHERE a.usable IS NULL \
         ORDER BY r.name COLLATE \"C\", a.kind, a.name COLLATE \"C\""
    };
    let rows = sqlx::query(sql).fetch_all(&pool).await?;
    if rows.is_empty() {
        println!("nothing to check");
        return Ok(());
    }

    let (mut ok, mut bad, mut skipped) = (0usize, 0usize, 0usize);
    for row in rows {
        let id: Uuid = row.get("id");
        let kind: String = row.get("kind");
        let name: String = row.get("name");
        let release: String = row.get("release");
        let hash: String = row.get("content_hash");

        let Ok(parsed) = hash.parse() else {
            println!("  ?  {release} {kind} {name} — content hash unreadable");
            skipped += 1;
            continue;
        };
        let Ok(bytes) = state.attachments.get(&parsed).await else {
            println!("  ?  {release} {kind} {name} — blob missing");
            skipped += 1;
            continue;
        };
        let Some(usable) =
            crate::handlers::artifacts_upload::usable_for_symbolication(&kind, &bytes)
        else {
            skipped += 1;
            continue;
        };
        sqlx::query("UPDATE release_artifacts SET usable = $1 WHERE id = $2")
            .bind(usable)
            .bind(id)
            .execute(&pool)
            .await?;
        if usable {
            ok += 1;
        } else {
            bad += 1;
            println!("  ✗  {release} {kind} {name} — stored, unreadable");
        }
    }
    println!("checked: {ok} readable, {bad} not, {skipped} skipped");
    Ok(())
}
