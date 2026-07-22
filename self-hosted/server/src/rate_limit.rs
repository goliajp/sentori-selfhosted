//! Per-token in-memory sliding-window rate limiter for /v1/events.
//!
//! Sliding window: token_id → ring-buffer of N timestamps. Reject
//! when buffer is full AND oldest timestamp is < window seconds old.
//!
//! v0.2 ships per-process (single-instance self-hosted). Horizontal
//! scale would need Redis/Valkey backing — v0.3+ if SaaS demand
//! pushes us there.
//!
//! Tunables (env-vars):
//! - `SENTORI_RATELIMIT_DISABLED` default off (set to "1" or "true"
//!   to skip the ingest middleware entirely)
//! - `SENTORI_RATELIMIT_PER_TOKEN_RPS`  default 100 (events/sec/token)
//! - `SENTORI_RATELIMIT_WINDOW_SEC`     default 1
//!
//! A second instance guards the auth surface (login, register, forgot
//! password). Its numbers are much smaller — the goal there is not
//! throughput but slowing a password brute-force to something a human
//! notices — and its own tunables let an operator loosen one without
//! affecting ingest:
//! - `SENTORI_AUTH_RATELIMIT_DISABLED`  default off
//! - `SENTORI_AUTH_RATELIMIT_PER_IP`    default 10 (per window per IP)
//! - `SENTORI_AUTH_RATELIMIT_WINDOW_SEC` default 300 (5 minutes)

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Requests per window, per token. Far above any legitimate SDK: the
/// clients batch precisely to avoid rates like this, so a token
/// sustaining it is either misconfigured or not the customer's.
const DEFAULT_CAPACITY: usize = 100;
const DEFAULT_WINDOW_SEC: u64 = 1;

const DEFAULT_AUTH_CAPACITY: usize = 10;
const DEFAULT_AUTH_WINDOW_SEC: u64 = 300;

/// Fold a client IP into the `Uuid` the limiter uses as a key. `Uuid`
/// wants a fixed byte width; the IP is a variable-length string, so
/// truncating a hash is what shapes it. SHA-256's first 16 bytes are
/// as good as any for hashing — this is a bucket key, not a security
/// primitive, and the point is spread rather than secrecy.
#[must_use]
pub fn ip_to_key(ip: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(ip.as_bytes());
    let out = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&out[..16]);
    Uuid::from_bytes(bytes)
}

pub struct RateLimiter {
    buckets: Mutex<HashMap<Uuid, VecDeque<Instant>>>,
    capacity: usize,
    window: Duration,
    disabled: bool,
}

impl RateLimiter {
    /// The auth-endpoint limiter. Read once at boot; env vars only
    /// take effect on restart, which is the same shape as `from_env`.
    #[must_use]
    pub fn auth_from_env() -> Self {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let disabled = matches!(
            env("SENTORI_AUTH_RATELIMIT_DISABLED")
                .as_deref()
                .map(str::to_ascii_lowercase),
            Some(s) if s == "1" || s == "true"
        );
        let capacity = env("SENTORI_AUTH_RATELIMIT_PER_IP")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_AUTH_CAPACITY);
        let window = env("SENTORI_AUTH_RATELIMIT_WINDOW_SEC")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_AUTH_WINDOW_SEC);
        Self {
            buckets: Mutex::new(HashMap::new()),
            capacity,
            window: Duration::from_secs(window),
            disabled,
        }
    }

    #[must_use]
    pub fn from_env() -> Self {
        let disabled = matches!(
            std::env::var("SENTORI_RATELIMIT_DISABLED")
                .ok()
                .as_deref()
                .map(str::to_ascii_lowercase),
            Some(s) if s == "1" || s == "true"
        );
        let capacity = std::env::var("SENTORI_RATELIMIT_PER_TOKEN_RPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_CAPACITY);
        let window = std::env::var("SENTORI_RATELIMIT_WINDOW_SEC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_WINDOW_SEC);
        Self {
            buckets: Mutex::new(HashMap::new()),
            capacity,
            window: Duration::from_secs(window),
            disabled,
        }
    }

    /// Try to acquire a slot for the given token. Returns true on
    /// admit, false on reject (caller should return 429).
    pub fn admit(&self, token_id: Uuid) -> bool {
        if self.disabled {
            return true;
        }
        let now = Instant::now();
        let Ok(mut buckets) = self.buckets.lock() else {
            return true;
        };
        let buf = buckets.entry(token_id).or_insert_with(VecDeque::new);
        // Evict timestamps outside the window.
        while let Some(&front) = buf.front() {
            if now.duration_since(front) > self.window {
                buf.pop_front();
            } else {
                break;
            }
        }
        if buf.len() >= self.capacity {
            return false;
        }
        buf.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(capacity: usize, window_ms: u64) -> RateLimiter {
        RateLimiter {
            buckets: Mutex::new(HashMap::new()),
            capacity,
            window: Duration::from_millis(window_ms),
            disabled: false,
        }
    }

    fn limiter_secs(capacity: usize, window_secs: u64) -> RateLimiter {
        RateLimiter {
            buckets: Mutex::new(HashMap::new()),
            capacity,
            window: Duration::from_secs(window_secs),
            disabled: false,
        }
    }

    #[test]
    fn admits_up_to_capacity_then_refuses() {
        let rl = limiter_secs(3, 10);
        let t = Uuid::now_v7();
        assert!(rl.admit(t));
        assert!(rl.admit(t));
        assert!(rl.admit(t));
        assert!(!rl.admit(t), "the fourth in the window must be refused");
    }

    /// The bucket is keyed by token. One noisy app must not throttle a
    /// different customer's — which is the failure that would be
    /// hardest to notice, since the victim sees dropped telemetry and
    /// has nothing in their own traffic to explain it.
    #[test]
    fn buckets_are_per_token() {
        let rl = limiter_secs(1, 10);
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        assert!(rl.admit(a));
        assert!(!rl.admit(a));
        assert!(rl.admit(b), "a second token has its own allowance");
    }

    #[test]
    fn the_window_slides() {
        let rl = limiter(2, 50);
        let t = Uuid::now_v7();
        assert!(rl.admit(t));
        assert!(rl.admit(t));
        assert!(!rl.admit(t));
        std::thread::sleep(Duration::from_millis(80));
        assert!(rl.admit(t), "slots must come back once the window passes");
    }

    /// The kill switch has to be absolute. It exists for the moment
    /// someone discovers the limiter is dropping real traffic, and at
    /// that moment nobody wants to reason about edge cases.
    #[test]
    fn disabled_admits_everything() {
        let rl = RateLimiter {
            buckets: Mutex::new(HashMap::new()),
            capacity: 1,
            // Long enough that nothing in this test can age out of it;
            // the point is the switch, not the window.
            window: Duration::from_hours(1),
            disabled: true,
        };
        let t = Uuid::now_v7();
        for _ in 0..50 {
            assert!(rl.admit(t));
        }
    }

    /// The constants the module documents as defaults. Read from the
    /// struct rather than through `from_env`, which would need to
    /// mutate process-global environment and race every other test in
    /// the binary.
    #[test]
    fn documented_defaults() {
        assert_eq!(DEFAULT_CAPACITY, 100);
        assert_eq!(DEFAULT_WINDOW_SEC, 1);
    }
}
