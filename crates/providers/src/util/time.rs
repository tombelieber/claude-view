// crates/providers/src/util/time.rs
//
// Timestamp parsing shared across parsers. Foreign formats use (at least):
// RFC3339 with/without nanos, naive ISO without timezone, unix seconds as
// float, unix millis as int, and SQL-style "YYYY-MM-DD HH:MM:SS[.ffffff]".
// Everything normalizes to epoch seconds (f64, fractional part preserved).

use chrono::{DateTime, NaiveDateTime, TimeZone};

/// Parse a timestamp string. Returns epoch seconds or `None`.
///
/// `assume_local` controls naive (timezone-less) inputs: some producers
/// (e.g. Hermes) write local time — pass `true` to replicate; the default
/// for unknown producers is UTC (`false`).
pub fn parse_timestamp(s: &str, assume_local: bool) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // RFC3339 (with offset or Z), any sub-second precision.
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(to_epoch(dt.timestamp(), dt.timestamp_subsec_nanos()));
    }
    // Naive variants: ISO 'T' separator or SQL space separator.
    for fmt in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return naive_to_epoch(naive, assume_local);
        }
    }
    None
}

fn naive_to_epoch(naive: NaiveDateTime, assume_local: bool) -> Option<f64> {
    if assume_local {
        chrono::Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| to_epoch(dt.timestamp(), dt.timestamp_subsec_nanos()))
    } else {
        Some(to_epoch(
            naive.and_utc().timestamp(),
            naive.and_utc().timestamp_subsec_nanos(),
        ))
    }
}

fn to_epoch(secs: i64, nanos: u32) -> f64 {
    secs as f64 + f64::from(nanos) / 1e9
}

/// Epoch milliseconds (int or float JSON number) → epoch seconds.
pub fn from_millis(ms: f64) -> f64 {
    ms / 1000.0
}

/// Heuristic for numeric timestamps of unknown unit: values above ~1e12 are
/// millis (epoch-seconds won't exceed 1e12 until year 33658).
pub fn from_number(n: f64) -> Option<f64> {
    if !(n.is_finite() && n > 0.0) {
        return None;
    }
    Some(if n >= 1e12 { n / 1000.0 } else { n })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_variants() {
        assert_eq!(
            parse_timestamp("2026-01-02T03:04:05Z", false),
            Some(1767323045.0)
        );
        let nanos = parse_timestamp("2026-01-02T03:04:05.5Z", false).unwrap();
        assert!((nanos - 1767323045.5).abs() < 1e-6);
        assert!(parse_timestamp("2026-01-02T03:04:05+08:00", false).is_some());
    }

    #[test]
    fn naive_utc_and_sql() {
        let a = parse_timestamp("2026-01-02T03:04:05.014566", false).unwrap();
        let b = parse_timestamp("2026-01-02 03:04:05.014566", false).unwrap();
        assert!((a - b).abs() < 1e-9);
    }

    #[test]
    fn numeric_unit_heuristic() {
        assert_eq!(from_number(1767323045.0), Some(1767323045.0));
        assert_eq!(from_number(1767323045123.0), Some(1767323045.123));
        assert_eq!(from_number(-5.0), None);
    }
}
