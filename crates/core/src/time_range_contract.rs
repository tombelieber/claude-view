use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Canonical source for how a request's effective time range was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum EffectiveRangeSource {
    ExplicitFromTo,
    ExplicitRangeParam,
    DefaultAllTime,
    LegacyOneSidedCoercion,
}

impl EffectiveRangeSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitFromTo => "explicit_from_to",
            Self::ExplicitRangeParam => "explicit_range_param",
            Self::DefaultAllTime => "default_all_time",
            Self::LegacyOneSidedCoercion => "legacy_one_sided_coercion",
        }
    }
}

/// Canonical resolved range metadata returned additively by API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct EffectiveRangeMeta {
    #[ts(type = "number")]
    pub from: i64,
    #[ts(type = "number")]
    pub to: i64,
    pub source: EffectiveRangeSource,
}
