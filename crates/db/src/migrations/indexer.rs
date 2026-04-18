//! Indexer-side migrations 21–48.
//!
//! Covers the `session_facets` cache, `fluency_scores` history,
//! wall-clock task time, dead `pricing_cache`, `hook_events`, work
//! `reports`, the canonical `valid_sessions` view, registry fingerprint,
//! `app_settings`, derived session topology fields (cwd, parent,
//! kind, start_type, git_root), and the nine `index_runs` integrity
//! counters added in Migrations 40–48.
//!
//! Migration ordering is load-bearing. Append-only — never insert in the
//! middle and never reorder. Migrations 49+ live in `features.rs`.

pub const MIGRATIONS: &[&str] = &[
    // Migration 21a: session_facets table for /insights facet cache
    // NOTE: No FK to sessions — facets may reference sessions not yet indexed.
    // Join at query time instead.
    r#"
CREATE TABLE IF NOT EXISTS session_facets (
    session_id TEXT PRIMARY KEY,
    source TEXT NOT NULL DEFAULT 'insights_cache',
    underlying_goal TEXT,
    goal_categories TEXT NOT NULL DEFAULT '{}',
    outcome TEXT,
    satisfaction TEXT,
    user_satisfaction_counts TEXT NOT NULL DEFAULT '{}',
    claude_helpfulness TEXT,
    session_type TEXT,
    friction_counts TEXT NOT NULL DEFAULT '{}',
    friction_detail TEXT,
    primary_success TEXT,
    brief_summary TEXT,
    ingested_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    CONSTRAINT valid_outcome CHECK (outcome IN ('not_achieved', 'partially_achieved', 'mostly_achieved', 'fully_achieved', 'unclear_from_transcript') OR outcome IS NULL),
    CONSTRAINT valid_helpfulness CHECK (claude_helpfulness IN ('unhelpful', 'slightly_helpful', 'moderately_helpful', 'very_helpful', 'essential') OR claude_helpfulness IS NULL),
    CONSTRAINT valid_session_type CHECK (session_type IN ('single_task', 'multi_task', 'iterative_refinement', 'exploration', 'quick_question') OR session_type IS NULL),
    CONSTRAINT valid_source CHECK (source IN ('insights_cache', 'our_extraction', 'manual'))
);
"#,
    // Migration 21b: indexes for session_facets
    r#"CREATE INDEX IF NOT EXISTS idx_session_facets_outcome ON session_facets(outcome);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_facets_satisfaction ON session_facets(satisfaction);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_facets_helpfulness ON session_facets(claude_helpfulness);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_facets_ingested ON session_facets(ingested_at);"#,
    // Migration 21c: fluency_scores table for weekly score history
    r#"
CREATE TABLE IF NOT EXISTS fluency_scores (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    week_start TEXT NOT NULL,
    score INTEGER NOT NULL,
    achievement_rate REAL NOT NULL,
    friction_rate REAL NOT NULL,
    cost_efficiency REAL NOT NULL,
    satisfaction_trend REAL NOT NULL,
    consistency REAL NOT NULL,
    sessions_analyzed INTEGER NOT NULL,
    computed_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    CONSTRAINT valid_score CHECK (score BETWEEN 0 AND 100)
);
"#,
    r#"CREATE UNIQUE INDEX IF NOT EXISTS idx_fluency_scores_week ON fluency_scores(week_start);"#,
    // Migration 22: Wall-clock task time metrics
    r#"ALTER TABLE sessions ADD COLUMN total_task_time_seconds INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN longest_task_seconds INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN longest_task_preview TEXT;"#,
    // Migration 23: Pricing cache (DEAD — pricing now embedded via data/anthropic-pricing.json)
    r#"CREATE TABLE IF NOT EXISTS pricing_cache (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    data TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
)"#,
    // Migration 24: Hook events log for event timeline
    r#"
CREATE TABLE IF NOT EXISTS hook_events (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    event_name TEXT NOT NULL,
    tool_name TEXT,
    label TEXT NOT NULL,
    group_name TEXT NOT NULL,
    context TEXT
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_hook_events_session ON hook_events(session_id, timestamp);"#,
    // Migration 25: Work Reports table
    r#"
CREATE TABLE IF NOT EXISTS reports (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    report_type         TEXT NOT NULL,
    date_start          TEXT NOT NULL,
    date_end            TEXT NOT NULL,
    content_md          TEXT NOT NULL,
    context_digest      TEXT,
    session_count       INTEGER NOT NULL,
    project_count       INTEGER NOT NULL,
    total_duration_secs INTEGER NOT NULL,
    total_cost_cents    INTEGER NOT NULL,
    generation_ms       INTEGER,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    CONSTRAINT valid_report_type CHECK (report_type IN ('daily', 'weekly', 'custom'))
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_reports_date ON reports(date_start, date_end);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_reports_type ON reports(report_type);"#,
    // Migration 26: Canonical view for user-facing session queries.
    //
    // All user-facing queries should use `valid_sessions` instead of `sessions`
    // to ensure consistent filtering (no sidechains).
    // Historical note: this migration included a `last_message_at > 0` guard,
    // which was removed in migration 39.
    // System/classification queries that intentionally count ALL sessions
    // should continue using the `sessions` table directly.
    r#"CREATE VIEW IF NOT EXISTS valid_sessions AS SELECT * FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0;"#,
    // Migration 27: Registry fingerprint for auto-reindex on registry changes.
    // When plugins are installed/removed or user skills are added/deleted,
    // the stored hash differs from the live registry → triggers re-index.
    r#"ALTER TABLE index_metadata ADD COLUMN registry_hash TEXT;"#,
    // Migration 28: Unified LLM settings (replaces hardcoded "haiku" in classify + reports).
    // Default timeout is 120s (matches the existing report generation timeout, which is the
    // longest LLM operation). Classification also uses this timeout — 120s is fine for classify
    // since it only affects the max wait, not the typical latency.
    r#"BEGIN;
CREATE TABLE IF NOT EXISTS app_settings (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    llm_model        TEXT NOT NULL DEFAULT 'haiku',
    llm_timeout_secs INTEGER NOT NULL DEFAULT 120
);
INSERT OR IGNORE INTO app_settings (id) VALUES (1);
COMMIT;"#,
    // Migrations 29-31: Report generation metadata (model + token counts).
    // Separate entries so each ALTER is independently idempotent — if one column
    // already exists (branch collision), the others still get created.
    r#"ALTER TABLE reports ADD COLUMN generation_model TEXT;"#,
    r#"ALTER TABLE reports ADD COLUMN generation_input_tokens INTEGER;"#,
    r#"ALTER TABLE reports ADD COLUMN generation_output_tokens INTEGER;"#,
    // Migration 32: Session total cost storage.
    // REAL with no NOT NULL DEFAULT: NULL = "not yet parsed",
    // distinguishable from 0.0 (parsed but zero cost).
    r#"ALTER TABLE sessions ADD COLUMN total_cost_usd REAL;"#,
    // Migrations 33-36: Session topology fields for reliability release.
    // Separate entries so each ALTER is independently idempotent (branch collision safety).
    // Migration 33: session_cwd — raw cwd from JSONL (source of truth for project path)
    r#"ALTER TABLE sessions ADD COLUMN session_cwd TEXT;"#,
    // Migration 34: parent_session_id — parentUuid from first user line (fork/continuation)
    r#"ALTER TABLE sessions ADD COLUMN parent_session_id TEXT;"#,
    // Migration 35: session_kind — 'conversation' or 'metadata_only' (content classification)
    r#"ALTER TABLE sessions ADD COLUMN session_kind TEXT;"#,
    // Migration 36: start_type — first line type (user, file-history-snapshot, etc.)
    r#"ALTER TABLE sessions ADD COLUMN start_type TEXT;"#,
    // Migration 37: git_root — canonical git repository root resolved from session_cwd.
    // NULL = non-git directory or path deleted before indexing.
    r#"ALTER TABLE sessions ADD COLUMN git_root TEXT;"#,
    // Migration 38: index for fast project grouping by git root
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_git_root ON sessions (git_root);"#,
    // Migration 39: Remove last_message_at > 0 guard from valid_sessions.
    // No longer needed — the unified pipeline guarantees all rows have real timestamps.
    r#"DROP VIEW IF EXISTS valid_sessions;
CREATE VIEW valid_sessions AS SELECT * FROM sessions WHERE is_sidechain = 0;"#,
    // Migrations 40-48: Index run integrity counters (non-negative, default zero).
    r#"ALTER TABLE index_runs ADD COLUMN unknown_top_level_type_count INTEGER NOT NULL DEFAULT 0 CHECK (unknown_top_level_type_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN unknown_required_path_count INTEGER NOT NULL DEFAULT 0 CHECK (unknown_required_path_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN imaginary_path_access_count INTEGER NOT NULL DEFAULT 0 CHECK (imaginary_path_access_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN legacy_fallback_path_count INTEGER NOT NULL DEFAULT 0 CHECK (legacy_fallback_path_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN dropped_line_invalid_json_count INTEGER NOT NULL DEFAULT 0 CHECK (dropped_line_invalid_json_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN schema_mismatch_count INTEGER NOT NULL DEFAULT 0 CHECK (schema_mismatch_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN unknown_source_role_count INTEGER NOT NULL DEFAULT 0 CHECK (unknown_source_role_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN derived_source_message_doc_count INTEGER NOT NULL DEFAULT 0 CHECK (derived_source_message_doc_count >= 0);"#,
    r#"ALTER TABLE index_runs ADD COLUMN source_message_non_source_provenance_count INTEGER NOT NULL DEFAULT 0 CHECK (source_message_non_source_provenance_count >= 0);"#,
];
