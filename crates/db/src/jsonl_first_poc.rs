//! JSONL-first architecture POC — re-export shims + handlers.
//!
//! All core types have been extracted to `claude_view_core`:
//!   - `session_catalog` (step 1) — `SessionCatalog`, `CatalogRow`, `Filter`, `Sort`
//!   - `jsonl_reader` (step 2) — `open_reader`, `read_all`, `count_parseable`
//!   - `features` (step 3) — `Feature`, `FeatureRegistry`, `SessionEvent`, etc.
//!
//! This module contains:
//!   - Re-export shims (so bench binaries keep their imports unchanged)
//!   - The `handlers` module (pure handler functions for the new routes)
//!
//! The handlers module stays here until it too is extracted to the
//! server crate (steps 6-9 of the implementation plan).

/// Step 1 shim — maps POC names to core types.
pub mod session_index {
    pub use claude_view_core::session_catalog::{
        CatalogRow as SessionIndexRow, Filter, ProjectId, SessionCatalog as SessionIndex,
        SessionId, Sort, WalkStats,
    };
}

/// Step 2 shim.
pub mod jsonl_reader {
    pub use claude_view_core::jsonl_reader::{count_parseable, open_reader, read_all};
}

/// Step 3 shim.
pub mod features {
    pub use claude_view_core::features::*;
}

pub mod handlers {
    use serde::Serialize;

    use super::session_index::{Filter, SessionIndex, SessionIndexRow, Sort};

    #[derive(Debug, Clone, Serialize)]
    pub struct SessionListItem {
        pub id: String,
        pub project_id: String,
        pub file_path: String,
        pub is_compressed: bool,
        pub bytes: u64,
        pub mtime: i64,
    }

    impl From<SessionIndexRow> for SessionListItem {
        fn from(r: SessionIndexRow) -> Self {
            Self {
                id: r.id,
                project_id: r.project_id,
                file_path: r.file_path.to_string_lossy().into_owned(),
                is_compressed: r.is_compressed,
                bytes: r.bytes,
                mtime: r.mtime,
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct SessionsListResponse {
        pub total: usize,
        pub items: Vec<SessionListItem>,
    }

    pub fn list_sessions(
        idx: &SessionIndex,
        filter: &Filter,
        sort: Sort,
        limit: usize,
    ) -> SessionsListResponse {
        let rows = idx.list(filter, sort, limit);
        SessionsListResponse {
            total: idx.len(),
            items: rows.into_iter().map(Into::into).collect(),
        }
    }

    pub fn list_projects(idx: &SessionIndex) -> Vec<ProjectSummary> {
        let mut out: Vec<ProjectSummary> = idx
            .projects()
            .into_iter()
            .map(|(project_id, session_count)| ProjectSummary {
                project_id,
                session_count,
            })
            .collect();
        out.sort_unstable_by(|a, b| b.session_count.cmp(&a.session_count));
        out
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct ProjectSummary {
        pub project_id: String,
        pub session_count: usize,
    }

    // --- Step 7: session detail handler (reads JSONL on the fly) ---

    /// Derived session detail — computed from JSONL at read time.
    /// Replaces the current SQL-backed `GET /api/sessions/:id` which
    /// reads materialised `sessions.total_*` columns.
    #[derive(Debug, Clone, Serialize)]
    pub struct SessionDetail {
        pub id: String,
        pub project_id: String,
        pub is_compressed: bool,
        pub bytes: u64,
        pub mtime: i64,
        pub total_input_tokens: u64,
        pub total_output_tokens: u64,
        pub cache_read_tokens: u64,
        pub cache_creation_tokens: u64,
        pub turn_count: u32,
        pub line_count: u32,
    }

    /// Minimal typed shape for counting tokens from a JSONL file.
    #[derive(serde::Deserialize)]
    struct MinLine {
        #[serde(rename = "type")]
        line_type: Option<String>,
        #[serde(default)]
        message: Option<MinMsg>,
    }

    #[derive(serde::Deserialize)]
    struct MinMsg {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        usage: Option<MinUsage>,
    }

    #[derive(serde::Deserialize)]
    struct MinUsage {
        #[serde(default)]
        input_tokens: Option<u64>,
        #[serde(default)]
        output_tokens: Option<u64>,
        #[serde(default)]
        cache_read_input_tokens: Option<u64>,
        #[serde(default)]
        cache_creation_input_tokens: Option<u64>,
    }

    /// `GET /api/v2/sessions/:id` — reads the JSONL and computes
    /// token totals on the fly. No DB round-trip.
    pub fn get_session_detail(idx: &SessionIndex, session_id: &str) -> Option<SessionDetail> {
        let row = idx.get(session_id)?;
        let lines: Vec<MinLine> =
            claude_view_core::jsonl_reader::read_all(&row.file_path, row.is_compressed).ok()?;

        let mut total_input: u64 = 0;
        let mut total_output: u64 = 0;
        let mut cache_read: u64 = 0;
        let mut cache_create: u64 = 0;
        let mut turn_count: u32 = 0;
        let mut seen_msg_ids = std::collections::HashSet::new();

        for line in &lines {
            if line.line_type.as_deref() != Some("assistant") {
                continue;
            }
            let Some(ref msg) = line.message else {
                continue;
            };
            let Some(ref usage) = msg.usage else { continue };
            if let Some(ref mid) = msg.id {
                if !seen_msg_ids.insert(mid.clone()) {
                    continue;
                }
            }
            if msg.model.is_some() {
                turn_count += 1;
            }
            total_input += usage.input_tokens.unwrap_or(0);
            total_output += usage.output_tokens.unwrap_or(0);
            cache_read += usage.cache_read_input_tokens.unwrap_or(0);
            cache_create += usage.cache_creation_input_tokens.unwrap_or(0);
        }

        Some(SessionDetail {
            id: row.id,
            project_id: row.project_id,
            is_compressed: row.is_compressed,
            bytes: row.bytes,
            mtime: row.mtime,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_create,
            turn_count,
            line_count: lines.len() as u32,
        })
    }

    // --- Step 8: session turns handler (stream JSONL lines) ---

    /// One turn in the session — assistant message with usage.
    #[derive(Debug, Clone, Serialize)]
    pub struct TurnItem {
        pub seq: u32,
        pub model: Option<String>,
        pub input_tokens: Option<u64>,
        pub output_tokens: Option<u64>,
    }

    /// `GET /api/v2/sessions/:id/turns` — reads assistant lines from
    /// JSONL and returns them as typed turn items.
    pub fn get_session_turns(idx: &SessionIndex, session_id: &str) -> Option<Vec<TurnItem>> {
        let row = idx.get(session_id)?;
        let lines: Vec<MinLine> =
            claude_view_core::jsonl_reader::read_all(&row.file_path, row.is_compressed).ok()?;

        let mut turns = Vec::new();
        let mut seq: u32 = 0;
        let mut seen_msg_ids = std::collections::HashSet::new();

        for line in &lines {
            if line.line_type.as_deref() != Some("assistant") {
                continue;
            }
            let Some(ref msg) = line.message else {
                continue;
            };
            if let Some(ref mid) = msg.id {
                if !seen_msg_ids.insert(mid.clone()) {
                    continue;
                }
            }
            let model = msg.model.clone();
            let (inp, out) = match &msg.usage {
                Some(u) => (u.input_tokens, u.output_tokens),
                None => (None, None),
            };
            turns.push(TurnItem {
                seq,
                model,
                input_tokens: inp,
                output_tokens: out,
            });
            seq += 1;
        }

        Some(turns)
    }

    // --- Step 9: insights/benchmarks handler (reads from rollup) ---

    /// `GET /api/v2/insights/benchmarks` — reads from
    /// `AnalyticsRollupFeature` instead of `SUM(sessions.total_*)`.
    pub fn get_insights_benchmarks(
        rollup: &claude_view_core::analytics_rollup::AnalyticsRollupFeature,
        project_id: Option<&str>,
    ) -> InsightsBenchmarks {
        let filter = claude_view_core::analytics_rollup::RollupFilter {
            project_id: project_id.map(|s| s.to_string()),
            ..Default::default()
        };
        let total = rollup.total_sums(&filter);
        InsightsBenchmarks {
            total_input_tokens: total.input_tokens,
            total_output_tokens: total.output_tokens,
            cache_read_tokens: total.cache_read_tokens,
            cache_creation_tokens: total.cache_creation_tokens,
            session_count: total.session_count,
            bucket_count: total.bucket_count,
        }
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct InsightsBenchmarks {
        pub total_input_tokens: u64,
        pub total_output_tokens: u64,
        pub cache_read_tokens: u64,
        pub cache_creation_tokens: u64,
        pub session_count: u32,
        pub bucket_count: usize,
    }
}
