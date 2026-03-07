use claude_view_core::{EffectiveRangeMeta, EffectiveRangeSource};

pub const ALLOW_LEGACY_ONE_SIDED_RANGES_ENV: &str = "ALLOW_LEGACY_ONE_SIDED_RANGES";

#[derive(Debug, Clone, Copy)]
pub struct ResolveFromToInput {
    pub endpoint: &'static str,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub now: i64,
    pub oldest_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeRangeResolutionErrorReason {
    OneSidedInput,
    InvertedRange,
    InvalidRangeParam,
}

impl TimeRangeResolutionErrorReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneSidedInput => "one_sided_input",
            Self::InvertedRange => "inverted_range",
            Self::InvalidRangeParam => "invalid_range_param",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct TimeRangeResolutionError {
    pub reason: TimeRangeResolutionErrorReason,
    pub message: String,
}

impl TimeRangeResolutionError {
    fn one_sided_input() -> Self {
        Self {
            reason: TimeRangeResolutionErrorReason::OneSidedInput,
            message: "Both 'from' and 'to' must be provided together".to_string(),
        }
    }

    fn inverted_range() -> Self {
        Self {
            reason: TimeRangeResolutionErrorReason::InvertedRange,
            message: "'from' must be <= 'to'".to_string(),
        }
    }

    fn invalid_range_param(valid_values: &[&str]) -> Self {
        let message = if valid_values.is_empty() {
            "Invalid range parameter".to_string()
        } else {
            format!("Invalid range. Must be one of: {}", valid_values.join(", "))
        };
        Self {
            reason: TimeRangeResolutionErrorReason::InvalidRangeParam,
            message,
        }
    }
}

pub fn allow_legacy_one_sided_ranges() -> bool {
    std::env::var(ALLOW_LEGACY_ONE_SIDED_RANGES_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub fn resolve_from_to_or_all_time(
    input: ResolveFromToInput,
) -> Result<EffectiveRangeMeta, TimeRangeResolutionError> {
    if let (Some(from), Some(to)) = (input.from, input.to) {
        if from > to {
            return Err(TimeRangeResolutionError::inverted_range());
        }
        return Ok(EffectiveRangeMeta {
            from,
            to,
            source: EffectiveRangeSource::ExplicitFromTo,
        });
    }

    match (input.from, input.to) {
        (Some(from), None) | (None, Some(from)) if !allow_legacy_one_sided_ranges() => {
            Err(TimeRangeResolutionError::one_sided_input())
        }
        (Some(from), None) => {
            let to = input.now.max(from);
            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::LegacyOneSidedCoercion,
            })
        }
        (None, Some(to)) => {
            let from = input.oldest_timestamp.unwrap_or(0).min(to);
            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::LegacyOneSidedCoercion,
            })
        }
        (None, None) => {
            let from = input.oldest_timestamp.unwrap_or(0);
            let to = input.now.max(from);
            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::DefaultAllTime,
            })
        }
        (Some(_), Some(_)) => unreachable!(),
    }
}

pub fn resolve_range_param_or_all_time<F>(
    input: ResolveFromToInput,
    range_param: Option<&str>,
    valid_range_values: &[&str],
    range_to_seconds: F,
) -> Result<EffectiveRangeMeta, TimeRangeResolutionError>
where
    F: Fn(&str) -> Option<i64>,
{
    if let (Some(from), Some(to)) = (input.from, input.to) {
        if from > to {
            return Err(TimeRangeResolutionError::inverted_range());
        }
        return Ok(EffectiveRangeMeta {
            from,
            to,
            source: EffectiveRangeSource::ExplicitFromTo,
        });
    }

    match (input.from, input.to) {
        (Some(from), None) if !allow_legacy_one_sided_ranges() => {
            Err(TimeRangeResolutionError::one_sided_input())
        }
        (None, Some(_)) if !allow_legacy_one_sided_ranges() => {
            Err(TimeRangeResolutionError::one_sided_input())
        }
        (Some(from), None) => {
            let to = input.now.max(from);
            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::LegacyOneSidedCoercion,
            })
        }
        (None, Some(to)) => {
            let from = if let Some(range) = range_param {
                let seconds = range_to_seconds(range).ok_or_else(|| {
                    TimeRangeResolutionError::invalid_range_param(valid_range_values)
                })?;
                input.now.saturating_sub(seconds)
            } else {
                input.oldest_timestamp.unwrap_or(0)
            }
            .min(to);

            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::LegacyOneSidedCoercion,
            })
        }
        (None, None) => {
            if let Some(range) = range_param {
                let seconds = range_to_seconds(range).ok_or_else(|| {
                    TimeRangeResolutionError::invalid_range_param(valid_range_values)
                })?;
                return Ok(EffectiveRangeMeta {
                    from: input.now.saturating_sub(seconds),
                    to: input.now,
                    source: EffectiveRangeSource::ExplicitRangeParam,
                });
            }

            let from = input.oldest_timestamp.unwrap_or(0);
            let to = input.now.max(from);
            Ok(EffectiveRangeMeta {
                from,
                to,
                source: EffectiveRangeSource::DefaultAllTime,
            })
        }
        (Some(_), Some(_)) => unreachable!(),
    }
}
