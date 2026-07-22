//! Turning a minified frame back into a line of source.
//!
//! A JS stack from a shipped bundle points at
//! `index.android.bundle:1:284913`, which tells nobody anything. The
//! build that produced that bundle also produced a source map; upload
//! it against the release and this rewrites the frame to
//! `src/screens/CheckoutScreen.tsx:142:27`, with the function name the
//! developer wrote.
//!
//! Both halves shipped separately and were never joined:
//! `sourcemap-resolver` has had tests and benchmarks since before the
//! v0.2 cutover, `release_artifacts` has had a table, and ingest had
//! `frame: None` behind a TODO.
//!
//! ## Where this runs
//!
//! At ingest, on the event's own payload, before it is stored. The
//! alternative — symbolicating on read — means every dashboard load
//! re-does the work and an artifact uploaded later cannot fix events
//! already captured. Storing the resolved frame makes a crash readable
//! forever, at the cost of one parse per release per process.
//!
//! Original coordinates are kept alongside the rewritten ones. A wrong
//! source map is a real failure mode, and without the raw line an
//! operator has no way to tell "the map is stale" from "the code is
//! confusing".
//!
//! ## Best effort, always
//!
//! No map, an unparseable map, a frame the map does not cover — all
//! leave the frame as it arrived. A crash report is worth more than the
//! prettiness of its frames, and failing an ingest because a build
//! forgot to upload a map would lose the very report that reveals the
//! problem.

use std::num::NonZeroUsize;
use std::sync::Arc;

use sentori_sourcemap_resolver::{ParsedMap, ResolverCache};
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::warn;
use uuid::Uuid;

/// Parsed maps, keyed by content hash.
///
/// Keyed by hash rather than by release so a re-upload of identical
/// bytes reuses the parse, and a changed map evicts itself by getting a
/// different key. Capacity is small because the working set is "the
/// releases currently crashing", not "every release ever shipped".
pub type MapCache = ResolverCache<String>;

/// 16 parsed maps. A large React Native map parses to a few tens of MB.
const CACHE_CAPACITY: usize = 16;

#[must_use]
pub fn new_cache() -> MapCache {
    ResolverCache::new(NonZeroUsize::new(CACHE_CAPACITY).unwrap_or(NonZeroUsize::MIN))
}

/// Rewrite every JS frame in this payload that a map can resolve.
///
/// Returns the number of frames rewritten, for the ingest log — zero on
/// a release with no map is normal and not worth a warning, but zero on
/// a release that *has* one means the map does not match the bundle,
/// which is worth knowing.
pub async fn symbolicate_payload(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    cache: &MapCache,
    project_id: Uuid,
    release: &str,
    payload: &mut Value,
) -> usize {
    if release.is_empty() {
        return 0;
    }
    let Some(map) = map_for_release(pool, attachments, cache, project_id, release).await else {
        return 0;
    };
    let mut rewritten = 0;
    if let Some(error) = payload.get_mut("error") {
        rewritten += walk_causes(&map, error);
    }
    rewritten
}

/// Everything that has to happen to a payload before it is stored.
///
/// Two steps that look unrelated but share a reason to be here rather
/// than at read time: symbolication because a later upload cannot fix
/// an event already written, and the identity slice because it has to
/// be taken before the event moves into ingest.
///
/// Returns `(frames_symbolicated, identity_slice)`.
pub async fn prepare(
    state: &crate::state::AppState,
    project_id: Uuid,
    release: &str,
    payload: &mut Value,
) -> (usize, Value) {
    let n = symbolicate_payload(
        &state.pool,
        &state.attachments,
        &state.source_maps,
        project_id,
        release,
        payload,
    )
    .await;
    (n, crate::identity_link::payload_slice(payload))
}

/// Symbolicate an error and every link in its `cause` chain.
///
/// Recursive rather than iterative because a chain is a tree walk and
/// the borrow checker follows it naturally that way; an explicit loop
/// over `&mut` links needs the two borrows to overlap. Depth is bounded
/// by what an SDK can construct, which is a handful.
fn walk_causes(map: &ParsedMap, error: &mut Value) -> usize {
    let mut n = 0;
    if let Some(frames) = error.get_mut("stack").and_then(Value::as_array_mut) {
        for frame in frames {
            if rewrite_frame(map, frame) {
                n += 1;
            }
        }
    }
    // A cause chain is where the useful frame usually lives — the throw
    // site is often several `wrap and rethrow` layers above the code
    // that actually broke.
    if let Some(cause) = error.get_mut("cause")
        && !cause.is_null()
    {
        n += walk_causes(map, cause);
    }
    n
}

/// Rewrite one frame in place. `true` if the map covered it.
fn rewrite_frame(map: &ParsedMap, frame: &mut Value) -> bool {
    let Some(obj) = frame.as_object_mut() else {
        return false;
    };
    // Already resolved, or never minified: a frame carrying source
    // context has been through this once.
    if obj.contains_key("preContext") {
        return false;
    }
    let (Some(line), Some(column)) = (
        obj.get("line").and_then(Value::as_u64),
        obj.get("column").and_then(Value::as_u64),
    ) else {
        return false;
    };
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        return false;
    };
    let Some(res) = map.resolve(line, column) else {
        return false;
    };

    // Keep where it pointed before. A stale map produces confident
    // nonsense, and this is the only way to notice.
    obj.insert("minifiedLine".into(), Value::from(line));
    obj.insert("minifiedColumn".into(), Value::from(column));
    if let Some(f) = obj.get("file").cloned() {
        obj.insert("minifiedFile".into(), f);
    }

    obj.insert("line".into(), Value::from(res.line));
    obj.insert("column".into(), Value::from(res.column));
    if let Some(file) = res.file {
        // `inApp` is what the crash view colours on, and a resolved
        // path is the first point at which we can tell the reader's own
        // code from a dependency.
        let in_app = !file.contains("node_modules/");
        obj.insert("file".into(), Value::from(file));
        obj.insert("inApp".into(), Value::from(in_app));
    }
    if let Some(function) = res.function {
        obj.insert("function".into(), Value::from(function));
    }
    obj.insert("symbolicated".into(), Value::from(true));
    true
}

/// Load and parse the source map for a release, via the cache.
async fn map_for_release(
    pool: &PgPool,
    attachments: &crate::blob_store::AttachmentStore,
    cache: &MapCache,
    project_id: Uuid,
    release: &str,
) -> Option<Arc<ParsedMap>> {
    let row = sqlx::query(
        "SELECT a.content_hash \
         FROM release_artifacts a \
         JOIN releases r ON r.id = a.release_id \
         WHERE r.project_id = $1 AND r.name = $2 AND a.kind = 'sourcemap' \
         ORDER BY a.created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .bind(release)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    let hash: String = row.get("content_hash");

    if let Some(hit) = cache.get(&hash) {
        return Some(hit);
    }
    let bytes = attachments
        .get(&hash.parse().ok()?)
        .await
        .inspect_err(|e| warn!(error = %e, %release, "symbolicate: blob read failed"))
        .ok()?;
    let parsed = ParsedMap::parse(&bytes)
        .inspect_err(|e| warn!(error = %e, %release, "symbolicate: map unparseable"))
        .ok()?;
    let parsed = Arc::new(parsed);
    cache.insert(hash, Arc::clone(&parsed));
    Some(parsed)
}

#[cfg(test)]
// A fixture that will not parse is a broken test, not a runtime path.
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A map that resolves nothing, so the tests below exercise the
    /// walking and the guards rather than the resolver (which has its
    /// own suite in `sourcemap-resolver`).
    fn empty_map() -> ParsedMap {
        ParsedMap::parse(br#"{"version":3,"sources":[],"names":[],"mappings":""}"#)
            .expect("a minimal source map parses")
    }

    #[test]
    fn walks_the_whole_cause_chain() {
        let mut err = json!({
            "type": "A", "stack": [{"line": 1, "column": 1}],
            "cause": {
                "type": "B", "stack": [{"line": 1, "column": 2}],
                "cause": { "type": "C", "stack": [{"line": 1, "column": 3}] }
            }
        });
        // Nothing resolves against an empty map, but every frame must
        // be visited — a chain walk that stops at the first link would
        // also report zero.
        assert_eq!(walk_causes(&empty_map(), &mut err), 0);
        assert_eq!(err["cause"]["cause"]["stack"][0]["line"], 1);
    }

    #[test]
    fn a_null_cause_ends_the_chain() {
        let mut err = json!({ "type": "A", "cause": null });
        assert_eq!(walk_causes(&empty_map(), &mut err), 0);
    }

    /// A frame that already carries source context has been through
    /// this once; re-running would overwrite the real coordinates with
    /// a second lookup of coordinates that are no longer minified.
    #[test]
    fn a_resolved_frame_is_left_alone() {
        let map = empty_map();
        let mut frame = json!({
            "line": 1, "column": 1, "preContext": ["const a = 1"]
        });
        assert!(!rewrite_frame(&map, &mut frame));
        assert!(frame.get("minifiedLine").is_none());
    }

    /// The end-to-end shape, against a map that really resolves.
    ///
    /// Generated by hand rather than by a bundler so the expected
    /// output is arithmetic rather than whatever Metro happened to
    /// emit.
    ///
    /// Lines are 1-based on both sides of `resolve`, matching what a
    /// stack frame carries; the map's own encoding is 0-based, so the
    /// segment written as source line 141 comes back as 142.
    #[test]
    fn a_resolvable_frame_is_rewritten_and_keeps_its_original() {
        let map = ParsedMap::parse(
            br#"{"version":3,"sources":["src/screens/CheckoutScreen.tsx"],"names":["onPay"],"mappings":"UA6I0BA"}"#,
        )
        .expect("hand-built map parses");
        let mut frame = json!({
            "file": "index.android.bundle", "line": 1, "column": 10
        });
        assert!(rewrite_frame(&map, &mut frame), "the map covers 1:10");

        assert_eq!(frame["file"], "src/screens/CheckoutScreen.tsx");
        assert_eq!(frame["line"], 142);
        assert_eq!(frame["column"], 26);
        assert_eq!(frame["function"], "onPay");
        assert_eq!(frame["symbolicated"], true);
        // Not in node_modules, so it is the reader's own code.
        assert_eq!(frame["inApp"], true);
        // Where it pointed before, kept so a stale map is detectable.
        assert_eq!(frame["minifiedFile"], "index.android.bundle");
        assert_eq!(frame["minifiedLine"], 1);
        assert_eq!(frame["minifiedColumn"], 10);
    }

    #[test]
    fn a_frame_without_coordinates_is_left_alone() {
        let map = empty_map();
        let mut frame = json!({ "file": "native" });
        assert!(!rewrite_frame(&map, &mut frame));
    }
}
