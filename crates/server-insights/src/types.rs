// crates/server/src/insights/types.rs
//! Core types for the insight system.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A single insight with optional severity/type.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Insight {
    /// The insight text (plain English).
    pub text: String,
    /// Insight type for styling (info, success, warning, tip).
    #[serde(default)]
    pub kind: InsightKind,
}

/// Insight severity/type for UI styling.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS, utoipa::ToSchema,
)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum InsightKind {
    /// Neutral informational insight
    #[default]
    Info,
    /// Positive/encouraging insight
    Success,
    /// Warning or area of concern
    Warning,
    /// Actionable suggestion
    Tip,
}

impl Insight {
    /// Create a new info insight.
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Info,
        }
    }

    /// Create a new success insight.
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Success,
        }
    }

    /// Create a new warning insight.
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Warning,
        }
    }

    /// Create a new tip insight.
    pub fn tip(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: InsightKind::Tip,
        }
    }
}
