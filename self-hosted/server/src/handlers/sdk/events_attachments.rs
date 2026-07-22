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
use sentori_billing::CounterKind;
use sentori_ingest_token::IngestContext;
use serde_json::{Value, json};
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;

use crate::handlers::sdk::quota;
use crate::state::AppState;

const MAX_BODY_BYTES: usize = 50 * 1024 * 1024; // 50 MiB hard cap

/// The kinds migration 0022's CHECK constraint accepts. Anything else
/// would be rejected by the database, so reject it here with a usable
/// message instead.
const KINDS: [&str; 6] = [
    "logTail",
    "replay",
    "screenshot",
    "sessionTrail",
    "stateSnapshot",
    "viewTree",
];

/// `source` is CHECK-constrained too.
const SOURCES: [&str; 3] = ["android", "ios", "js"];

pub async fn handle(
    Extension(ctx): Extension<IngestContext>,
    State(state): State<Arc<AppState>>,
    Path((event_id, kind)): Path<(Uuid, String)>,
    multipart: Multipart,
) -> (StatusCode, Json<Value>) {
    let Parsed {
        bytes,
        media_type,
        source,
    } = match accept(&kind, multipart).await {
        Ok(p) => p,
        Err((status, body)) => {
            warn!(workspace_id = %ctx.workspace_id, %kind, ?body, "sdk.attachments rejected");
            return (status, Json(body));
        }
    };

    // K17 quota: only replay recordings meter (as one Replays unit).
    // The other five kinds are debug artefacts attached to an event
    // that was already metered, not a separately billed counter.
    let now = OffsetDateTime::now_utc();
    if kind == "replay"
        && let Err(body) = quota::meter(&state, ctx.project_id, CounterKind::Replays, 1, now).await
    {
        return (StatusCode::PAYMENT_REQUIRED, Json(body));
    }

    let hash = match state.attachments.put(&bytes).await {
        Ok(h) => h,
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.attachments blob_store_error");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "blob_store_failed" })),
            );
        }
    };

    // Body length is bounded above, so this cannot saturate.
    let size_bytes = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
    let reference = Uuid::now_v7();
    let result = sqlx::query(
        "INSERT INTO event_attachments \
         (ref, workspace_id, project_id, event_id, kind, media_type, \
          size_bytes, captured_at, source, blob_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(reference)
    .bind(ctx.workspace_id.into_uuid())
    .bind(ctx.project_id.into_uuid())
    .bind(event_id)
    .bind(&kind)
    .bind(&media_type)
    .bind(size_bytes)
    .bind(now)
    .bind(&source)
    .bind(hash.to_hex())
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                workspace_id = %ctx.workspace_id,
                project_id = %ctx.project_id,
                %event_id,
                %kind,
                %source,
                size_bytes,
                "sdk.attachments stored",
            );
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "refId": reference.to_string(),
                    "sizeBytes": size_bytes,
                    "mediaType": media_type,
                    "kind": kind,
                })),
            )
        }
        Err(e) => {
            warn!(workspace_id = %ctx.workspace_id, error = %e, "sdk.attachments db_error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "internal" })),
            )
        }
    }
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
    let parsed = read_multipart(multipart).await.map_err(|detail| {
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
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| format!("reading file part: {e}"))?;
                bytes = Some(data.to_vec());
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
