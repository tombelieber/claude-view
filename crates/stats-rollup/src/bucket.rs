//! Rollup time-bucket granularity.
//!
//! Phase 1 accepted this as a `rollup()` argument without branching on
//! it. Phase 4 adds [`Bucket::period_start_unix`] — the boundary-
//! alignment function Stage C uses to route each `StatsDelta` into the
//! correct pre-aggregated row.
//!
//! All boundaries are UTC-aligned; the display layer handles local-
//! timezone presentation at the API boundary. Mixing timezones at the
//! storage layer would break the "one row per bucket" invariant.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §2.2`.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};

/// Rollup bucket granularity.
///
/// Ordered from narrowest (`Daily`) to widest (`Monthly`). Consumers must
/// not assume `Monthly` == 30×`Daily` — calendar-month length varies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bucket {
    Daily,
    Weekly,
    Monthly,
}

impl Bucket {
    /// Unix seconds at the start of this bucket's boundary containing
    /// `ts_unix`. Always UTC-aligned.
    ///
    /// | Bucket   | Boundary                              |
    /// |----------|---------------------------------------|
    /// | `Daily`  | midnight UTC of that day              |
    /// | `Weekly` | Monday 00:00 UTC of that ISO week     |
    /// | `Monthly`| 1st 00:00 UTC of that calendar month  |
    ///
    /// Returns the original `ts_unix` unchanged only when `ts_unix`
    /// already coincides with a boundary (rare; useful for tests).
    ///
    /// # Panics
    /// Never — arithmetic uses `chrono`'s saturating date functions and
    /// the helper rejects `ts_unix` values outside `i64` range at the
    /// type boundary.
    pub fn period_start_unix(self, ts_unix: i64) -> i64 {
        let dt: DateTime<Utc> = Utc.timestamp_opt(ts_unix, 0).single().unwrap_or_else(|| {
            // Timestamp outside chrono's representable range — fall back
            // to epoch. Happens only for hostile inputs; production
            // timestamps are always in a normal decade.
            Utc.timestamp_opt(0, 0).unwrap()
        });
        match self {
            Bucket::Daily => {
                let d = dt.date_naive();
                unix_midnight_utc(d)
            }
            Bucket::Weekly => {
                // ISO week starts Monday. `weekday().num_days_from_monday()`
                // returns 0 for Monday, 6 for Sunday.
                let d = dt.date_naive();
                let monday = d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64);
                unix_midnight_utc(monday)
            }
            Bucket::Monthly => {
                let d = dt.date_naive();
                let first = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)
                    .expect("year+month+1 is always a valid date");
                unix_midnight_utc(first)
            }
        }
    }
}

fn unix_midnight_utc(d: NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0)
        .expect("00:00:00 is always valid")
        .and_utc()
        .timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    /// Build a Unix timestamp from a UTC `YYYY-MM-DD HH:MM:SS` tuple.
    /// Hand-rolled so the tests don't silently skew if a timezone lib
    /// default changes.
    fn ts(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> i64 {
        let date = NaiveDate::from_ymd_opt(y, m, d).expect("valid date");
        let time = NaiveTime::from_hms_opt(h, min, s).expect("valid time");
        date.and_time(time).and_utc().timestamp()
    }

    #[test]
    fn daily_aligns_to_midnight_utc() {
        let sample = ts(2026, 4, 19, 14, 30, 0);
        let expected = ts(2026, 4, 19, 0, 0, 0);
        assert_eq!(Bucket::Daily.period_start_unix(sample), expected);
    }

    #[test]
    fn weekly_aligns_to_monday() {
        // 2026-04-19 is a Sunday → previous Monday = 2026-04-13.
        let sample = ts(2026, 4, 19, 14, 30, 0);
        let expected = ts(2026, 4, 13, 0, 0, 0);
        assert_eq!(Bucket::Weekly.period_start_unix(sample), expected);
    }

    #[test]
    fn monthly_aligns_to_first_of_month() {
        let sample = ts(2026, 4, 19, 14, 30, 0);
        let expected = ts(2026, 4, 1, 0, 0, 0);
        assert_eq!(Bucket::Monthly.period_start_unix(sample), expected);
    }

    #[test]
    fn already_aligned_midnight_roundtrips_for_daily() {
        let midnight = ts(2026, 4, 19, 0, 0, 0);
        assert_eq!(Bucket::Daily.period_start_unix(midnight), midnight);
    }

    #[test]
    fn monday_roundtrips_for_weekly() {
        // 2026-04-13 was a Monday — Weekly bucket of a Monday midnight
        // must return the Monday itself.
        let monday_midnight = ts(2026, 4, 13, 0, 0, 0);
        assert_eq!(
            Bucket::Weekly.period_start_unix(monday_midnight),
            monday_midnight
        );
    }

    #[test]
    fn first_of_month_roundtrips_for_monthly() {
        let first = ts(2026, 4, 1, 0, 0, 0);
        assert_eq!(Bucket::Monthly.period_start_unix(first), first);
    }

    #[test]
    fn epoch_is_stable() {
        // Daily of epoch is epoch itself.
        assert_eq!(Bucket::Daily.period_start_unix(0), 0);
        // Monthly of epoch is epoch (1970-01-01 is already the 1st).
        assert_eq!(Bucket::Monthly.period_start_unix(0), 0);
        // Weekly: 1970-01-01 was a Thursday — previous Monday = 1969-12-29.
        let expected_monday = ts(1969, 12, 29, 0, 0, 0);
        assert_eq!(Bucket::Weekly.period_start_unix(0), expected_monday);
    }
}
