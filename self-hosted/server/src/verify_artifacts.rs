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
        "SELECT a.id, a.kind, a.name, a.content_hash, r.name AS release, r.project_id \
         FROM release_artifacts a JOIN releases r ON r.id = a.release_id \
         ORDER BY r.name COLLATE \"C\", a.kind, a.name COLLATE \"C\""
    } else {
        "SELECT a.id, a.kind, a.name, a.content_hash, r.name AS release, r.project_id \
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
    let mut renamed_count = 0usize;
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

        // A dSYM stored before 3.11.0 was named whatever the uploader
        // called the file, and lookup matches a frame's `imageUuid`
        // against that name — so one uploaded by hand as `MyApp`
        // resolves nothing, and says nothing, because its bytes are
        // perfectly readable. New uploads get the id appended; the
        // rows already on disk only get it here.
        //
        // Renaming makes the artifact reachable; re-reading the
        // crashes that were waiting on it happens below, because a
        // reachable artifact and an unchanged stored stack still
        // leave the operator looking at nothing.
        if kind == "dsym" && usable {
            let renamed = crate::handlers::artifacts_upload::name_carrying(
                name.clone(),
                &sentori_dwarf_resolver::MachoSlicer::debug_ids(&bytes),
            );
            if renamed != name {
                // `(release_id, kind, name)` is unique, so the new
                // name may already be taken — by the correctly-named
                // upload of the same slice, most likely. Leave both
                // rows alone and say so rather than deleting either:
                // the working one is already working.
                let done = sqlx::query(
                    "UPDATE release_artifacts SET name = $1 WHERE id = $2 \
                     AND NOT EXISTS ( \
                       SELECT 1 FROM release_artifacts b \
                       WHERE b.release_id = release_artifacts.release_id \
                         AND b.kind = release_artifacts.kind AND b.name = $1 \
                     )",
                )
                .bind(&renamed)
                .bind(id)
                .execute(&pool)
                .await?;
                if done.rows_affected() == 1 {
                    // The rename alone leaves the crashes that were
                    // waiting on this artifact exactly as unreadable
                    // as they were. Re-read them here, so that the
                    // command an operator runs to find the problem is
                    // also the one that finishes fixing it.
                    let project_id: Uuid = row.get("project_id");
                    let rescued = crate::resymbolicate::one_release(&state, project_id, &release)
                        .await
                        .map_or(0, |t| t.rewritten);
                    println!(
                        "  →  {release} dsym {name} — renamed to {renamed}; \
                         {rescued} stored crashes re-read"
                    );
                    renamed_count += 1;
                } else {
                    println!(
                        "  ?  {release} dsym {name} — carries a debug id already stored \
                         under {renamed}; left as it is"
                    );
                    skipped += 1;
                }
            }
        }
        if usable {
            ok += 1;
        } else {
            bad += 1;
            println!("  ✗  {release} {kind} {name} — stored, unreadable");
        }
    }
    println!("checked: {ok} readable, {bad} not, {skipped} skipped, {renamed_count} renamed");
    Ok(())
}
