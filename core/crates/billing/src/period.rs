//! UTC YYYYMM period helpers — pure functions, no I/O.
//!
//! Single-source for period key formatting + next-rollover
//! computation so quota dashboards + reset cron + tests
//! all agree on what "this period" means.

use time::{Date, Month, OffsetDateTime};

/// `YYYYMM` string in UTC. Stable string key for the
/// `usage_counters.period_yyyymm` column.
#[must_use]
pub fn period_key(now: OffsetDateTime) -> String {
    let utc = now.to_offset(time::UtcOffset::UTC);
    format!("{:04}{:02}", utc.year(), u8::from(utc.month()))
}

/// First instant of the next UTC month — when the
/// workspace's quota resets. Used in the 429 response body
/// + dashboard banner.
///
/// # Panics
///
/// Never — month math is closed over (1..=12) with the
/// year-roll branch handling December.
#[must_use]
pub fn next_period_start(now: OffsetDateTime) -> OffsetDateTime {
    let utc = now.to_offset(time::UtcOffset::UTC);
    let y = utc.year();
    let m = u8::from(utc.month());
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let next = Date::from_calendar_date(ny, Month::try_from(nm).expect("month 1..=12"), 1)
        .expect("valid 1st-of-month date");
    OffsetDateTime::new_utc(next, time::Time::MIDNIGHT)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn period_key_zero_pads() {
        let ts = datetime!(2026-03-15 10:00:00 UTC);
        assert_eq!(period_key(ts), "202603");
    }

    #[test]
    fn period_key_january() {
        let ts = datetime!(2026-01-01 00:00:00 UTC);
        assert_eq!(period_key(ts), "202601");
    }

    #[test]
    fn period_key_december() {
        let ts = datetime!(2026-12-31 23:59:59 UTC);
        assert_eq!(period_key(ts), "202612");
    }

    #[test]
    fn next_period_within_year() {
        let ts = datetime!(2026-03-15 10:00:00 UTC);
        assert_eq!(next_period_start(ts), datetime!(2026-04-01 00:00:00 UTC));
    }

    #[test]
    fn next_period_year_rollover() {
        let ts = datetime!(2026-12-15 10:00:00 UTC);
        assert_eq!(next_period_start(ts), datetime!(2027-01-01 00:00:00 UTC));
    }

    #[test]
    fn next_period_is_greater() {
        let ts = datetime!(2026-06-15 10:00:00 UTC);
        assert!(next_period_start(ts) > ts);
    }
}
