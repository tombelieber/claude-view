// crates/db/src/queries/types.rs
// Public type definitions shared across query modules and exported to the server crate.

use serde::Serialize;
use ts_rs::TS;

/// Branch count for a project.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BranchCount {
    pub branch: Option<String>,
    #[ts(type = "number")]
    pub count: i64,
}

/// Indexer state entry returned from the database.
#[derive(Debug, Clone)]
pub struct IndexerEntry {
    pub file_path: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub indexed_at: i64,
}

/// An invocable (tool/skill/MCP) with its aggregated invocation count.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct InvocableWithCount {
    pub id: String,
    pub plugin_name: Option<String>,
    pub name: String,
    pub kind: String,
    pub description: String,
    #[ts(type = "number")]
    pub invocation_count: i64,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for InvocableWithCount {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            plugin_name: row.try_get("plugin_name")?,
            name: row.try_get("name")?,
            kind: row.try_get("kind")?,
            description: row.try_get("description")?,
            invocation_count: row.try_get("invocation_count")?,
            last_used_at: row.try_get("last_used_at")?,
        })
    }
}

/// A model record with aggregated usage stats (for GET /api/models).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ModelWithStats {
    pub id: String,
    pub provider: Option<String>,
    pub family: Option<String>,
    #[ts(type = "number | null")]
    pub first_seen: Option<i64>,
    #[ts(type = "number | null")]
    pub last_seen: Option<i64>,
    #[ts(type = "number")]
    pub total_turns: i64,
    #[ts(type = "number")]
    pub total_sessions: i64,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for ModelWithStats {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            provider: row.try_get("provider")?,
            family: row.try_get("family")?,
            first_seen: row.try_get("first_seen")?,
            last_seen: row.try_get("last_seen")?,
            total_turns: row.try_get("total_turns")?,
            total_sessions: row.try_get("total_sessions")?,
        })
    }
}

/// Aggregate token usage statistics (for GET /api/stats/tokens).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokenStats {
    #[ts(type = "number")]
    pub total_input_tokens: u64,
    #[ts(type = "number")]
    pub total_output_tokens: u64,
    #[ts(type = "number")]
    pub total_cache_read_tokens: u64,
    #[ts(type = "number")]
    pub total_cache_creation_tokens: u64,
    pub cache_hit_ratio: f64,
    #[ts(type = "number")]
    pub turns_count: u64,
    #[ts(type = "number")]
    pub sessions_count: u64,
}

/// Token usage breakdown by model.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokensByModel {
    pub model: String,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
}

/// Token usage breakdown by project.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TokensByProject {
    pub project: String,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
}

/// Aggregate cost breakdown in USD (computed from per-model token data + pricing engine).
#[derive(Debug, Clone, Default, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AggregateCostBreakdown {
    pub total_cost_usd: f64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_creation_cost_usd: f64,
    pub cache_savings_usd: f64,
}

/// AI Generation statistics (for GET /api/stats/ai-generation).
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AIGenerationStats {
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub files_created: i64,
    #[ts(type = "number")]
    pub total_input_tokens: i64,
    #[ts(type = "number")]
    pub total_output_tokens: i64,
    #[ts(type = "number")]
    pub cache_read_tokens: i64,
    #[ts(type = "number")]
    pub cache_creation_tokens: i64,
    pub tokens_by_model: Vec<TokensByModel>,
    pub tokens_by_project: Vec<TokensByProject>,
    /// Aggregate cost breakdown (computed server-side using pricing engine).
    pub cost: AggregateCostBreakdown,
}

/// Storage statistics for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    #[ts(type = "number")]
    pub jsonl_bytes: u64,
    #[ts(type = "number")]
    pub index_bytes: u64,
    #[ts(type = "number")]
    pub db_bytes: u64,
    #[ts(type = "number")]
    pub cache_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

/// Health status enum for the system page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Error,
}

/// Health statistics for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct HealthStats {
    #[ts(type = "number")]
    pub sessions_count: i64,
    #[ts(type = "number")]
    pub commits_count: i64,
    #[ts(type = "number")]
    pub projects_count: i64,
    #[ts(type = "number")]
    pub errors_count: i64,
    #[ts(type = "number | null")]
    pub last_sync_at: Option<i64>,
    pub status: HealthStatus,
}

/// Classification status summary for the system page.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationStatus {
    #[ts(type = "number")]
    pub classified_count: i64,
    #[ts(type = "number")]
    pub unclassified_count: i64,
    pub last_run_at: Option<String>,
    #[ts(type = "number | null")]
    pub last_run_duration_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub last_run_cost_cents: Option<i64>,
    pub provider: String,
    pub model: String,
    pub is_running: bool,
    #[ts(type = "number | null")]
    pub progress: Option<i64>,
}

/// Aggregate statistics overview for the API.
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    #[ts(type = "number")]
    pub total_sessions: i64,
    #[ts(type = "number")]
    pub total_invocations: i64,
    #[ts(type = "number")]
    pub unique_invocables_used: i64,
    pub top_invocables: Vec<InvocableWithCount>,
}
