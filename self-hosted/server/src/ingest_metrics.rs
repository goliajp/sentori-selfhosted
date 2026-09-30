//! Process-local ingest counters.
//!
//! `/metrics` could report how many events are in the database and
//! how many connections the pool held, but not how many events were
//! *refused* — the one number an operator needs to answer "is the
//! edge healthy?". A deploy checklist told you to watch
//! `sentori_ingest_total{status="rejected"}` during a roll, and
//! `ops/prometheus-alerts.yml` alerted on its rate, for a series the
//! server has never emitted.
//!
//! Counters, not gauges: a restart resets them to zero, which is
//! exactly what a Prometheus counter means, and `rate()` handles the
//! reset. `sentori_events_24h` still answers "how much traffic" from
//! the database; this answers "how much of it was turned away", which
//! the database cannot, because rejected events are never stored.
//!
//! **What it does not count.** A body that fails to deserialise never
//! reaches a handler — axum's `Json` extractor rejects it with `422`
//! and plain text, which `docs/errors.md` documents as deliberately
//! outside the error-code contract. So a `422` is invisible here, and
//! the error rate computed from these counters has it in neither the
//! numerator nor the denominator. That is the right call for a
//! per-event counter (a batch counts once per event, and a body that
//! did not parse has no events to count), but it means a client
//! sending structurally wrong JSON shows up as silence rather than as
//! errors. If that becomes a real failure mode, it needs its own
//! counter at the extractor, not a widening of this one.
//!
//! `Relaxed` ordering throughout: each counter is independent and
//! publishes no other memory, so there is nothing for a stronger
//! ordering to protect.

use std::sync::atomic::{AtomicU64, Ordering};

/// Terminal outcomes of an ingest request, one counter each.
#[derive(Debug, Default)]
pub struct IngestCounters {
    accepted: AtomicU64,
    rejected: AtomicU64,
    failed: AtomicU64,
    rate_limited: AtomicU64,
    client_dropped: AtomicU64,
    unknown_platform: AtomicU64,
}

impl IngestCounters {
    /// The event was stored and grouped (202).
    pub fn accepted(&self) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
    }

    /// The payload did not validate (400). The client's fault, and
    /// retrying it unchanged will fail the same way.
    pub fn rejected(&self) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
    }

    /// The pipeline errored (500). Ours to fix; the client should
    /// retry.
    pub fn failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Turned away by the per-token limiter (429).
    pub fn rate_limited(&self) {
        self.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    /// Events the SDK threw away before we ever saw them — a full
    /// queue, a spill that could not be written. The client counts
    /// them and carries the number on the next envelope; without this
    /// the count arrives and dies there, which is the same as not
    /// counting at all.
    pub fn client_dropped(&self, n: u64) {
        if n > 0 {
            self.client_dropped.fetch_add(n, Ordering::Relaxed);
        }
    }

    #[must_use]
    pub fn client_dropped_total(&self) -> u64 {
        self.client_dropped.load(Ordering::Relaxed)
    }

    /// An event named a platform this build does not know. It was
    /// accepted and stored as `unknown`, so it appears in `accepted`
    /// as well — this counter is the only way to tell that a fleet is
    /// running an SDK newer than this server.
    pub fn unknown_platform(&self) {
        self.unknown_platform.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn unknown_platform_total(&self) -> u64 {
        self.unknown_platform.load(Ordering::Relaxed)
    }

    /// `(status label, count)` for every outcome, always all four —
    /// a status that has not happened yet reads 0, which for a
    /// counter is a measurement, not an absence.
    #[must_use]
    pub fn snapshot(&self) -> [(&'static str, u64); 4] {
        [
            ("accepted", self.accepted.load(Ordering::Relaxed)),
            ("rejected", self.rejected.load(Ordering::Relaxed)),
            ("failed", self.failed.load(Ordering::Relaxed)),
            ("rate_limited", self.rate_limited.load(Ordering::Relaxed)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::IngestCounters;

    #[test]
    fn every_status_is_reported_even_at_zero() {
        let c = IngestCounters::default();
        let snap = c.snapshot();
        assert_eq!(snap.len(), 4);
        assert!(snap.iter().all(|(_, v)| *v == 0));
        // An absent series and a zero series mean different things to
        // a scraper; a counter that has not counted yet is zero.
        let labels: Vec<_> = snap.iter().map(|(k, _)| *k).collect();
        assert_eq!(labels, ["accepted", "rejected", "failed", "rate_limited"]);
    }

    #[test]
    fn an_unknown_platform_counts_separately_from_the_outcome() {
        let c = IngestCounters::default();
        c.unknown_platform();
        c.accepted();
        // The event was kept, so it is accepted as well. If the two
        // ever shared a counter there would be no way to tell a fleet
        // running ahead of this server from ordinary traffic.
        assert_eq!(c.unknown_platform_total(), 1);
        assert_eq!(c.snapshot()[0], ("accepted", 1));
        assert_eq!(c.snapshot()[1], ("rejected", 0));
    }

    #[test]
    fn each_counter_moves_independently() {
        let c = IngestCounters::default();
        c.accepted();
        c.accepted();
        c.rejected();
        c.failed();
        c.rate_limited();
        c.rate_limited();
        c.rate_limited();
        assert_eq!(
            c.snapshot(),
            [
                ("accepted", 2),
                ("rejected", 1),
                ("failed", 1),
                ("rate_limited", 3)
            ]
        );
    }
}
