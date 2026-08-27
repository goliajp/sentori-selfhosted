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
use tracing::warn;

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

/// `GET /v1/releases/{release}/artifacts`
///
/// The read half of the upload route, on the same api-scope token.
///
/// insight's release pipeline lost its dSYM step for a year: the
/// upload never ran, and because the CLI exits 0 on failure by
/// design, nothing in their CI could tell. The fix they wanted was to
/// ask the server what actually landed — but the only endpoint that
/// answered needs a browser session and the project's UUID, and a
/// laptop running `/pub` has neither and should not be handed an
/// admin session to get one. Reading back what your own api token
/// just wrote is strictly weaker than writing it.
///
/// `kinds` carries a count for every kind, zeros included: a gate
/// wants to test a number, not the absence of a key. `missing` is
/// that same fact stated once so the common case is a one-line
/// check.
pub async fn list_by_release_name(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<IngestContext>,
    Path(release): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    super::sdk::require_admin_token(&ctx)?;

    let rows = sqlx::query(
        "SELECT a.kind, a.name, a.content_hash, a.size_bytes, a.created_at, a.usable \
         FROM release_artifacts a \
         JOIN releases r ON r.id = a.release_id \
         WHERE r.project_id = $1 AND r.name = $2 \
         ORDER BY a.kind COLLATE \"C\", a.name COLLATE \"C\"",
    )
    .bind(ctx.project_id)
    .bind(&release)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    // A release row can exist without artifacts (the deploy marker
    // arrives on first launch), and artifacts can exist without a
    // deploy marker (maps are built before the app ever runs). Both
    // are normal; a name nobody has ever heard of is not, and a
    // pipeline that typos the release string would otherwise read
    // "artifacts missing" and never suspect the name.
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM releases WHERE project_id = $1 AND name = $2)",
    )
    .bind(ctx.project_id)
    .bind(&release)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    let mut counts = serde_json::Map::new();
    for k in KINDS {
        counts.insert((*k).to_owned(), json!(0));
    }
    // Same rule as the upload: a row this handler cannot read is a
    // row it skips, not a panic. The listing is what the release page
    // and `sentori-cli artifacts check` both call, and neither can do
    // anything with a worker that died.
    let artifacts: Vec<Value> = rows
        .iter()
        .filter_map(|r| {
            let kind: String = r.try_get("kind").ok()?;
            let name: String = r.try_get("name").ok()?;
            let usable: Option<bool> = r.try_get("usable").unwrap_or(None);
            // An artifact the reader cannot parse does not count
            // towards its kind. Counting it green is how a release
            // shows three lit lights and symbolicates nothing.
            if usable != Some(false)
                && let Some(n) = counts.get_mut(&kind)
            {
                *n = json!(n.as_u64().unwrap_or(0) + 1);
            }
            Some(json!({
                "kind": kind,
                "name": name,
                // For dSYMs the slice's debug id lives in the file
                // name — it is what a crashing frame is matched
                // against, so it is the only field that answers "is
                // this the build that shipped?".
                "debugId": debug_id_from_name(&name),
                "contentHash": r.try_get::<String, _>("content_hash").unwrap_or_default(),
                "sizeBytes": r.try_get::<i64, _>("size_bytes").unwrap_or(0),
                // null on artifacts uploaded before the check
                // existed — "never looked at" is not the same claim
                // as "looked at and fine".
                "usable": usable,
                "createdAt": r
                    .try_get("created_at")
                    .ok()
                    .map(crate::wire_time::rfc3339),
            }))
        })
        .collect();

    let missing: Vec<&str> = KINDS
        .iter()
        .copied()
        .filter(|k| counts.get(*k).and_then(Value::as_u64).unwrap_or(0) == 0)
        .collect();

    Ok(Json(json!({
        "release": release,
        "known": known,
        "kinds": Value::Object(counts),
        "missing": missing,
        "artifacts": artifacts,
    })))
}

/// The 32-hex debug id embedded in an uploaded slice name, if there
/// is one. `Insight.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9`
/// and `e63a748c3f0e302d95ec8da5b55c97d9.dSYM` both carry
/// `E63A748C3F0E302D95EC8DA5B55C97D9`; `index.android.bundle.map`
/// carries nothing, and says so.
///
/// Read as tokens between the name's separators rather than as a run
/// of hex characters: `arm64` ends in two hex digits and sits one
/// dash away from the id, so a longest-run scan returns a 32-window
/// shifted two places — a plausible-looking id that matches no frame.
/// Anything this reports must be findable by the same normalisation
/// the symbolicator matches with, which the tests assert.
fn debug_id_from_name(name: &str) -> Option<String> {
    let tokens: Vec<&str> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let hex = |t: &str, n: usize| t.len() == n && t.bytes().all(|b| b.is_ascii_hexdigit());

    // The bare form: one token that is exactly a debug id.
    if let Some(t) = tokens.iter().find(|t| hex(t, 32)) {
        return Some(t.to_ascii_uppercase());
    }
    // The dashed form Apple and `sentori-cli` write: 8-4-4-4-12.
    for w in tokens.windows(5) {
        if hex(w[0], 8) && hex(w[1], 4) && hex(w[2], 4) && hex(w[3], 4) && hex(w[4], 12) {
            return Some(w.concat().to_ascii_uppercase());
        }
    }
    None
}

/// Blob + row. Shared so the two routes cannot drift into storing
/// artifacts the symbolicator reads differently depending on who
/// uploaded them.
async fn store(
    state: &Arc<AppState>,
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

    // Can the symbolicator actually read this? Decided here, once,
    // because the answer costs a parse of tens of megabytes and the
    // alternative is deriving it on every list request — or, as it
    // was, never.
    //
    // A wrong artifact is stored anyway. Refusing it would break the
    // rule that Sentori cannot fail a release, and a file the reader
    // did not expect is not necessarily worthless. But it stops being
    // silent: the response says so, the listing says so, and
    // `sentori-cli artifacts check` fails on it.
    let usable = usable_for_symbolication(&kind, bytes);

    // The table's unique key is (release_id, kind, name), so a
    // re-upload after a failed ship replaces rather than accumulating
    // near-duplicates a symbolicator would have to choose between.
    //
    // `prev` reads the row as it was before this statement — CTEs see
    // one snapshot — so the response can say whether anything about
    // this artifact is actually new. Re-archiving a dSYM does not
    // guarantee the same debug id as the build that shipped, and an
    // uploader with no way to tell cannot know whether the re-upload
    // was worth anything.
    let row = sqlx::query(
        "WITH prev AS ( \
           SELECT content_hash FROM release_artifacts \
           WHERE release_id = $1 AND kind = $2 AND name = $3 \
         ), up AS ( \
           INSERT INTO release_artifacts \
             (id, release_id, kind, name, content_hash, size_bytes, usable) \
           VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6) \
           ON CONFLICT (release_id, kind, name) DO UPDATE \
             SET content_hash = EXCLUDED.content_hash, \
                 size_bytes = EXCLUDED.size_bytes, \
                 usable = EXCLUDED.usable, \
                 created_at = now() \
           RETURNING id \
         ) \
         SELECT up.id, prev.content_hash AS prev_hash \
         FROM up LEFT JOIN prev ON true",
    )
    .bind(release_id)
    .bind(&kind)
    .bind(&name)
    .bind(hash.to_hex())
    .bind(i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    .bind(usable)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    // The crashes that arrived before this artifact did are stored
    // with unreadable stacks. Re-read them now — that recovery is the
    // whole reason the upload CLI is allowed to fail without failing
    // a build. Behind the response: the uploader asked whether the
    // artifact landed, and it did.
    if let Ok(r) = sqlx::query("SELECT project_id, name FROM releases WHERE id = $1")
        .bind(release_id)
        .fetch_one(&state.pool)
        .await
    {
        // Best-effort by design — this re-reads crashes that arrived
        // before the artifact. Failing to read the row means no
        // re-symbolication, which is what happened anyway before the
        // upload; it is not worth a panic.
        if let (Ok(pid), Ok(rel)) = (
            r.try_get::<Uuid, _>("project_id"),
            r.try_get::<String, _>("name"),
        ) {
            crate::resymbolicate::spawn_for_release(state, pid, rel);
        }
    }

    // `try_get`, not `get`. `Row::get` panics on a column the result
    // does not carry, and a panic here takes a tokio worker down over
    // an artifact upload — the one thing this endpoint is not allowed
    // to do. It happened: a backend whose Describe reported no columns
    // for a data-modifying CTE turned a map upload into
    // `ColumnNotFound("prev_hash")` mid-request, and the client saw an
    // empty reply rather than an error it could read.
    //
    // `prev_hash` is informational — it answers "did these bytes
    // change" — so losing it costs a field, not the upload.
    let prev_hash: Option<String> = row.try_get("prev_hash").unwrap_or_else(|e| {
        warn!(error = %e, "artifact upload: no prev_hash column in the result");
        None
    });
    // `id` is not informational; without it the response is a lie.
    // Still an error rather than a panic.
    let Ok(id) = row.try_get::<Uuid, _>("id") else {
        warn!("artifact upload: the insert returned no id column");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "artifact stored but the insert returned no id" })),
        ));
    };
    let hash_hex = hash.to_hex();
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "kind": kind,
            "name": name,
            "content_hash": hash_hex,
            "size_bytes": bytes.len(),
            // Was this slice already on the release, and did its
            // bytes change? A re-archived dSYM whose debug id the
            // server has never seen is a new slice; one that lands on
            // an existing name with identical content changed
            // nothing, and the uploader deserves to know which.
            "debug_id": debug_id_from_name(&name),
            "first_seen": prev_hash.is_none(),
            "content_changed": prev_hash.as_deref() != Some(hash_hex.as_str()),
            // null for kinds this server does not parse ahead of
            // time; false means stored but unreadable.
            "usable": usable,
            "hint": usable.map_or(Value::Null, |ok| if ok {
                Value::Null
            } else {
                Value::from(unusable_hint(&kind))
            }),
        })),
    ))
}

/// What to tell someone whose upload stored fine and will never
/// symbolicate anything.
/// Mach-O, or a fat/universal archive of them. Little- and
/// big-endian, 32- and 64-bit — a dSYM slice is one of these six and
/// nothing else is.
fn looks_like_macho(bytes: &[u8]) -> bool {
    // MH_MAGIC / MH_CIGAM / MH_MAGIC_64 / MH_CIGAM_64, then
    // FAT_MAGIC / FAT_CIGAM.
    const MAGICS: [[u8; 4]; 6] = [
        [0xfe, 0xed, 0xfa, 0xce],
        [0xce, 0xfa, 0xed, 0xfe],
        [0xfe, 0xed, 0xfa, 0xcf],
        [0xcf, 0xfa, 0xed, 0xfe],
        [0xca, 0xfe, 0xba, 0xbe],
        [0xbe, 0xba, 0xfe, 0xca],
    ];
    let Some(magic) = bytes.get(..4) else {
        return false;
    };
    MAGICS.iter().any(|m| m == magic)
}

fn unusable_hint(kind: &str) -> &'static str {
    match kind {
        "sourcemap" => {
            "stored, but this is not a source map this server can read — for a React Native \
             build upload the composed map (`sentori-cli react-native upload --metro-map <m> \
             --hermes-map <h>`), not the bundle"
        }
        "proguard" => {
            "stored, but this is not an R8/proguard mapping — upload \
             build/outputs/mapping/<variant>/mapping.txt"
        }
        "dsym" => {
            "stored, but this has no DWARF a symbolicator can read — upload the binary inside \
             the .dSYM bundle (Contents/Resources/DWARF/<name>), or let `sentori-cli upload \
             dsym <path.dSYM>` find the slices for you"
        }
        _ => "stored, but this server cannot read it",
    }
}

/// `Some(true|false)` for the three kinds a symbolicator reads,
/// `None` for the rest.
///
/// Each is answered with the same parser that will be asked for real
/// later — the point is to fail here, once, in front of whoever
/// uploaded it, rather than months later in a log nobody reads.
/// `srcbundle` is a plain JSON blob the reader tolerates loosely, so
/// there is nothing sharp to assert and it stays `None`; claiming
/// `true` for "we did not look" is worse than saying nothing.
pub fn usable_for_symbolication(kind: &str, bytes: &[u8]) -> Option<bool> {
    match kind {
        "sourcemap" => Some(sentori_sourcemap_resolver::ParsedMap::parse(bytes).is_ok()),
        "proguard" => Some(sentori_proguard_resolver::ParsedMapping::parse(bytes.to_vec()).is_ok()),
        // Header only. insight's main slice is 310 MB, and a full
        // DWARF parse here would hold the upload's buffer plus a copy
        // plus the parse — on a small self-hosted box that is how an
        // artifact upload becomes an OOM. The magic catches the
        // failure that actually happens (a zip, a plist, the .dSYM
        // directory wrapper, a text file) at no cost; a Mach-O whose
        // DWARF turns out to be unusable still surfaces later as an
        // unresolved frame, which is where it always did.
        "dsym" => Some(looks_like_macho(bytes)),
        _ => None,
    }
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

    #[test]
    fn only_a_mach_o_passes_as_a_dsym() {
        // The six magics a dSYM slice can start with.
        for m in [
            [0xfe, 0xed, 0xfa, 0xce],
            [0xce, 0xfa, 0xed, 0xfe],
            [0xfe, 0xed, 0xfa, 0xcf],
            [0xcf, 0xfa, 0xed, 0xfe],
            [0xca, 0xfe, 0xba, 0xbe],
            [0xbe, 0xba, 0xfe, 0xca],
        ] {
            assert!(looks_like_macho(&m), "{m:?} is a Mach-O magic");
        }
        // What people actually upload by mistake.
        assert!(!looks_like_macho(b"PK\x03\x04zip"), "a zip is not a dSYM");
        assert!(!looks_like_macho(b"<?xml version"), "a plist is not a dSYM");
        assert!(!looks_like_macho(b""), "nothing is not a dSYM");
        assert!(
            !looks_like_macho(b"\xfe\xed"),
            "a truncated magic is not one"
        );
    }

    #[test]
    fn a_dsym_slice_name_yields_the_id_the_symbolicator_matches_on() {
        // The exact shape `sentori-cli upload dsym` produces.
        assert_eq!(
            debug_id_from_name("Insight.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9"),
            Some("E63A748C3F0E302D95EC8DA5B55C97D9".to_owned()),
        );
        // Same id, already bare and lowercase.
        assert_eq!(
            debug_id_from_name("e63a748c3f0e302d95ec8da5b55c97d9.dSYM"),
            Some("E63A748C3F0E302D95EC8DA5B55C97D9".to_owned()),
        );
    }

    #[test]
    fn a_name_without_an_id_reports_none_rather_than_a_fragment() {
        assert_eq!(debug_id_from_name("index.android.bundle.map"), None);
        assert_eq!(debug_id_from_name("mapping.txt"), None);
        // 31 hex digits is not a debug id, and half of one is worse
        // than nothing: it would match no frame and read like a fact.
        assert_eq!(
            debug_id_from_name("abc-0123456789abcdef0123456789abcde"),
            None
        );
    }

    #[test]
    fn the_reported_id_is_what_native_symbolicate_would_look_for() {
        // The two normalisations must agree; a debug id we report but
        // the symbolicator cannot find is a lie with a hex face.
        let name = "Insight.app-arm64-E63A748C-3F0E-302D-95EC-8DA5B55C97D9";
        let id = debug_id_from_name(name).unwrap_or_default();
        assert!(!id.is_empty(), "expected a debug id in {name}");
        let normalised: String = name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_uppercase();
        assert!(normalised.contains(&id));
    }

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

    /// This endpoint may not panic, and the reason is a contract
    /// rather than taste: an artifact upload runs inside a customer's
    /// release pipeline, and Sentori is not allowed to fail it.
    ///
    /// `sqlx::Row::get` panics when the result does not carry the
    /// column. That is not hypothetical — a backend whose Describe
    /// reported no columns for this handler's data-modifying CTE
    /// turned a map upload into `ColumnNotFound("prev_hash")` on a
    /// tokio worker, and the uploader saw an empty reply instead of
    /// an error it could act on.
    ///
    /// Scoped to this one file on purpose. The rest of the server
    /// still reads columns the panicking way in ~300 places; widening
    /// this is a separate decision about what a missing column means
    /// at each of them, and a check that quietly covered all of them
    /// would be a check nobody could keep green.
    #[test]
    fn nothing_in_this_handler_reads_a_column_the_panicking_way() {
        let src = include_str!("artifacts_upload.rs");
        let offenders: Vec<&str> = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            // `.get("name")` / `.get::<T, _>("name")` — the panicking
            // pair. `try_get` contains `_get` and is excluded by the
            // dot immediately before `get`.
            .filter(|l| l.contains(".get(\"") || l.contains("_>(\"") && l.contains(".get::<"))
            .collect();
        assert!(
            offenders.is_empty(),
            "a panicking column read is back in the upload handler:\n{}",
            offenders.join("\n")
        );
    }
}
