//! Uploading the files that turn a minified frame back into source.
//!
//! `release_artifacts` and the three resolver crates
//! (`sourcemap-resolver`, `dwarf-resolver`, `proguard-resolver`) have
//! both existed since before the v0.2 cutover, with tests and
//! benchmarks. Nothing connected them: there was no way to put a file
//! in, so ingest had nothing to symbolicate against and left
//! `frame: None` behind a TODO.
//!
//! A build produces these once, at ship time, so the endpoint is
//! deliberately dull: POST the file, get an id. The interesting work
//! happens at ingest, where a stack arrives and has to be matched
//! against the right release.
//!
//! Content is stored in the blob store and referenced by hash, so two
//! releases sharing an artifact store one copy — a React Native
//! sourcemap for an unchanged JS bundle is byte-identical across a
//! native-only rebuild.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Multipart, Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use sentori_ingest_token::IngestContext;

use crate::session_mw::SessionContext;
use crate::state::AppState;

/// What a symbolicator can consume. Anything else is a typo, and
/// storing it would mean an artifact that silently never matches.
const KINDS: [&str; 4] = ["sourcemap", "dsym", "proguard", "srcbundle"];

/// Decompressed cap, 512 MB. The main dSYM of an RN + Expo app runs
/// hundreds of MB raw (insight-mobile's is 291 MB) — this bounds what
/// a mistaken upload can put on the blob volume, not what a real
/// artifact needs. The transport-side cap is smaller (the route's
/// DefaultBodyLimit): anything bigger must arrive gzipped.
const MAX_BYTES: usize = 512 * 1024 * 1024;

/// `POST /admin/api/projects/:project_id/releases/:release_id/artifacts`
///
/// multipart: `kind` (text), `file` (the artifact).
pub async fn upload(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<SessionContext>,
    Path((project_id, release_id)): Path<(Uuid, Uuid)>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    super::admin::tokens::ensure_project_access(&state, &ctx, project_id).await?;

    let (kind, name, bytes) = read_upload(multipart)
        .await
        .map_err(|m| (StatusCode::BAD_REQUEST, Json(json!({ "error": m }))))?;

    store(&state, release_id, kind, name, &bytes).await
}

/// `POST /v1/releases/{release}/artifacts`
///
/// Same upload, reached with an ingest token instead of a session.
/// A build pipeline has a token and a release name; it does not have a
/// browser session or the project's UUID, which is what the admin route
/// above requires — so without this there was no way for CI to upload a
/// map at all, and the documented `sentori-cli upload sourcemap` posted
/// to a route that did not exist.
///
/// The release is created if it is not there yet. Maps are produced at
/// build time, usually before the app has ever run and announced its
/// deploy, so requiring the row to exist first would make the ordering
/// a trap.
pub async fn upload_by_release_name(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<IngestContext>,
    Path(release): Path<String>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    // A public token ships inside the customer's app. Anyone who has
    // the app could otherwise replace the source map for a release,
    // and every stack in it would then symbolicate to whatever they
    // chose — silently, since a wrong map looks exactly like a right
    // one until someone reads a frame.
    super::sdk::require_admin_token(&ctx)?;

    if release.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "release required" })),
        ));
    }

    let (kind, name, bytes) = read_upload(multipart)
        .await
        .map_err(|m| (StatusCode::BAD_REQUEST, Json(json!({ "error": m }))))?;

    // Same UPSERT `/v1/deploys` uses, so an upload before the deploy
    // marker and a deploy marker before an upload land on one row.
    let release_id: Uuid = sqlx::query_scalar(
        "INSERT INTO releases (id, project_id, name) \
         VALUES (gen_random_uuid(), $1, $2) \
         ON CONFLICT (project_id, name) DO UPDATE SET name = EXCLUDED.name \
         RETURNING id",
    )
    .bind(ctx.project_id)
    .bind(&release)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    store(&state, release_id, kind, name, &bytes).await
}

/// Blob + row. Shared so the two routes cannot drift into storing
/// artifacts the symbolicator reads differently depending on who
/// uploaded them.
async fn store(
    state: &AppState,
    release_id: Uuid,
    kind: String,
    name: String,
    bytes: &[u8],
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let hash = state.attachments.put(bytes).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    // The table's unique key is (release_id, kind, name), so a
    // re-upload after a failed ship replaces rather than accumulating
    // near-duplicates a symbolicator would have to choose between.
    let row = sqlx::query(
        "INSERT INTO release_artifacts \
           (id, release_id, kind, name, content_hash, size_bytes) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5) \
         ON CONFLICT (release_id, kind, name) DO UPDATE \
           SET content_hash = EXCLUDED.content_hash, \
               size_bytes = EXCLUDED.size_bytes, \
               created_at = now() \
         RETURNING id",
    )
    .bind(release_id)
    .bind(&kind)
    .bind(&name)
    .bind(hash.to_hex())
    .bind(i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": row.get::<Uuid, _>("id"),
            "kind": kind,
            "name": name,
            "content_hash": hash.to_hex(),
            "size_bytes": bytes.len(),
        })),
    ))
}

async fn read_upload(mut multipart: Multipart) -> Result<(String, String, Vec<u8>), String> {
    let mut kind: Option<String> = None;
    let mut name: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| format!("malformed multipart: {e}"))?
    {
        match field.name() {
            Some("kind") => {
                kind = Some(field.text().await.map_err(|e| e.to_string())?);
            }
            Some("file") => {
                // The uploaded filename is what a symbolicator matches
                // on for dSYMs and proguard maps, so it is data, not
                // decoration.
                name = field.file_name().map(str::to_owned);
                let data = field.bytes().await.map_err(|e| e.to_string())?;
                if data.len() > MAX_BYTES {
                    return Err(format!("file exceeds {MAX_BYTES} bytes"));
                }
                // Gzip-transparent: symbolicators read raw DWARF /
                // proguard / sourcemap bytes, so compressed uploads
                // are inflated HERE — storing the .gz would mean a
                // 201 on data symbolication can never use.
                bytes = Some(maybe_gunzip(&data)?);
            }
            _ => {}
        }
    }

    let kind = kind.ok_or("missing `kind` field")?;
    if !KINDS.contains(&kind.as_str()) {
        return Err(format!("unknown kind {kind:?}; expected one of {KINDS:?}"));
    }
    let bytes = bytes.ok_or("missing `file` field")?;
    if bytes.is_empty() {
        return Err("file is empty".into());
    }
    let name = name.unwrap_or_else(|| format!("{kind}.bin"));
    // A gzipped upload usually arrives named `foo.map.gz`; the
    // stored artifact is the inflated file, so the suffix comes off
    // (dSYM/proguard matching is by filename).
    let name = name.strip_suffix(".gz").map_or(name.clone(), str::to_owned);
    Ok((kind, name, bytes))
}

/// Inflate if the payload is gzip (magic `1f 8b`), else pass through.
/// The decompressed size is capped by [`MAX_BYTES`] — a zip bomb hits
/// the limit and errors instead of exhausting memory.
fn maybe_gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if data.len() < 2 || data[0] != 0x1f || data[1] != 0x8b {
        return Ok(data.to_vec());
    }
    let mut out = Vec::new();
    let decoder = flate2::read::GzDecoder::new(data);
    // +1 so an at-limit stream is distinguishable from an over-limit one.
    let mut limited = decoder.take(MAX_BYTES as u64 + 1);
    limited
        .read_to_end(&mut out)
        .map_err(|e| format!("gzip decode failed: {e}"))?;
    if out.len() > MAX_BYTES {
        return Err(format!("decompressed file exceeds {MAX_BYTES} bytes"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gz(data: &[u8]) -> std::io::Result<Vec<u8>> {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data)?;
        enc.finish()
    }

    #[test]
    fn plain_bytes_pass_through() -> Result<(), String> {
        let data = b"DWARF is not gzip".to_vec();
        assert_eq!(maybe_gunzip(&data)?, data);
        Ok(())
    }

    #[test]
    fn gzip_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let original = vec![7u8; 128 * 1024];
        let inflated = maybe_gunzip(&gz(&original)?)?;
        assert_eq!(inflated, original);
        Ok(())
    }

    #[test]
    fn truncated_gzip_is_an_error_not_a_panic() -> std::io::Result<()> {
        let mut z = gz(b"hello sourcemap")?;
        z.truncate(z.len() / 2);
        assert!(maybe_gunzip(&z).is_err());
        Ok(())
    }

    /// A typo in `kind` would store an artifact that never matches
    /// anything, and the upload would look like it worked.
    #[test]
    fn kinds_are_the_ones_a_symbolicator_understands() {
        assert!(KINDS.contains(&"sourcemap"));
        assert!(KINDS.contains(&"dsym"));
        assert!(KINDS.contains(&"proguard"));
        assert!(KINDS.contains(&"srcbundle"));
        assert!(!KINDS.contains(&"source-map"));
        assert!(!KINDS.contains(&"symbols"));
    }
}
