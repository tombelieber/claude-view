use std::panic::{catch_unwind, resume_unwind, UnwindSafe};
use std::sync::Mutex;

use claude_view_core::EffectiveRangeSource;
use claude_view_server::time_range::{
    resolve_from_to_or_all_time, resolve_range_param_or_all_time, ResolveFromToInput,
    ALLOW_LEGACY_ONE_SIDED_RANGES_ENV,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_legacy_env<T, F>(value: Option<&str>, f: F) -> T
where
    F: FnOnce() -> T + UnwindSafe,
{
    let _guard = ENV_LOCK.lock().expect("env lock");
    unsafe {
        if let Some(v) = value {
            std::env::set_var(ALLOW_LEGACY_ONE_SIDED_RANGES_ENV, v);
        } else {
            std::env::remove_var(ALLOW_LEGACY_ONE_SIDED_RANGES_ENV);
        }
    }

    let result = catch_unwind(f);
    unsafe {
        std::env::remove_var(ALLOW_LEGACY_ONE_SIDED_RANGES_ENV);
    }

    match result {
        Ok(v) => v,
        Err(panic) => resume_unwind(panic),
    }
}

fn from_to_input(
    from: Option<i64>,
    to: Option<i64>,
    now: i64,
    oldest: Option<i64>,
) -> ResolveFromToInput {
    ResolveFromToInput {
        endpoint: "test",
        from,
        to,
        now,
        oldest_timestamp: oldest,
    }
}

fn parse_trend_range(range: &str) -> Option<i64> {
    match range {
        "3mo" => Some(90 * 86400),
        "6mo" => Some(180 * 86400),
        "1yr" => Some(365 * 86400),
        "all" => Some(365 * 10 * 86400),
        _ => None,
    }
}

#[test]
fn explicit_pair_is_preserved() {
    with_legacy_env(None, || {
        let resolved = resolve_from_to_or_all_time(from_to_input(
            Some(1_700_000_000),
            Some(1_700_100_000),
            1_800_000_000,
            Some(1_600_000_000),
        ))
        .expect("explicit pair resolves");

        assert_eq!(resolved.from, 1_700_000_000);
        assert_eq!(resolved.to, 1_700_100_000);
        assert_eq!(resolved.source, EffectiveRangeSource::ExplicitFromTo);
    });
}

#[test]
fn one_sided_is_rejected_in_strict_mode() {
    with_legacy_env(None, || {
        let from_only = resolve_from_to_or_all_time(from_to_input(
            Some(1_700_000_000),
            None,
            1_800_000_000,
            None,
        ))
        .expect_err("from-only should be rejected");
        assert_eq!(from_only.reason.as_str(), "one_sided_input");

        let to_only = resolve_from_to_or_all_time(from_to_input(
            None,
            Some(1_700_000_000),
            1_800_000_000,
            None,
        ))
        .expect_err("to-only should be rejected");
        assert_eq!(to_only.reason.as_str(), "one_sided_input");
    });
}

#[test]
fn one_sided_is_coerced_in_legacy_mode() {
    with_legacy_env(Some("true"), || {
        let from_only = resolve_from_to_or_all_time(from_to_input(
            Some(1_700_000_000),
            None,
            1_800_000_000,
            None,
        ))
        .expect("from-only legacy coercion");
        assert_eq!(from_only.from, 1_700_000_000);
        assert_eq!(from_only.to, 1_800_000_000);
        assert_eq!(
            from_only.source,
            EffectiveRangeSource::LegacyOneSidedCoercion
        );

        let to_only = resolve_from_to_or_all_time(from_to_input(
            None,
            Some(1_700_000_000),
            1_800_000_000,
            Some(1_600_000_000),
        ))
        .expect("to-only legacy coercion");
        assert_eq!(to_only.from, 1_600_000_000);
        assert_eq!(to_only.to, 1_700_000_000);
        assert_eq!(to_only.source, EffectiveRangeSource::LegacyOneSidedCoercion);
    });
}

#[test]
fn inverted_range_is_rejected() {
    with_legacy_env(None, || {
        let err = resolve_from_to_or_all_time(from_to_input(
            Some(1_700_100_000),
            Some(1_700_000_000),
            1_800_000_000,
            None,
        ))
        .expect_err("inverted range should be rejected");

        assert_eq!(err.reason.as_str(), "inverted_range");
    });
}

#[test]
fn equal_range_is_valid() {
    with_legacy_env(None, || {
        let resolved = resolve_from_to_or_all_time(from_to_input(
            Some(1_700_000_000),
            Some(1_700_000_000),
            1_800_000_000,
            None,
        ))
        .expect("equality range should be valid");

        assert_eq!(resolved.from, 1_700_000_000);
        assert_eq!(resolved.to, 1_700_000_000);
        assert_eq!(resolved.source, EffectiveRangeSource::ExplicitFromTo);
    });
}

#[test]
fn default_all_time_uses_oldest_or_zero_fallback() {
    with_legacy_env(None, || {
        let with_oldest = resolve_from_to_or_all_time(from_to_input(
            None,
            None,
            1_800_000_000,
            Some(1_500_000_000),
        ))
        .expect("all-time with oldest should resolve");
        assert_eq!(with_oldest.from, 1_500_000_000);
        assert_eq!(with_oldest.to, 1_800_000_000);
        assert_eq!(with_oldest.source, EffectiveRangeSource::DefaultAllTime);

        let without_oldest =
            resolve_from_to_or_all_time(from_to_input(None, None, 1_800_000_000, None))
                .expect("all-time without oldest should resolve");
        assert_eq!(without_oldest.from, 0);
        assert_eq!(without_oldest.to, 1_800_000_000);
        assert_eq!(without_oldest.source, EffectiveRangeSource::DefaultAllTime);
    });
}

#[test]
fn range_param_resolver_precedence_is_from_to_then_range_then_default() {
    with_legacy_env(None, || {
        let explicit_pair = resolve_range_param_or_all_time(
            from_to_input(
                Some(1_700_000_000),
                Some(1_700_000_001),
                1_800_000_000,
                Some(1_600_000_000),
            ),
            Some("3mo"),
            &["3mo", "6mo", "1yr", "all"],
            parse_trend_range,
        )
        .expect("explicit pair should win");
        assert_eq!(explicit_pair.source, EffectiveRangeSource::ExplicitFromTo);
        assert_eq!(explicit_pair.from, 1_700_000_000);
        assert_eq!(explicit_pair.to, 1_700_000_001);

        let explicit_range = resolve_range_param_or_all_time(
            from_to_input(None, None, 1_800_000_000, Some(1_600_000_000)),
            Some("3mo"),
            &["3mo", "6mo", "1yr", "all"],
            parse_trend_range,
        )
        .expect("explicit range should resolve");
        assert_eq!(
            explicit_range.source,
            EffectiveRangeSource::ExplicitRangeParam
        );
        assert_eq!(explicit_range.from, 1_800_000_000 - 90 * 86400);
        assert_eq!(explicit_range.to, 1_800_000_000);

        let default_all_time = resolve_range_param_or_all_time(
            from_to_input(None, None, 1_800_000_000, Some(1_600_000_000)),
            None,
            &["3mo", "6mo", "1yr", "all"],
            parse_trend_range,
        )
        .expect("default all-time should resolve");
        assert_eq!(
            default_all_time.source,
            EffectiveRangeSource::DefaultAllTime
        );
        assert_eq!(default_all_time.from, 1_600_000_000);
        assert_eq!(default_all_time.to, 1_800_000_000);
    });
}
