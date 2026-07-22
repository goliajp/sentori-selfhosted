//! Filing an event under the people it belongs to.
//!
//! The SDK hashes a user's email (or phone, or OAuth subject) on the
//! device and sends the digest as `payload.user.linkHashes`. Storing it
//! as-is would leave a table matchable against any other Sentori
//! deployment, so the server salts it again per identity scope and
//! keeps only that. `sentori-identity-fingerprint` owns the formula.
//!
//! This ran in the v1 stack and was lost in the v0.2 cutover:
//! `workspace_identity_scopes` was never populated, so there was no
//! scope to resolve and ingest quietly stopped writing fingerprints on
//! 2026-07-19 while the SDK kept sending the hashes. Migration 0036
//! restores the bindings; this module restores the write.
//!
//! Best-effort by design. A crash report that arrives is worth keeping
//! even if we cannot file it under a person — failing the ingest would
//! trade a real signal for a lookup convenience.

use std::collections::HashMap;

use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

/// Scope id and salt for a workspace, cached for the process lifetime.
///
/// Salts do not change: rotating one would orphan every fingerprint
/// already written under it, so a scope's salt is fixed once minted.
/// That makes an unbounded cache safe — it is bounded by the number of
/// workspaces, and each entry is 32 bytes.
pub type ScopeCache = HashMap<Uuid, (Uuid, Vec<u8>)>;

/// The cache as `AppState` holds it: shared, and written rarely.
pub type SharedScopeCache = std::sync::Arc<tokio::sync::RwLock<ScopeCache>>;

/// The slice of an event payload identity needs, cloned before the
/// event moves into ingest.
///
/// Only the `user` branch: copying the whole payload to reach one key
/// would double the allocation on the hottest path in the server.
#[must_use]
pub fn payload_slice(payload: &Value) -> Value {
    payload
        .get("user")
        .map_or(Value::Null, |u| serde_json::json!({ "user": u }))
}

/// Pull the client-side hashes off an event payload.
///
/// Returns `(key_type, client_hash)` pairs, skipping anything that does
/// not look like a digest. A raw address here means an SDK bug or a
/// forged payload; either way the correct answer is to not store it.
fn link_hashes(payload: &Value) -> Vec<(String, String)> {
    let Some(map) = payload
        .get("user")
        .and_then(|u| u.get("linkHashes"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(k, v)| {
            let s = v.as_str()?;
            if sentori_identity_fingerprint::is_valid_client_hash(s) {
                Some((k.clone(), s.to_owned()))
            } else {
                // Loud, because the only causes are a broken SDK or an
                // attempt to slip plaintext past the hashing promise.
                warn!(key_type = %k, "identity.link_hash rejected: not a lowercase hex digest");
                None
            }
        })
        .collect()
}

/// Resolve a workspace's identity scope, caching the salt.
async fn scope_for(
    pool: &PgPool,
    cache: &tokio::sync::RwLock<ScopeCache>,
    workspace_id: Uuid,
) -> Option<(Uuid, Vec<u8>)> {
    if let Some(hit) = cache.read().await.get(&workspace_id) {
        return Some(hit.clone());
    }
    let row: (Uuid, Vec<u8>) = sqlx::query_as(
        "SELECT s.id, s.salt \
         FROM workspace_identity_scopes ws \
         JOIN identity_scopes s ON s.id = ws.scope_id \
         WHERE ws.workspace_id = $1 AND ws.is_default \
         LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    cache.write().await.insert(workspace_id, row.clone());
    Some(row)
}

/// Store one fingerprint per link hash on this event.
///
/// Errors are logged, never returned: see the module note on why a
/// failure here must not cost the event.
pub async fn record(
    state: &crate::state::AppState,
    workspace_id: Uuid,
    event_id: Uuid,
    payload: &Value,
) {
    let (pool, cache) = (&state.pool, &state.identity_scopes);
    let pairs = link_hashes(payload);
    if pairs.is_empty() {
        return;
    }
    let Some((scope_id, salt)) = scope_for(pool, cache, workspace_id).await else {
        warn!(%workspace_id, "identity.scope missing — fingerprints not recorded");
        return;
    };
    for (key_type, client_hash) in pairs {
        let fp = sentori_identity_fingerprint::compute(&salt, &key_type, &client_hash);
        if let Err(e) = sqlx::query(
            "INSERT INTO identity_fingerprints (event_id, scope_id, key_type, fingerprint) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(event_id)
        .bind(scope_id)
        .bind(&key_type)
        .bind(fp.as_slice())
        .execute(pool)
        .await
        {
            warn!(error = %e, %event_id, "identity.fingerprint insert failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_valid_digests() {
        let p = json!({"user": {"id": "u1", "linkHashes": {"email": "a".repeat(64)}}});
        assert_eq!(link_hashes(&p), vec![("email".to_owned(), "a".repeat(64))],);
    }

    /// The case the validator exists for: a plaintext address must not
    /// reach the database, even though the event around it is fine.
    #[test]
    fn drops_anything_that_is_not_a_digest() {
        let p = json!({"user": {"linkHashes": {
            "email": "alice@example.com",
            "phone": "b".repeat(64),
        }}});
        assert_eq!(link_hashes(&p), vec![("phone".to_owned(), "b".repeat(64))]);
    }

    #[test]
    fn absent_user_or_hashes_is_not_an_error() {
        assert!(link_hashes(&json!({})).is_empty());
        assert!(link_hashes(&json!({"user": {"id": "u1"}})).is_empty());
        assert!(link_hashes(&json!({"user": {"linkHashes": "nope"}})).is_empty());
    }
}
