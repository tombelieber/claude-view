// crates/db/src/snapshots/helpers.rs
//! Helper functions for snapshot date handling and unit conversions.

use super::types::{DailyTrendPoint, TimeRange};
use crate::Database;
use chrono::{Local, NaiveDate};

/// Fill gaps in sparse trend data so every date in [from, to] has an entry.
/// Days with no sessions get a zero-value DailyTrendPoint.
/// For unbounded ranges (All) where from..to spans > 366 days, only fills
/// from the first data point to `to` to avoid generating thousands of empty days.
pub(crate) fn fill_date_gaps(
    sparse: Vec<DailyTrendPoint>,
    from: &str,
    to: &str,
) -> Vec<DailyTrendPoint> {
    if sparse.is_empty() {
        return sparse;
    }
    let Ok(mut start) = NaiveDate::parse_from_str(from, "%Y-%m-%d") else {
        return sparse;
    };
    let Ok(end) = NaiveDate::parse_from_str(to, "%Y-%m-%d") else {
        return sparse;
    };
    if start > end {
        return sparse;
    }

    // For very wide ranges (e.g. "All" starting from 1970), only fill from
    // the first actual data point to avoid thousands of empty entries.
    let span_days = (end - start).num_days();
    if span_days > 366 {
        if let Ok(first) = NaiveDate::parse_from_str(&sparse[0].date, "%Y-%m-%d") {
            start = first;
        }
    }

    // Build a lookup from date string -> existing data point
    let mut by_date: std::collections::HashMap<String, DailyTrendPoint> =
        sparse.into_iter().map(|p| (p.date.clone(), p)).collect();

    let mut result = Vec::new();
    let mut current = start;
    while current <= end {
        let date_str = current.format("%Y-%m-%d").to_string();
        let point = by_date.remove(&date_str).unwrap_or(DailyTrendPoint {
            date: date_str,
            lines_added: 0,
            lines_removed: 0,
            commits: 0,
            sessions: 0,
            tokens_used: 0,
            cost_cents: 0,
        });
        result.push(point);
        current += chrono::Duration::days(1);
    }
    result
}

/// Convert real USD totals to whole cents.
pub(crate) fn usd_to_cents(total_cost_usd: f64) -> i64 {
    (total_cost_usd * 100.0).round() as i64
}

/// Convert optional USD totals to cents.
///
/// `SUM(total_cost_usd)` returns NULL when there are no priced rows.
/// We keep the legacy integer snapshot shape and map NULL to 0 cents.
pub(crate) fn usd_opt_to_cents(total_cost_usd: Option<f64>, _sessions_count: i64) -> i64 {
    total_cost_usd.map(usd_to_cents).unwrap_or(0)
}

impl Database {
    /// Convert TimeRange to (from, to) date strings.
    pub(crate) fn date_range_from_time_range(
        &self,
        range: TimeRange,
        from_date: Option<&str>,
        to_date: Option<&str>,
    ) -> (String, String) {
        match range {
            TimeRange::Today => {
                let today = Local::now().format("%Y-%m-%d").to_string();
                (today.clone(), today)
            }
            TimeRange::Custom => {
                let from = from_date.unwrap_or("1970-01-01").to_string();
                let to = to_date
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
                (from, to)
            }
            TimeRange::All => (
                "1970-01-01".to_string(),
                Local::now().format("%Y-%m-%d").to_string(),
            ),
            _ => {
                let days = range.days_back().unwrap_or(7);
                let from = (Local::now() - chrono::Duration::days(days))
                    .format("%Y-%m-%d")
                    .to_string();
                let to = Local::now().format("%Y-%m-%d").to_string();
                (from, to)
            }
        }
    }
}
