//! Property tests for `period_key` + `next_period_start`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_billing::{next_period_start, period_key};
use time::{Date, Month, OffsetDateTime, Time};

fn ts(year: i32, month: u8, day: u8) -> OffsetDateTime {
    let date = Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap();
    OffsetDateTime::new_utc(date, Time::MIDNIGHT)
}

proptest! {
    #[test]
    fn period_key_is_six_digits(
        year in 2024i32..2040,
        month in 1u8..=12,
        day in 1u8..=28,
    ) {
        let key = period_key(ts(year, month, day));
        prop_assert_eq!(key.len(), 6);
        prop_assert!(key.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn next_period_is_strictly_greater(
        year in 2024i32..2040,
        month in 1u8..=12,
        day in 1u8..=28,
    ) {
        let now = ts(year, month, day);
        prop_assert!(next_period_start(now) > now);
    }

    #[test]
    fn next_period_is_one_month_ahead(
        year in 2024i32..2040,
        month in 1u8..=11,
    ) {
        let now = ts(year, month, 15);
        let next = next_period_start(now);
        prop_assert_eq!(next.year(), year);
        prop_assert_eq!(u8::from(next.month()), month + 1);
        prop_assert_eq!(next.day(), 1);
    }

    #[test]
    fn december_rolls_to_january_next_year(
        year in 2024i32..2039,
    ) {
        let now = ts(year, 12, 15);
        let next = next_period_start(now);
        prop_assert_eq!(next.year(), year + 1);
        prop_assert_eq!(u8::from(next.month()), 1);
    }
}
