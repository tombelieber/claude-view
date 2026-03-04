use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Canonical analytics data-scope vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsDataScope {
    PrimarySessionsOnly,
    PrimaryPlusSubagentWork,
}

/// Canonical `meta.dataScope` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsDataScopeMeta {
    pub sessions: AnalyticsDataScope,
    pub workload: AnalyticsDataScope,
}

impl Default for AnalyticsDataScopeMeta {
    fn default() -> Self {
        Self {
            sessions: AnalyticsDataScope::PrimarySessionsOnly,
            workload: AnalyticsDataScope::PrimaryPlusSubagentWork,
        }
    }
}

/// Canonical `meta.sessionBreakdown` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSessionBreakdown {
    #[ts(type = "number")]
    pub primary_sessions: i64,
    #[ts(type = "number")]
    pub sidechain_sessions: i64,
    #[ts(type = "number")]
    pub other_sessions: i64,
    #[ts(type = "number")]
    pub total_observed_sessions: i64,
}

impl AnalyticsSessionBreakdown {
    pub const fn new(primary_sessions: i64, sidechain_sessions: i64) -> Self {
        let other_sessions = 0;
        Self {
            primary_sessions,
            sidechain_sessions,
            other_sessions,
            total_observed_sessions: primary_sessions + sidechain_sessions + other_sessions,
        }
    }
}

impl Default for AnalyticsSessionBreakdown {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Canonical analytics scope metadata wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsScopeMeta {
    pub data_scope: AnalyticsDataScopeMeta,
    pub session_breakdown: AnalyticsSessionBreakdown,
}

impl AnalyticsScopeMeta {
    pub const fn new(session_breakdown: AnalyticsSessionBreakdown) -> Self {
        Self {
            data_scope: AnalyticsDataScopeMeta {
                sessions: AnalyticsDataScope::PrimarySessionsOnly,
                workload: AnalyticsDataScope::PrimaryPlusSubagentWork,
            },
            session_breakdown,
        }
    }
}
