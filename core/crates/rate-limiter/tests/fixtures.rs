//! Integration tests through the public crate surface only —
//! catches re-export regressions and locks in the public API
//! shape.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    missing_docs
)]

use core::time::Duration;
use std::sync::Arc;
use std::time::Instant;

use sentori_rate_limiter::{
    Clock, Limiter, MemoryBackend, MockClock, Policy, PolicyError, RateBackend, SystemClock,
    Verdict,
};

#[test]
fn end_to_end_via_public_api() {
    let policy = Policy::per_minute(3).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);

    for _ in 0..3 {
        assert!(limiter.check("user-42").is_allowed());
    }
    assert!(limiter.check("user-42").is_limited());
}

#[test]
fn arc_clock_pattern_works_via_public_api() {
    let anchor = Instant::now();
    let clock = Arc::new(MockClock::new(anchor));
    let limiter = Limiter::new(
        MemoryBackend::new(),
        Arc::clone(&clock),
        Policy::per_second(2).expect("policy"),
    );

    assert!(limiter.check("k").is_allowed());
    assert!(limiter.check("k").is_allowed());
    assert!(limiter.check("k").is_limited());

    clock.advance(Duration::from_secs(2));
    assert!(limiter.check("k").is_allowed());
}

#[test]
fn limiter_is_send_sync_via_arc() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Limiter<MemoryBackend, SystemClock>>();
    assert_sync::<Limiter<MemoryBackend, SystemClock>>();
    assert_send::<Arc<Limiter<MemoryBackend, SystemClock>>>();
    assert_sync::<Arc<Limiter<MemoryBackend, SystemClock>>>();
}

#[test]
fn policy_construction_rejects_zero() {
    let err = Policy::new(0, Duration::from_mins(1)).expect_err("zero");
    assert!(matches!(err, PolicyError::ZeroMaxRequests));
    let err = Policy::new(10, Duration::ZERO).expect_err("zero");
    assert!(matches!(err, PolicyError::ZeroWindow));
    let err = Policy::new(10, Duration::from_secs(3601)).expect_err("long");
    assert!(matches!(err, PolicyError::WindowTooLong));
}

#[test]
fn backend_trait_object_works() {
    // A caller wiring this up dynamically (e.g. a Limiter built
    // from a config file that picks "memory" vs "valkey") needs
    // `Box<dyn RateBackend>` to work. Lock that here.
    let backend: Box<dyn RateBackend> = Box::new(MemoryBackend::new());
    let now = Instant::now();
    let policy = Policy::new(2, Duration::from_mins(1)).expect("policy");
    assert!(matches!(
        backend.check_and_consume("k", policy, now),
        Verdict::Allowed { .. }
    ));
}

#[test]
fn limiter_reset_via_public_api() {
    let policy = Policy::per_minute(1).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
    limiter.check("a");
    limiter.check("b");
    limiter.reset("a");
    assert!(limiter.check("a").is_allowed());
    assert!(limiter.check("b").is_limited());
}

#[test]
fn limiter_clear_via_public_api() {
    let policy = Policy::per_minute(1).expect("policy");
    let limiter = Limiter::new(MemoryBackend::new(), SystemClock, policy);
    limiter.check("a");
    limiter.check("b");
    limiter.clear();
    assert!(limiter.check("a").is_allowed());
    assert!(limiter.check("b").is_allowed());
}

#[test]
fn approx_observability_hooks_via_public_api() {
    let policy = Policy::per_minute(5).expect("policy");
    let backend = MemoryBackend::new();
    let limiter = Limiter::new(backend, SystemClock, policy);
    for _ in 0..3 {
        limiter.check("user-42");
    }
    assert!(limiter.backend().approx_memory_bytes() > 0);
    assert_eq!(limiter.backend().approx_key_count(), 1);
}

#[test]
fn system_clock_now_is_monotonic_via_public_api() {
    let c = SystemClock;
    let a = c.now();
    let b = c.now();
    assert!(b >= a);
}
