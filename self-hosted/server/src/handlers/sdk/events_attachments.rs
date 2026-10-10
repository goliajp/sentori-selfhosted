//! POST `/v1/events/:event_id/attachments/:kind` — the evidence an
//! SDK captures alongside a crash.
//!
//! Six kinds, all defined by the `event_attachments` CHECK in
//! migration 0022: `screenshot`, `viewTree`, `stateSnapshot`,
//! `logTail`, `sessionTrail`, `replay`. The replay payload is the
//! wireframe NDJSON recording (keyframe + delta lines) the RN SDK
//! keeps in a rolling 60-second ring.
//!
//! ## Ordering contract
//!
//! The SDK uploads attachments **before** POSTing the event, then
//! echoes the returned `refId` into `event.attachments[]`. So this
//! handler must not assume the event row exists — and indeed
//! `event_attachments.event_id` deliberately carries no foreign key.
//!
//! ## Wire format
//!
//! `multipart/form-data` with a `file` part (the bytes, carrying the
//! media type) and a `source` field (`js` / `ios` / `android`). Both
//! the React Native and browser SDKs post this shape.
//!
//! Response is `{ refId, sizeBytes, mediaType, kind }`; the SDK drops
//! the attachment if `refId` is absent.
//!
//! This handler was rewritten 2026-07-21. Every upload had been
//! failing in production — the table was empty on a deployment with
//! 2211 events — for four independent reasons: it read the body as
//! raw bytes while both SDKs send multipart; its `kind` allowlist
//! admitted build artefacts (`sourcemap`, `dsym`, `proguard`) that
//! violate the CHECK while rejecting four kinds the SDK actually
//! sends; its INSERT named three columns that do not exist (`id`,
//! `content_type`, `blob_hash`) and omitted two that are NOT NULL
//! (`captured_at`, `source`); and its response used field names the
//! SDK does not read.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;

use crate::state::AppState;

const MAX_BODY_BYTES: usize = 50 * 1024 * 1024; // 50 MiB hard cap

/// The kinds migration 0022's CHECK constraint accepts. Anything else
/// would be rejected by the database, so reject it here with a usable
/// message instead.
const KINDS: [&str; 7] = [
    "logTail",
    "replay",
    "screens",
    "screenshot",
    "sessionTrail",
    "stateSnapshot",
    "viewTree",
];

/// `source` is CHECK-constrained too, and is the same vocabulary the
/// events table uses — it had been a third spelling of it (`js` where
/// an event says `javascript`, and no room for the two runtimes v4
/// adds). `js` stays acceptable on the wire because every SDK in the
/// field posts it; it is rewritten before it reaches the column.
const SOURCES: [&str; 6] = ["android", "ios", "javascript", "js", "weapp", "web"];

/// The wire spelling every shipped SDK uses for the browser / JS
/// runtime, and what it is stored as.
const LEGACY_JS: (&str, &str) = ("js", "javascript");

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path((event_id, kind)): Path<(Uuid, String)>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let parsed = accept(&kind, multipart)
        .await
        .map_err(|(status, body)| (status, Json(body)))?;
    store(&state, ctx.project_id, event_id, &kind, parsed).await
}

async fn store(
    state: &Arc<AppState>,
    project: Uuid,
    event: Uuid,
    kind: &str,
    parsed: Parsed,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(crate::handlers::jobs::database_error)?;
    if state.cloud.is_some() {
        crate::cloud_quota::attachment(&mut tx, project, event, kind, parsed.bytes.len())
            .await
            .map_err(|error| error.reply())?;
    }
    // Publish the blob and its reference under the shared GC lock. Re-uploading
    // an old content hash cannot race physical orphan deletion.
    sqlx::query("SELECT pg_advisory_xact_lock_shared($1)")
        .bind(sentori_snapshot::BLOB_GC_LOCK)
        .execute(&mut *tx)
        .await
        .map_err(crate::handlers::jobs::database_error)?;
    let hash = state
        .attachments
        .put(&parsed.bytes)
        .await
        .map_err(|error| {
            warn!(%error, "sdk.attachments blob_store_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"blob_store_failed"})),
            )
        })?;
    let size_bytes = i32::try_from(parsed.bytes.len()).unwrap_or(i32::MAX);
    let reference = Uuid::now_v7();
    sqlx::query("INSERT INTO event_attachments(ref,project_id,event_id,kind,media_type,size_bytes,captured_at,source,blob_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(reference).bind(project).bind(event).bind(kind).bind(&parsed.media_type).bind(size_bytes).bind(OffsetDateTime::now_utc()).bind(&parsed.source).bind(hash.to_hex()).execute(&mut *tx).await.map_err(crate::handlers::jobs::database_error)?;
    tx.commit()
        .await
        .map_err(crate::handlers::jobs::database_error)?;
    info!(project_id=%project, event_id=%event, %kind, size_bytes, "sdk.attachments stored");
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"refId":reference,"sizeBytes":size_bytes,"mediaType":parsed.media_type,"kind":kind}),
        ),
    ))
}

struct Parsed {
    bytes: Vec<u8>,
    media_type: String,
    source: String,
}

/// Validate the path `kind`, parse the multipart body, and check the
/// resulting bytes and `source` against the column constraints. Every
/// rejection here is a 4xx the SDK can act on.
async fn accept(kind: &str, multipart: Multipart) -> Result<Parsed, (StatusCode, Value)> {
    if !KINDS.contains(&kind) {
        return Err((
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid_kind", "got": kind, "expected": KINDS }),
        ));
    }
    let mut parsed = read_multipart(multipart).await.map_err(|detail| {
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid_multipart", "detail": detail }),
        )
    })?;
    if parsed.bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, json!({ "error": "empty_file" })));
    }
    if parsed.bytes.len() > MAX_BODY_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({ "error": "too_large", "max": MAX_BODY_BYTES, "got": parsed.bytes.len() }),
        ));
    }
    if !SOURCES.contains(&parsed.source.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid_source", "got": parsed.source, "expected": SOURCES }),
        ));
    }
    if parsed.source == LEGACY_JS.0 {
        parsed.source = LEGACY_JS.1.to_string();
    }
    Ok(parsed)
}

/// Pull the `file` part and the `source` field out of the multipart
/// body. Unknown parts are skipped so the SDKs can add fields without
/// a server release.
async fn read_multipart(mut multipart: Multipart) -> Result<Parsed, String> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut media_type = String::from("application/octet-stream");
    let mut source = String::from("js");

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Err(format!("reading field: {e}")),
        };
        match field.name() {
            Some("file") => {
                if let Some(ct) = field.content_type() {
                    media_type = ct.to_string();
                }
                // RN cannot put raw binary in a hand-built multipart
                // string body, so binary parts arrive base64 with
                // this header; decode so the stored blob is the
                // real content, same as any other client's upload.
                let is_base64 = field
                    .headers()
                    .get("content-transfer-encoding")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.eq_ignore_ascii_case("base64"));
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| format!("reading file part: {e}"))?;
                bytes = Some(if is_base64 {
                    use base64::Engine;
                    base64::engine::general_purpose::STANDARD
                        .decode(data.as_ref())
                        .map_err(|e| format!("base64 file part: {e}"))?
                } else {
                    data.to_vec()
                });
            }
            Some("source") => {
                source = field
                    .text()
                    .await
                    .map_err(|e| format!("reading source part: {e}"))?
                    .trim()
                    .to_string();
            }
            _ => {
                // Drain so the stream stays in sync.
                let _ = field.bytes().await;
            }
        }
    }

    bytes
        .map(|bytes| Parsed {
            bytes,
            media_type,
            source,
        })
        .ok_or_else(|| "missing `file` part".to_string())
}
