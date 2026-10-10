//! Per-process token cache for vendor auth tokens.
//!
//! APNs ES256 JWTs and HCM OAuth bearer tokens are both valid for
//! ~1 hour. Re-signing or re-OAuthing on every push is wasteful;
//! cache them per (credential_id, kind) until ~5 minutes before
//! expiry so the next send fetches a fresh one before the
//! current one falls off.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct CachedToken {
    pub token: String,
    pub expires_at: Instant,
}

pub struct TokenCache {
    inner: Mutex<HashMap<(Uuid, String), CachedToken>>,
}

impl TokenCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Return a cached token if it has at least 5 minutes of
    /// remaining validity, otherwise None.
    pub fn get(&self, credential_id: Uuid, kind: &str) -> Option<String> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.get(&(credential_id, kind.to_string()))?;
        let now = Instant::now();
        if entry.expires_at.checked_duration_since(now)? > Duration::from_mins(5) {
            Some(entry.token.clone())
        } else {
            None
        }
    }

    pub fn remove(&self, credential_id: Uuid, kind: &str) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.remove(&(credential_id, kind.to_string()));
        }
    }

    pub fn put(&self, credential_id: Uuid, kind: &str, token: String, ttl: Duration) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.retain(|_, entry| entry.expires_at > Instant::now());
            if guard.len() >= 1024 {
                guard.clear();
            }
            guard.insert(
                (credential_id, kind.to_string()),
                CachedToken {
                    token,
                    expires_at: Instant::now() + ttl,
                },
            );
        }
    }
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotated_credentials_never_share_tokens() {
        let cache = TokenCache::new();
        let old = Uuid::now_v7();
        let new = Uuid::now_v7();
        cache.put(old, "apns", "old".into(), Duration::from_mins(55));
        assert_eq!(cache.get(old, "apns").as_deref(), Some("old"));
        assert!(cache.get(new, "apns").is_none());
        cache.remove(old, "apns");
        assert!(cache.get(old, "apns").is_none());
        cache.put(new, "apns", "expiring".into(), Duration::from_secs(1));
        assert!(cache.get(new, "apns").is_none());
    }
}
