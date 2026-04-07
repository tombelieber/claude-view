//! Week period bounds calculations.

use chrono::{Datelike, Utc};

/// Get the bounds for the current week (Monday 00:00 UTC to now).
///
/// Returns `(start_timestamp, end_timestamp)` as Unix seconds.
pub fn current_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let days_since_monday = now.weekday().num_days_from_monday() as i64;
    let monday = now - chrono::Duration::days(days_since_monday);
    let start = monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end = now.timestamp();
    (start, end)
}

/// Get the bounds for the previous week (Monday 00:00 to Sunday 23:59:59 UTC).
///
/// Returns `(start_timestamp, end_timestamp)` as Unix seconds.
pub fn previous_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let days_since_monday = now.weekday().num_days_from_monday() as i64;
    let this_monday = now - chrono::Duration::days(days_since_monday);
    let prev_monday = this_monday - chrono::Duration::days(7);
    // Previous week ends at Sunday 23:59:59, which is this Monday 00:00:00 - 1 second
    let start = prev_monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let end = this_monday
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        - 1;
    (start, end)
}
