/// Inline SQL migrations for claude-view database schema.
///
/// We use simple inline migrations rather than sqlx migration files
/// because the schema is small and self-contained.
pub const MIGRATIONS: &[&str] = &[
    // Migration 1: sessions table
    r#"
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT,
    preview TEXT NOT NULL DEFAULT '',
    turn_count INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    first_message_at INTEGER,
    last_message_at INTEGER,
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,
    indexed_at INTEGER,
    project_path TEXT NOT NULL DEFAULT '',
    project_display_name TEXT NOT NULL DEFAULT '',
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_message TEXT NOT NULL DEFAULT '',
    files_touched TEXT NOT NULL DEFAULT '[]',
    skills_used TEXT NOT NULL DEFAULT '[]',
    tool_counts_edit INTEGER NOT NULL DEFAULT 0,
    tool_counts_read INTEGER NOT NULL DEFAULT 0,
    tool_counts_bash INTEGER NOT NULL DEFAULT 0,
    tool_counts_write INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0
);
"#,
    // Migration 2: sessions indexes
    r#"
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
"#,
    r#"
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);
"#,
    // Migration 3: indexer_state table
    r#"
CREATE TABLE IF NOT EXISTS indexer_state (
    file_path TEXT PRIMARY KEY,
    file_size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL
);
"#,
    // Migration 4: add session index fields
    r#"ALTER TABLE sessions ADD COLUMN summary TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN git_branch TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN is_sidechain BOOLEAN NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN deep_indexed_at INTEGER;"#,
    // Migration 5: invocables + invocations tables
    r#"
CREATE TABLE IF NOT EXISTS invocables (
    id          TEXT PRIMARY KEY,
    plugin_name TEXT,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    description TEXT DEFAULT '',
    status      TEXT DEFAULT 'enabled'
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS invocations (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    invocable_id TEXT NOT NULL REFERENCES invocables(id),
    session_id   TEXT NOT NULL,
    project      TEXT NOT NULL,
    timestamp    INTEGER NOT NULL,
    PRIMARY KEY (source_file, byte_offset)
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_invocable ON invocations(invocable_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_session   ON invocations(session_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_invocations_timestamp ON invocations(timestamp);"#,
    // Migration 6: models + turns tables (Phase 2B: token & model tracking)
    r#"
CREATE TABLE IF NOT EXISTS models (
    id         TEXT PRIMARY KEY,
    provider   TEXT,
    family     TEXT,
    first_seen INTEGER,
    last_seen  INTEGER
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS turns (
    session_id            TEXT NOT NULL,
    uuid                  TEXT NOT NULL,
    seq                   INTEGER NOT NULL,
    model_id              TEXT REFERENCES models(id),
    parent_uuid           TEXT,
    content_type          TEXT,
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cache_read_tokens     INTEGER,
    cache_creation_tokens INTEGER,
    service_tier          TEXT,
    timestamp             INTEGER,
    PRIMARY KEY (session_id, uuid)
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id, seq);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_turns_model   ON turns(model_id);"#,
    // Migration 7: Phase 2C indexes for branch/sidechain filtering
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_project_branch ON sessions(project_id, git_branch);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_sidechain ON sessions(is_sidechain);"#,
    // Migration 8: Phase 3 metrics engine schema
    // 8a: Add new session columns
    r#"ALTER TABLE sessions ADD COLUMN user_prompt_count INTEGER NOT NULL DEFAULT 0 CHECK (user_prompt_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN api_call_count INTEGER NOT NULL DEFAULT 0 CHECK (api_call_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN files_read TEXT NOT NULL DEFAULT '[]';"#,
    r#"ALTER TABLE sessions ADD COLUMN files_edited TEXT NOT NULL DEFAULT '[]';"#,
    r#"ALTER TABLE sessions ADD COLUMN files_read_count INTEGER NOT NULL DEFAULT 0 CHECK (files_read_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN files_edited_count INTEGER NOT NULL DEFAULT 0 CHECK (files_edited_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN reedited_files_count INTEGER NOT NULL DEFAULT 0 CHECK (reedited_files_count >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN commit_count INTEGER NOT NULL DEFAULT 0 CHECK (commit_count >= 0);"#,
    // 8b: Create commits table
    r#"
CREATE TABLE IF NOT EXISTS commits (
    hash            TEXT PRIMARY KEY,
    repo_path       TEXT NOT NULL,
    message         TEXT NOT NULL,
    author          TEXT,
    timestamp       INTEGER NOT NULL,
    branch          TEXT,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
"#,
    // 8c: Create session_commits junction table
    r#"
CREATE TABLE IF NOT EXISTS session_commits (
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL REFERENCES commits(hash) ON DELETE CASCADE,
    tier            INTEGER NOT NULL CHECK (tier IN (1, 2)),
    evidence        TEXT NOT NULL DEFAULT '{}',
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (session_id, commit_hash)
);
"#,
    // 8d: Create index_metadata singleton table
    r#"
CREATE TABLE IF NOT EXISTS index_metadata (
    id                      INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_indexed_at         INTEGER,
    last_index_duration_ms  INTEGER,
    sessions_indexed        INTEGER NOT NULL DEFAULT 0,
    projects_indexed        INTEGER NOT NULL DEFAULT 0,
    last_git_sync_at        INTEGER,
    commits_found           INTEGER NOT NULL DEFAULT 0,
    links_created           INTEGER NOT NULL DEFAULT 0,
    updated_at              INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
"#,
    r#"INSERT OR IGNORE INTO index_metadata (id) VALUES (1);"#,
    // 8e: Create indexes for commits and session_commits
    r#"CREATE INDEX IF NOT EXISTS idx_commits_repo_ts ON commits(repo_path, timestamp DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_commits_timestamp ON commits(timestamp DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_commits_session ON session_commits(session_id);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_session_commits_commit ON session_commits(commit_hash);"#,
    // 8f: Create indexes for new session columns
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_commit_count ON sessions(commit_count) WHERE commit_count > 0;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_reedit ON sessions(reedited_files_count) WHERE reedited_files_count > 0;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_duration ON sessions(duration_seconds);"#,
    // Migration 9: Add user-configurable git sync interval to settings
    r#"ALTER TABLE index_metadata ADD COLUMN git_sync_interval_secs INTEGER NOT NULL DEFAULT 60;"#,
    // Migration 10: Full JSONL parser schema (Phase 3.5)
    // 10a: Token aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN total_input_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN total_output_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN cache_creation_tokens INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN thinking_block_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10b: System line aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_avg_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_max_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN turn_duration_total_ms INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN api_error_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN api_retry_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN compaction_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN hook_blocked_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10c: Progress line aggregates on sessions
    r#"ALTER TABLE sessions ADD COLUMN agent_spawn_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN bash_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN hook_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN mcp_progress_count INTEGER NOT NULL DEFAULT 0;"#,
    // 10d: Summary + parse_version
    r#"ALTER TABLE sessions ADD COLUMN summary_text TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;"#,
    // 10e: Detail tables
    r#"
CREATE TABLE IF NOT EXISTS turn_metrics (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_seq INTEGER NOT NULL,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    model TEXT,
    PRIMARY KEY (session_id, turn_seq)
);
"#,
    r#"
CREATE TABLE IF NOT EXISTS api_errors (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,
    retry_attempt INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    retry_in_ms REAL NOT NULL DEFAULT 0.0
);
"#,
    r#"CREATE INDEX IF NOT EXISTS idx_api_errors_session_id ON api_errors(session_id);"#,
    // 10f: Partial index for re-indexing
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_needs_reindex ON sessions(id, file_path) WHERE parse_version < 1;"#,
    // Migration 11: File modification detection for re-indexing
    // Stores file size and mtime at last deep-index time so we can detect
    // when a JSONL file has grown (user continued a conversation).
    r#"ALTER TABLE sessions ADD COLUMN file_size_at_index INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN file_mtime_at_index INTEGER;"#,
    // Migration 12: Drop unused indexes on invocations table.
    // idx_invocations_session and idx_invocations_timestamp are never queried —
    // they only slow down bulk writes (~24k unnecessary B-tree ops per reindex).
    r#"DROP INDEX IF EXISTS idx_invocations_session;"#,
    r#"DROP INDEX IF EXISTS idx_invocations_timestamp;"#,
    // Migration 13: Add lines_added and lines_removed to sessions (Phase C: LOC estimation)
    r#"ALTER TABLE sessions ADD COLUMN lines_added INTEGER NOT NULL DEFAULT 0 CHECK (lines_added >= 0);"#,
    r#"ALTER TABLE sessions ADD COLUMN lines_removed INTEGER NOT NULL DEFAULT 0 CHECK (lines_removed >= 0);"#,
    // Source tracking: 0 = not computed, 1 = tool-call estimate, 2 = git diff
    r#"ALTER TABLE sessions ADD COLUMN loc_source INTEGER NOT NULL DEFAULT 0 CHECK (loc_source IN (0, 1, 2));"#,
    // Migration 14: Theme 3 contribution tracking fields
    // 14a: Add AI contribution metrics to sessions table
    r#"ALTER TABLE sessions ADD COLUMN ai_lines_added INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN ai_lines_removed INTEGER NOT NULL DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN work_type TEXT;"#,  // 'deep_work', 'quick_ask', 'planning', 'bug_fix', 'standard'
    // 14b: Add diff stats to commits table (nullable: diff stats may not be available initially)
    r#"ALTER TABLE commits ADD COLUMN files_changed INTEGER;"#,
    r#"ALTER TABLE commits ADD COLUMN insertions INTEGER;"#,
    r#"ALTER TABLE commits ADD COLUMN deletions INTEGER;"#,
    // 14c: Add contribution_snapshots table for daily aggregates
    r#"
CREATE TABLE IF NOT EXISTS contribution_snapshots (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,              -- YYYY-MM-DD
    project_id TEXT,                 -- NULL for global
    branch TEXT,                     -- NULL for project-wide
    sessions_count INTEGER DEFAULT 0,
    ai_lines_added INTEGER DEFAULT 0,
    ai_lines_removed INTEGER DEFAULT 0,
    commits_count INTEGER DEFAULT 0,
    commit_insertions INTEGER DEFAULT 0,
    commit_deletions INTEGER DEFAULT 0,
    tokens_used INTEGER DEFAULT 0,
    cost_cents INTEGER DEFAULT 0,
    UNIQUE(date, project_id, branch)
);
"#,
    // 14d: Indexes for contribution_snapshots
    r#"CREATE INDEX IF NOT EXISTS idx_snapshots_date ON contribution_snapshots(date);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_snapshots_project_date ON contribution_snapshots(project_id, date);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_snapshots_branch_date ON contribution_snapshots(project_id, branch, date);"#,
    // Migration 15: Add files_edited_count to contribution_snapshots
    // This replaces the fabricated estimate_files_count (lines/50) with real data.
    r#"ALTER TABLE contribution_snapshots ADD COLUMN files_edited_count INTEGER NOT NULL DEFAULT 0;"#,
    // Migration 16: Dashboard analytics indexes (Phase 2A time-range filtering)
    // 16a: Add primary_model column for efficient model-based grouping
    // This denormalizes the most-used model from turns table to avoid expensive subqueries.
    r#"ALTER TABLE sessions ADD COLUMN primary_model TEXT;"#,
    // 16b: Index for time-range queries on first_message_at
    // Supports "sessions started in date range" queries.
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_first_message ON sessions(first_message_at);"#,
    // 16c: Composite index for time-range queries scoped to project
    // Supports "sessions in project X during date range" queries.
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_project_first_message ON sessions(project_id, first_message_at);"#,
    // 16d: Index for model-based grouping and filtering
    // Supports "token usage by model" queries in AI Generation Breakdown.
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_primary_model ON sessions(primary_model);"#,
    // Migration 17: Add CASCADE FKs to turns and invocations tables.
    // SQLite can't ALTER TABLE to add constraints, so we recreate the tables.
    // IMPORTANT: This is a multi-statement migration executed via sqlx::raw_sql()
    // within a single transaction to prevent data loss if the app crashes mid-migration.
    // The migration runner detects multi-statement migrations (containing ";\n") and
    // uses raw_sql() instead of query().
    r#"
BEGIN;
CREATE TABLE turns_new (
    session_id            TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    uuid                  TEXT NOT NULL,
    seq                   INTEGER NOT NULL,
    model_id              TEXT REFERENCES models(id),
    parent_uuid           TEXT,
    content_type          TEXT,
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cache_read_tokens     INTEGER,
    cache_creation_tokens INTEGER,
    service_tier          TEXT,
    timestamp             INTEGER,
    PRIMARY KEY (session_id, uuid)
);
INSERT INTO turns_new SELECT * FROM turns;
DROP TABLE turns;
ALTER TABLE turns_new RENAME TO turns;
CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_turns_model ON turns(model_id);
CREATE TABLE invocations_new (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    invocable_id TEXT NOT NULL REFERENCES invocables(id),
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    project      TEXT NOT NULL,
    timestamp    INTEGER NOT NULL,
    PRIMARY KEY (source_file, byte_offset)
);
INSERT INTO invocations_new SELECT * FROM invocations;
DROP TABLE invocations;
ALTER TABLE invocations_new RENAME TO invocations;
CREATE INDEX IF NOT EXISTS idx_invocations_invocable ON invocations(invocable_id);
COMMIT;
"#,
    // Migration 18: Drop dead `file_hash` column from sessions table.
    // SQLite requires table recreation to drop a column.
    // IMPORTANT: Multi-statement migration in a single transaction via raw_sql()
    // to prevent data loss if the app crashes between DROP and RENAME.
    r#"
BEGIN;
CREATE TABLE sessions_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT,
    preview TEXT NOT NULL DEFAULT '',
    turn_count INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    first_message_at INTEGER,
    last_message_at INTEGER,
    file_path TEXT NOT NULL UNIQUE,
    indexed_at INTEGER,
    project_path TEXT NOT NULL DEFAULT '',
    project_display_name TEXT NOT NULL DEFAULT '',
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_message TEXT NOT NULL DEFAULT '',
    files_touched TEXT NOT NULL DEFAULT '[]',
    skills_used TEXT NOT NULL DEFAULT '[]',
    tool_counts_edit INTEGER NOT NULL DEFAULT 0,
    tool_counts_read INTEGER NOT NULL DEFAULT 0,
    tool_counts_bash INTEGER NOT NULL DEFAULT 0,
    tool_counts_write INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0,
    summary TEXT,
    git_branch TEXT,
    is_sidechain BOOLEAN NOT NULL DEFAULT 0,
    deep_indexed_at INTEGER,
    user_prompt_count INTEGER NOT NULL DEFAULT 0 CHECK (user_prompt_count >= 0),
    api_call_count INTEGER NOT NULL DEFAULT 0 CHECK (api_call_count >= 0),
    tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0),
    files_read TEXT NOT NULL DEFAULT '[]',
    files_edited TEXT NOT NULL DEFAULT '[]',
    files_read_count INTEGER NOT NULL DEFAULT 0 CHECK (files_read_count >= 0),
    files_edited_count INTEGER NOT NULL DEFAULT 0 CHECK (files_edited_count >= 0),
    reedited_files_count INTEGER NOT NULL DEFAULT 0 CHECK (reedited_files_count >= 0),
    duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0),
    commit_count INTEGER NOT NULL DEFAULT 0 CHECK (commit_count >= 0),
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
    thinking_block_count INTEGER NOT NULL DEFAULT 0,
    turn_duration_avg_ms INTEGER,
    turn_duration_max_ms INTEGER,
    turn_duration_total_ms INTEGER,
    api_error_count INTEGER NOT NULL DEFAULT 0,
    api_retry_count INTEGER NOT NULL DEFAULT 0,
    compaction_count INTEGER NOT NULL DEFAULT 0,
    hook_blocked_count INTEGER NOT NULL DEFAULT 0,
    agent_spawn_count INTEGER NOT NULL DEFAULT 0,
    bash_progress_count INTEGER NOT NULL DEFAULT 0,
    hook_progress_count INTEGER NOT NULL DEFAULT 0,
    mcp_progress_count INTEGER NOT NULL DEFAULT 0,
    summary_text TEXT,
    parse_version INTEGER NOT NULL DEFAULT 0,
    file_size_at_index INTEGER,
    file_mtime_at_index INTEGER,
    lines_added INTEGER NOT NULL DEFAULT 0 CHECK (lines_added >= 0),
    lines_removed INTEGER NOT NULL DEFAULT 0 CHECK (lines_removed >= 0),
    loc_source INTEGER NOT NULL DEFAULT 0 CHECK (loc_source IN (0, 1, 2)),
    ai_lines_added INTEGER NOT NULL DEFAULT 0,
    ai_lines_removed INTEGER NOT NULL DEFAULT 0,
    work_type TEXT,
    primary_model TEXT
);
INSERT INTO sessions_new (
    id, project_id, title, preview, turn_count, file_count,
    first_message_at, last_message_at, file_path, indexed_at,
    project_path, project_display_name, size_bytes, last_message,
    files_touched, skills_used,
    tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
    message_count, summary, git_branch, is_sidechain, deep_indexed_at,
    user_prompt_count, api_call_count, tool_call_count,
    files_read, files_edited, files_read_count, files_edited_count,
    reedited_files_count, duration_seconds, commit_count,
    total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
    thinking_block_count, turn_duration_avg_ms, turn_duration_max_ms, turn_duration_total_ms,
    api_error_count, api_retry_count, compaction_count, hook_blocked_count,
    agent_spawn_count, bash_progress_count, hook_progress_count, mcp_progress_count,
    summary_text, parse_version, file_size_at_index, file_mtime_at_index,
    lines_added, lines_removed, loc_source,
    ai_lines_added, ai_lines_removed, work_type, primary_model
)
SELECT
    id, project_id, title, preview, turn_count, file_count,
    first_message_at, last_message_at, file_path, indexed_at,
    project_path, project_display_name, size_bytes, last_message,
    files_touched, skills_used,
    tool_counts_edit, tool_counts_read, tool_counts_bash, tool_counts_write,
    message_count, summary, git_branch, is_sidechain, deep_indexed_at,
    user_prompt_count, api_call_count, tool_call_count,
    files_read, files_edited, files_read_count, files_edited_count,
    reedited_files_count, duration_seconds, commit_count,
    total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
    thinking_block_count, turn_duration_avg_ms, turn_duration_max_ms, turn_duration_total_ms,
    api_error_count, api_retry_count, compaction_count, hook_blocked_count,
    agent_spawn_count, bash_progress_count, hook_progress_count, mcp_progress_count,
    summary_text, parse_version, file_size_at_index, file_mtime_at_index,
    lines_added, lines_removed, loc_source,
    ai_lines_added, ai_lines_removed, work_type, primary_model
FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;
CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_project_branch ON sessions(project_id, git_branch);
CREATE INDEX IF NOT EXISTS idx_sessions_sidechain ON sessions(is_sidechain);
CREATE INDEX IF NOT EXISTS idx_sessions_commit_count ON sessions(commit_count) WHERE commit_count > 0;
CREATE INDEX IF NOT EXISTS idx_sessions_reedit ON sessions(reedited_files_count) WHERE reedited_files_count > 0;
CREATE INDEX IF NOT EXISTS idx_sessions_duration ON sessions(duration_seconds);
CREATE INDEX IF NOT EXISTS idx_sessions_needs_reindex ON sessions(id, file_path) WHERE parse_version < 1;
CREATE INDEX IF NOT EXISTS idx_sessions_first_message ON sessions(first_message_at);
CREATE INDEX IF NOT EXISTS idx_sessions_project_first_message ON sessions(project_id, first_message_at);
CREATE INDEX IF NOT EXISTS idx_sessions_primary_model ON sessions(primary_model);
COMMIT;
"#,
    // Migration 19: Normalize empty/whitespace-only git_branch values to NULL.
    // Fixes duplicate "(no branch)" entries caused by NULL vs '' in GROUP BY.
    r#"UPDATE sessions SET git_branch = NULL WHERE git_branch = '' OR TRIM(git_branch) = '';"#,
    // Migration 20: Theme 4 Foundation
    // 20a: Classification hierarchy columns on sessions
    r#"ALTER TABLE sessions ADD COLUMN category_l1 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_l2 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_l3 TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_confidence REAL;"#,
    r#"ALTER TABLE sessions ADD COLUMN category_source TEXT;"#,
    r#"ALTER TABLE sessions ADD COLUMN classified_at TEXT;"#,
    // 20b: Behavioral metrics columns on sessions
    r#"ALTER TABLE sessions ADD COLUMN prompt_word_count INTEGER;"#,
    r#"ALTER TABLE sessions ADD COLUMN correction_count INTEGER DEFAULT 0;"#,
    r#"ALTER TABLE sessions ADD COLUMN same_file_edit_count INTEGER DEFAULT 0;"#,
    // 20c: classification_jobs table
    r#"
CREATE TABLE IF NOT EXISTS classification_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    total_sessions INTEGER NOT NULL,
    classified_count INTEGER DEFAULT 0,
    skipped_count INTEGER DEFAULT 0,
    failed_count INTEGER DEFAULT 0,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT DEFAULT 'running',
    error_message TEXT,
    cost_estimate_cents INTEGER,
    actual_cost_cents INTEGER,
    tokens_used INTEGER,
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
);
"#,
    // 20d: index_runs table
    r#"
CREATE TABLE IF NOT EXISTS index_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    type TEXT NOT NULL,
    sessions_before INTEGER,
    sessions_after INTEGER,
    duration_ms INTEGER,
    throughput_mb_per_sec REAL,
    status TEXT DEFAULT 'running',
    error_message TEXT,
    CONSTRAINT valid_type CHECK (type IN ('full', 'incremental', 'deep')),
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'failed'))
);
"#,
    // 20e: Indexes for classification and index_runs
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_category_l1 ON sessions(category_l1) WHERE category_l1 IS NOT NULL;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_sessions_classified ON sessions(classified_at) WHERE classified_at IS NOT NULL;"#,
    r#"CREATE INDEX IF NOT EXISTS idx_classification_jobs_status ON classification_jobs(status);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_classification_jobs_started ON classification_jobs(started_at DESC);"#,
    r#"CREATE INDEX IF NOT EXISTS idx_index_runs_started ON index_runs(started_at DESC);"#,
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
    // Migration 23: Pricing cache for three-tier resolution (litellm → SQLite → defaults)
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
    // to ensure consistent filtering (no sidechains, valid timestamps).
    // System/classification queries that intentionally count ALL sessions
    // should continue using the `sessions` table directly.
    r#"CREATE VIEW IF NOT EXISTS valid_sessions AS SELECT * FROM sessions WHERE is_sidechain = 0 AND last_message_at > 0;"#,
    // Migration 27: costUSD parity — accumulate per-entry costUSD from JSONL
    // REAL with no NOT NULL DEFAULT: NULL = "not yet parsed with costUSD support",
    // distinguishable from 0.0 (parsed but zero cost).
    r#"ALTER TABLE sessions ADD COLUMN total_cost_usd REAL;"#,
];

// ============================================================================
// Tests for migrations
// ============================================================================

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    /// Helper to create an in-memory database and run migrations.
    async fn setup_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        // Enable foreign keys (required for CASCADE)
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();

        // Create migration tracking table
        sqlx::query("CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();

        // Run all migrations
        for (i, migration) in super::MIGRATIONS.iter().enumerate() {
            let version = i + 1;
            // Multi-statement migrations (with BEGIN/COMMIT) use raw_sql()
            let is_multi = migration.contains("BEGIN;") || migration.contains("BEGIN\n");
            let result = if is_multi {
                sqlx::raw_sql(migration).execute(&pool).await.map(|_| ())
            } else {
                sqlx::query(migration).execute(&pool).await.map(|_| ())
            };
            match result {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column name") => {}
                Err(e) => panic!("Migration {} failed: {}", version, e),
            }
            sqlx::query("INSERT OR IGNORE INTO _migrations (version) VALUES (?)")
                .bind(version as i64)
                .execute(&pool)
                .await
                .unwrap();
        }

        pool
    }

    #[tokio::test]
    async fn test_migration8_sessions_new_columns_exist() {
        let pool = setup_db().await;

        // Query the sessions table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        // Verify all new Phase 3 columns exist
        assert!(column_names.contains(&"user_prompt_count"), "Missing user_prompt_count column");
        assert!(column_names.contains(&"api_call_count"), "Missing api_call_count column");
        assert!(column_names.contains(&"tool_call_count"), "Missing tool_call_count column");
        assert!(column_names.contains(&"files_read"), "Missing files_read column");
        assert!(column_names.contains(&"files_edited"), "Missing files_edited column");
        assert!(column_names.contains(&"files_read_count"), "Missing files_read_count column");
        assert!(column_names.contains(&"files_edited_count"), "Missing files_edited_count column");
        assert!(column_names.contains(&"reedited_files_count"), "Missing reedited_files_count column");
        assert!(column_names.contains(&"duration_seconds"), "Missing duration_seconds column");
        assert!(column_names.contains(&"commit_count"), "Missing commit_count column");
    }

    #[tokio::test]
    async fn test_migration8_commits_table_exists() {
        let pool = setup_db().await;

        // Query commits table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('commits')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"hash"), "Missing hash column");
        assert!(column_names.contains(&"repo_path"), "Missing repo_path column");
        assert!(column_names.contains(&"message"), "Missing message column");
        assert!(column_names.contains(&"author"), "Missing author column");
        assert!(column_names.contains(&"timestamp"), "Missing timestamp column");
        assert!(column_names.contains(&"branch"), "Missing branch column");
        assert!(column_names.contains(&"created_at"), "Missing created_at column");
    }

    #[tokio::test]
    async fn test_migration8_session_commits_table_exists() {
        let pool = setup_db().await;

        // Query session_commits table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('session_commits')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"session_id"), "Missing session_id column");
        assert!(column_names.contains(&"commit_hash"), "Missing commit_hash column");
        assert!(column_names.contains(&"tier"), "Missing tier column");
        assert!(column_names.contains(&"evidence"), "Missing evidence column");
        assert!(column_names.contains(&"created_at"), "Missing created_at column");
    }

    #[tokio::test]
    async fn test_migration8_index_metadata_table_exists() {
        let pool = setup_db().await;

        // Query index_metadata table schema
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('index_metadata')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"last_indexed_at"), "Missing last_indexed_at column");
        assert!(column_names.contains(&"last_index_duration_ms"), "Missing last_index_duration_ms column");
        assert!(column_names.contains(&"sessions_indexed"), "Missing sessions_indexed column");
        assert!(column_names.contains(&"projects_indexed"), "Missing projects_indexed column");
        assert!(column_names.contains(&"last_git_sync_at"), "Missing last_git_sync_at column");
        assert!(column_names.contains(&"commits_found"), "Missing commits_found column");
        assert!(column_names.contains(&"links_created"), "Missing links_created column");
        assert!(column_names.contains(&"updated_at"), "Missing updated_at column");

        // Verify singleton row was inserted
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM index_metadata")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1, "index_metadata should have exactly 1 row");
    }

    #[tokio::test]
    async fn test_migration8_indexes_created() {
        let pool = setup_db().await;

        // Query all index names
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        // Verify new Phase 3 indexes
        assert!(index_names.contains(&"idx_commits_repo_ts"), "Missing idx_commits_repo_ts index");
        assert!(index_names.contains(&"idx_commits_timestamp"), "Missing idx_commits_timestamp index");
        assert!(index_names.contains(&"idx_session_commits_session"), "Missing idx_session_commits_session index");
        assert!(index_names.contains(&"idx_session_commits_commit"), "Missing idx_session_commits_commit index");
        assert!(index_names.contains(&"idx_sessions_commit_count"), "Missing idx_sessions_commit_count index");
        assert!(index_names.contains(&"idx_sessions_reedit"), "Missing idx_sessions_reedit index");
        assert!(index_names.contains(&"idx_sessions_duration"), "Missing idx_sessions_duration index");
    }

    #[tokio::test]
    async fn test_migration12_unused_indexes_dropped() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        // These unused indexes should be dropped by migration 12
        assert!(!index_names.contains(&"idx_invocations_session"),
            "idx_invocations_session should be dropped (unused)");
        assert!(!index_names.contains(&"idx_invocations_timestamp"),
            "idx_invocations_timestamp should be dropped (unused)");

        // These used indexes must still exist
        assert!(index_names.contains(&"idx_invocations_invocable"),
            "idx_invocations_invocable must still exist (used by skills dashboard)");
        assert!(index_names.contains(&"idx_turns_session"),
            "idx_turns_session must still exist (used by session listing)");
        assert!(index_names.contains(&"idx_turns_model"),
            "idx_turns_model must still exist (used by models API)");
    }

    // ========================================================================
    // Migration 13 tests (Theme 4: Classification + Index Runs)
    // ========================================================================

    #[tokio::test]
    async fn test_migration13_classification_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"category_l1"), "Missing category_l1 column");
        assert!(column_names.contains(&"category_l2"), "Missing category_l2 column");
        assert!(column_names.contains(&"category_l3"), "Missing category_l3 column");
        assert!(column_names.contains(&"category_confidence"), "Missing category_confidence column");
        assert!(column_names.contains(&"category_source"), "Missing category_source column");
        assert!(column_names.contains(&"classified_at"), "Missing classified_at column");
        assert!(column_names.contains(&"prompt_word_count"), "Missing prompt_word_count column");
        assert!(column_names.contains(&"correction_count"), "Missing correction_count column");
        assert!(column_names.contains(&"same_file_edit_count"), "Missing same_file_edit_count column");
    }

    #[tokio::test]
    async fn test_migration13_classification_jobs_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('classification_jobs')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"started_at"), "Missing started_at column");
        assert!(column_names.contains(&"completed_at"), "Missing completed_at column");
        assert!(column_names.contains(&"total_sessions"), "Missing total_sessions column");
        assert!(column_names.contains(&"classified_count"), "Missing classified_count column");
        assert!(column_names.contains(&"skipped_count"), "Missing skipped_count column");
        assert!(column_names.contains(&"failed_count"), "Missing failed_count column");
        assert!(column_names.contains(&"provider"), "Missing provider column");
        assert!(column_names.contains(&"model"), "Missing model column");
        assert!(column_names.contains(&"status"), "Missing status column");
        assert!(column_names.contains(&"error_message"), "Missing error_message column");
        assert!(column_names.contains(&"cost_estimate_cents"), "Missing cost_estimate_cents column");
        assert!(column_names.contains(&"actual_cost_cents"), "Missing actual_cost_cents column");
        assert!(column_names.contains(&"tokens_used"), "Missing tokens_used column");
    }

    #[tokio::test]
    async fn test_migration13_index_runs_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('index_runs')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"started_at"), "Missing started_at column");
        assert!(column_names.contains(&"completed_at"), "Missing completed_at column");
        assert!(column_names.contains(&"type"), "Missing type column");
        assert!(column_names.contains(&"sessions_before"), "Missing sessions_before column");
        assert!(column_names.contains(&"sessions_after"), "Missing sessions_after column");
        assert!(column_names.contains(&"duration_ms"), "Missing duration_ms column");
        assert!(column_names.contains(&"throughput_mb_per_sec"), "Missing throughput_mb_per_sec column");
        assert!(column_names.contains(&"status"), "Missing status column");
        assert!(column_names.contains(&"error_message"), "Missing error_message column");
    }

    #[tokio::test]
    async fn test_migration13_classification_jobs_check_constraints() {
        let pool = setup_db().await;

        // Valid status should work
        let result = sqlx::query(
            "INSERT INTO classification_jobs (started_at, total_sessions, provider, model, status) VALUES ('2026-02-05T12:00:00Z', 100, 'claude-cli', 'haiku', 'running')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "Valid status 'running' should be accepted");

        // Invalid status should fail
        let result = sqlx::query(
            "INSERT INTO classification_jobs (started_at, total_sessions, provider, model, status) VALUES ('2026-02-05T12:00:00Z', 100, 'claude-cli', 'haiku', 'invalid')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid status should be rejected by CHECK constraint");
    }

    #[tokio::test]
    async fn test_migration13_index_runs_check_constraints() {
        let pool = setup_db().await;

        // Valid type and status should work
        let result = sqlx::query(
            "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'running')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "Valid type 'full' and status 'running' should be accepted");

        // Invalid type should fail
        let result = sqlx::query(
            "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'invalid', 'running')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid type should be rejected by CHECK constraint");

        // Invalid status should fail
        let result = sqlx::query(
            "INSERT INTO index_runs (started_at, type, status) VALUES ('2026-02-05T12:00:00Z', 'full', 'invalid')"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid status should be rejected by CHECK constraint");
    }

    #[tokio::test]
    async fn test_migration13_indexes_created() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        assert!(index_names.contains(&"idx_sessions_category_l1"), "Missing idx_sessions_category_l1 index");
        assert!(index_names.contains(&"idx_sessions_classified"), "Missing idx_sessions_classified index");
        assert!(index_names.contains(&"idx_classification_jobs_status"), "Missing idx_classification_jobs_status index");
        assert!(index_names.contains(&"idx_classification_jobs_started"), "Missing idx_classification_jobs_started index");
        assert!(index_names.contains(&"idx_index_runs_started"), "Missing idx_index_runs_started index");
    }

    #[tokio::test]
    async fn test_migration8_check_constraints_work() {
        let pool = setup_db().await;

        // Insert a valid session first (required for FK in session_commits)
        sqlx::query(
            r#"
            INSERT INTO sessions (id, project_id, file_path, preview)
            VALUES ('test-sess', 'test-proj', '/tmp/test.jsonl', 'Test')
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        // Test that negative values are rejected for user_prompt_count
        let result = sqlx::query(
            "UPDATE sessions SET user_prompt_count = -1 WHERE id = 'test-sess'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative user_prompt_count should be rejected");

        // Test that negative values are rejected for duration_seconds
        let result = sqlx::query(
            "UPDATE sessions SET duration_seconds = -1 WHERE id = 'test-sess'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative duration_seconds should be rejected");

        // Test that valid tier values work (1 and 2)
        sqlx::query(
            "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('abc123', '/repo', 'test', 1000)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = sqlx::query(
            "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('test-sess', 'abc123', 1)"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "tier=1 should be valid");

        // Test that invalid tier value is rejected
        let result = sqlx::query(
            "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('test-sess', 'abc123', 3)"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "tier=3 should be rejected (only 1 or 2 allowed)");

        // Test index_metadata singleton constraint
        let result = sqlx::query(
            "INSERT INTO index_metadata (id) VALUES (2)"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "index_metadata should only allow id=1");
    }

    #[tokio::test]
    async fn test_migration8_default_values() {
        let pool = setup_db().await;

        // Insert a minimal session
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('test-sess', 'proj', '/tmp/t.jsonl', 'Preview')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Verify default values
        let row: (i64, i64, i64, String, String, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT user_prompt_count, api_call_count, tool_call_count,
                   files_read, files_edited, files_read_count, files_edited_count,
                   reedited_files_count, duration_seconds, commit_count
            FROM sessions WHERE id = 'test-sess'
            "#
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, 0, "user_prompt_count default should be 0");
        assert_eq!(row.1, 0, "api_call_count default should be 0");
        assert_eq!(row.2, 0, "tool_call_count default should be 0");
        assert_eq!(row.3, "[]", "files_read default should be '[]'");
        assert_eq!(row.4, "[]", "files_edited default should be '[]'");
        assert_eq!(row.5, 0, "files_read_count default should be 0");
        assert_eq!(row.6, 0, "files_edited_count default should be 0");
        assert_eq!(row.7, 0, "reedited_files_count default should be 0");
        assert_eq!(row.8, 0, "duration_seconds default should be 0");
        assert_eq!(row.9, 0, "commit_count default should be 0");
    }

    #[tokio::test]
    async fn test_migration9_full_parser_columns_exist() {
        let pool = setup_db().await;
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(names.contains(&"parse_version"), "Missing parse_version");
        assert!(names.contains(&"turn_duration_avg_ms"), "Missing turn_duration_avg_ms");
        assert!(names.contains(&"turn_duration_max_ms"), "Missing turn_duration_max_ms");
        assert!(names.contains(&"turn_duration_total_ms"), "Missing turn_duration_total_ms");
        assert!(names.contains(&"api_error_count"), "Missing api_error_count");
        assert!(names.contains(&"api_retry_count"), "Missing api_retry_count");
        assert!(names.contains(&"compaction_count"), "Missing compaction_count");
        assert!(names.contains(&"hook_blocked_count"), "Missing hook_blocked_count");
        assert!(names.contains(&"agent_spawn_count"), "Missing agent_spawn_count");
        assert!(names.contains(&"bash_progress_count"), "Missing bash_progress_count");
        assert!(names.contains(&"hook_progress_count"), "Missing hook_progress_count");
        assert!(names.contains(&"mcp_progress_count"), "Missing mcp_progress_count");
        assert!(names.contains(&"summary_text"), "Missing summary_text");
        assert!(names.contains(&"total_input_tokens"), "Missing total_input_tokens");
        assert!(names.contains(&"total_output_tokens"), "Missing total_output_tokens");
        assert!(names.contains(&"cache_read_tokens"), "Missing cache_read_tokens");
        assert!(names.contains(&"cache_creation_tokens"), "Missing cache_creation_tokens");
        assert!(names.contains(&"thinking_block_count"), "Missing thinking_block_count");
    }

    #[tokio::test]
    async fn test_migration9_detail_tables_exist() {
        let pool = setup_db().await;

        // Verify turn_metrics table
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('turn_metrics')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        assert!(names.contains(&"session_id"));
        assert!(names.contains(&"turn_seq"));
        assert!(names.contains(&"duration_ms"));
        assert!(names.contains(&"input_tokens"));
        assert!(names.contains(&"model"));

        // Verify api_errors table
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('api_errors')"
        ).fetch_all(&pool).await.unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        assert!(names.contains(&"session_id"));
        assert!(names.contains(&"timestamp_unix"));
        assert!(names.contains(&"retry_attempt"));
    }

    #[tokio::test]
    async fn test_migration9_parse_version_default() {
        let pool = setup_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('pv-test', 'proj', '/tmp/pv.jsonl', 'Test')"
        ).execute(&pool).await.unwrap();

        let row: (i64,) = sqlx::query_as(
            "SELECT parse_version FROM sessions WHERE id = 'pv-test'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 0, "parse_version default should be 0");
    }

    #[tokio::test]
    async fn test_migration8_cascade_delete() {
        let pool = setup_db().await;

        // Insert session and commit
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('sess-1', 'proj', '/tmp/s.jsonl', 'Test')"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('hash1', '/repo', 'msg', 1000)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO session_commits (session_id, commit_hash, tier) VALUES ('sess-1', 'hash1', 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Verify link exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);

        // Delete session - should cascade to session_commits
        sqlx::query("DELETE FROM sessions WHERE id = 'sess-1'")
            .execute(&pool)
            .await
            .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM session_commits")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "session_commits should be deleted via CASCADE");
    }

    // ========================================================================
    // Migration 13: LOC estimation columns (from main)
    // ========================================================================

    #[tokio::test]
    async fn test_migration13_loc_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"lines_added"), "Missing lines_added column");
        assert!(column_names.contains(&"lines_removed"), "Missing lines_removed column");
        assert!(column_names.contains(&"loc_source"), "Missing loc_source column");
    }

    #[tokio::test]
    async fn test_migration13_loc_defaults() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('loc-test', 'proj', '/tmp/loc.jsonl', 'Test')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'loc-test'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, 0, "lines_added default should be 0");
        assert_eq!(row.1, 0, "lines_removed default should be 0");
        assert_eq!(row.2, 0, "loc_source default should be 0");
    }

    #[tokio::test]
    async fn test_migration13_loc_check_constraints() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('loc-check', 'proj', '/tmp/c.jsonl', 'Test')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Test that negative values are rejected for lines_added
        let result = sqlx::query(
            "UPDATE sessions SET lines_added = -1 WHERE id = 'loc-check'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative lines_added should be rejected");

        // Test that negative values are rejected for lines_removed
        let result = sqlx::query(
            "UPDATE sessions SET lines_removed = -1 WHERE id = 'loc-check'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Negative lines_removed should be rejected");

        // Test that valid loc_source values work (0, 1, 2)
        let result = sqlx::query(
            "UPDATE sessions SET loc_source = 1 WHERE id = 'loc-check'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "loc_source=1 should be valid");

        let result = sqlx::query(
            "UPDATE sessions SET loc_source = 2 WHERE id = 'loc-check'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "loc_source=2 should be valid");

        // Test that invalid loc_source value is rejected
        let result = sqlx::query(
            "UPDATE sessions SET loc_source = 3 WHERE id = 'loc-check'"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "loc_source=3 should be rejected (only 0, 1, 2 allowed)");
    }

    // ========================================================================
    // Migration 14: Theme 3 contribution tracking
    // ========================================================================

    #[tokio::test]
    async fn test_migration14_sessions_contribution_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"ai_lines_added"), "Missing ai_lines_added column");
        assert!(column_names.contains(&"ai_lines_removed"), "Missing ai_lines_removed column");
        assert!(column_names.contains(&"work_type"), "Missing work_type column");
    }

    #[tokio::test]
    async fn test_migration14_commits_diff_stats_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('commits')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"files_changed"), "Missing files_changed column");
        assert!(column_names.contains(&"insertions"), "Missing insertions column");
        assert!(column_names.contains(&"deletions"), "Missing deletions column");
    }

    #[tokio::test]
    async fn test_migration14_contribution_snapshots_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('contribution_snapshots')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"date"), "Missing date column");
        assert!(column_names.contains(&"project_id"), "Missing project_id column");
        assert!(column_names.contains(&"branch"), "Missing branch column");
        assert!(column_names.contains(&"sessions_count"), "Missing sessions_count column");
        assert!(column_names.contains(&"ai_lines_added"), "Missing ai_lines_added column");
        assert!(column_names.contains(&"ai_lines_removed"), "Missing ai_lines_removed column");
        assert!(column_names.contains(&"commits_count"), "Missing commits_count column");
        assert!(column_names.contains(&"commit_insertions"), "Missing commit_insertions column");
        assert!(column_names.contains(&"commit_deletions"), "Missing commit_deletions column");
        assert!(column_names.contains(&"tokens_used"), "Missing tokens_used column");
        assert!(column_names.contains(&"cost_cents"), "Missing cost_cents column");
    }

    #[tokio::test]
    async fn test_migration14_contribution_snapshots_unique_constraint() {
        let pool = setup_db().await;

        // Insert first snapshot
        sqlx::query(
            "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-05', 'proj1', 'main', 5)"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert second snapshot - same date+project+branch should fail
        let result = sqlx::query(
            "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-05', 'proj1', 'main', 10)"
        )
        .execute(&pool)
        .await;

        assert!(result.is_err(), "Should reject duplicate date+project_id+branch combination");

        // Insert different date - should succeed
        let result = sqlx::query(
            "INSERT INTO contribution_snapshots (date, project_id, branch, sessions_count) VALUES ('2026-02-06', 'proj1', 'main', 10)"
        )
        .execute(&pool)
        .await;

        assert!(result.is_ok(), "Should allow different date");
    }

    #[tokio::test]
    async fn test_migration14_sessions_default_values() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('contrib-test', 'proj', '/tmp/c.jsonl', 'Test')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row: (i64, i64, Option<String>) = sqlx::query_as(
            "SELECT ai_lines_added, ai_lines_removed, work_type FROM sessions WHERE id = 'contrib-test'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, 0, "ai_lines_added default should be 0");
        assert_eq!(row.1, 0, "ai_lines_removed default should be 0");
        assert!(row.2.is_none(), "work_type default should be NULL");
    }

    #[tokio::test]
    async fn test_migration14_commits_default_values() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO commits (hash, repo_path, message, timestamp) VALUES ('abc123def456789012345678901234567890abcd', '/repo', 'test', 1000)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row: (i64, i64, i64) = sqlx::query_as(
            "SELECT files_changed, insertions, deletions FROM commits WHERE hash = 'abc123def456789012345678901234567890abcd'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, 0, "files_changed default should be 0");
        assert_eq!(row.1, 0, "insertions default should be 0");
        assert_eq!(row.2, 0, "deletions default should be 0");
    }

    #[tokio::test]
    async fn test_migration14_indexes_created() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_snapshots%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        assert!(index_names.contains(&"idx_snapshots_date"), "Missing idx_snapshots_date index");
        assert!(index_names.contains(&"idx_snapshots_project_date"), "Missing idx_snapshots_project_date index");
        assert!(index_names.contains(&"idx_snapshots_branch_date"), "Missing idx_snapshots_branch_date index");
    }

    // ========================================================================
    // Migration 16: Dashboard analytics indexes (renumbered from branch's 13)
    // ========================================================================

    #[tokio::test]
    async fn test_migration16_dashboard_analytics_indexes() {
        let pool = setup_db().await;

        // Verify primary_model column was added
        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        assert!(column_names.contains(&"primary_model"), "Missing primary_model column");

        // Verify new indexes were created
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        assert!(index_names.contains(&"idx_sessions_first_message"),
            "Missing idx_sessions_first_message index");
        assert!(index_names.contains(&"idx_sessions_project_first_message"),
            "Missing idx_sessions_project_first_message index");
        assert!(index_names.contains(&"idx_sessions_primary_model"),
            "Missing idx_sessions_primary_model index");
    }

    #[tokio::test]
    async fn test_migration16_primary_model_can_be_set() {
        let pool = setup_db().await;

        // Insert a session with primary_model
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, primary_model) VALUES ('pm-test', 'proj', '/tmp/pm.jsonl', 'Test', 'claude-sonnet-4')"
        )
        .execute(&pool)
        .await
        .unwrap();

        // Verify the value
        let row: (Option<String>,) = sqlx::query_as(
            "SELECT primary_model FROM sessions WHERE id = 'pm-test'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0, Some("claude-sonnet-4".to_string()));
    }

    // ========================================================================
    // Migration 18: Drop file_hash, verify schema cleanup
    // ========================================================================

    #[tokio::test]
    async fn test_migration18_file_hash_column_dropped() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        // file_hash should be gone
        assert!(!column_names.contains(&"file_hash"),
            "file_hash column should be dropped by migration 18");

        // All other essential columns should still exist
        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"project_id"), "Missing project_id column");
        assert!(column_names.contains(&"file_path"), "Missing file_path column");
        assert!(column_names.contains(&"summary"), "Missing summary column");
        assert!(column_names.contains(&"summary_text"), "Missing summary_text column");
        assert!(column_names.contains(&"primary_model"), "Missing primary_model column");
        assert!(column_names.contains(&"work_type"), "Missing work_type column");
    }

    #[tokio::test]
    async fn test_migration18_indexes_preserved() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_sessions%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();

        // All session indexes should be recreated
        assert!(index_names.contains(&"idx_sessions_project"), "Missing idx_sessions_project");
        assert!(index_names.contains(&"idx_sessions_last_message"), "Missing idx_sessions_last_message");
        assert!(index_names.contains(&"idx_sessions_project_branch"), "Missing idx_sessions_project_branch");
        assert!(index_names.contains(&"idx_sessions_sidechain"), "Missing idx_sessions_sidechain");
        assert!(index_names.contains(&"idx_sessions_commit_count"), "Missing idx_sessions_commit_count");
        assert!(index_names.contains(&"idx_sessions_reedit"), "Missing idx_sessions_reedit");
        assert!(index_names.contains(&"idx_sessions_duration"), "Missing idx_sessions_duration");
        assert!(index_names.contains(&"idx_sessions_needs_reindex"), "Missing idx_sessions_needs_reindex");
        assert!(index_names.contains(&"idx_sessions_first_message"), "Missing idx_sessions_first_message");
        assert!(index_names.contains(&"idx_sessions_project_first_message"), "Missing idx_sessions_project_first_message");
        assert!(index_names.contains(&"idx_sessions_primary_model"), "Missing idx_sessions_primary_model");
    }

    #[tokio::test]
    async fn test_migration18_data_preserved() {
        let pool = setup_db().await;

        // Insert a session before migration runs (it already ran via setup_db)
        // Instead, insert and verify data round-trips correctly
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, summary, summary_text, primary_model) VALUES ('m18-test', 'proj', '/tmp/m18.jsonl', 'Test', 'index summary', 'deep summary', 'claude-sonnet-4')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT summary, summary_text, primary_model FROM sessions WHERE id = 'm18-test'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.0.as_deref(), Some("index summary"));
        assert_eq!(row.1.as_deref(), Some("deep summary"));
        assert_eq!(row.2.as_deref(), Some("claude-sonnet-4"));
    }

    #[tokio::test]
    async fn test_migration18_coalesce_summary_behavior() {
        let pool = setup_db().await;

        // Session with both summaries: summary_text wins
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, summary, summary_text) VALUES ('coal-1', 'proj', '/tmp/c1.jsonl', 'Test', 'from index', 'from deep')"
        ).execute(&pool).await.unwrap();

        // Session with only index summary: summary as fallback
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview, summary) VALUES ('coal-2', 'proj', '/tmp/c2.jsonl', 'Test', 'from index only')"
        ).execute(&pool).await.unwrap();

        // Session with neither
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('coal-3', 'proj', '/tmp/c3.jsonl', 'Test')"
        ).execute(&pool).await.unwrap();

        let row: (Option<String>,) = sqlx::query_as(
            "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-1'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0.as_deref(), Some("from deep"), "summary_text should win when both present");

        let row: (Option<String>,) = sqlx::query_as(
            "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-2'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0.as_deref(), Some("from index only"), "summary should be fallback");

        let row: (Option<String>,) = sqlx::query_as(
            "SELECT COALESCE(summary_text, summary) AS summary FROM sessions WHERE id = 'coal-3'"
        ).fetch_one(&pool).await.unwrap();
        assert!(row.0.is_none(), "Both NULL should yield NULL");
    }

    #[tokio::test]
    async fn test_migration_task_time_columns_exist() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('sessions')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"total_task_time_seconds"), "Missing total_task_time_seconds column");
        assert!(column_names.contains(&"longest_task_seconds"), "Missing longest_task_seconds column");
        assert!(column_names.contains(&"longest_task_preview"), "Missing longest_task_preview column");
    }

    #[tokio::test]
    async fn test_migration_task_time_defaults() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('task-time-test', 'proj', '/tmp/tt.jsonl', 'Test')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let row: (Option<i64>, Option<i64>, Option<String>) = sqlx::query_as(
            "SELECT total_task_time_seconds, longest_task_seconds, longest_task_preview FROM sessions WHERE id = 'task-time-test'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert!(row.0.is_none(), "total_task_time_seconds default should be NULL");
        assert!(row.1.is_none(), "longest_task_seconds default should be NULL");
        assert!(row.2.is_none(), "longest_task_preview default should be NULL");
    }

    #[tokio::test]
    async fn test_migration18_check_constraints_preserved() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, preview) VALUES ('m18-chk', 'proj', '/tmp/chk.jsonl', 'Test')"
        ).execute(&pool).await.unwrap();

        // Verify CHECK constraints survived the table recreation
        let result = sqlx::query(
            "UPDATE sessions SET lines_added = -1 WHERE id = 'm18-chk'"
        ).execute(&pool).await;
        assert!(result.is_err(), "CHECK constraint on lines_added should survive migration 18");

        let result = sqlx::query(
            "UPDATE sessions SET loc_source = 3 WHERE id = 'm18-chk'"
        ).execute(&pool).await;
        assert!(result.is_err(), "CHECK constraint on loc_source should survive migration 18");
    }

    // ========================================================================
    // Migration 25: Work Reports table
    // ========================================================================

    #[tokio::test]
    async fn test_migration25_reports_table_exists() {
        let pool = setup_db().await;

        let columns: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM pragma_table_info('reports')"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let column_names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();

        assert!(column_names.contains(&"id"), "Missing id column");
        assert!(column_names.contains(&"report_type"), "Missing report_type column");
        assert!(column_names.contains(&"date_start"), "Missing date_start column");
        assert!(column_names.contains(&"date_end"), "Missing date_end column");
        assert!(column_names.contains(&"content_md"), "Missing content_md column");
        assert!(column_names.contains(&"context_digest"), "Missing context_digest column");
        assert!(column_names.contains(&"session_count"), "Missing session_count column");
        assert!(column_names.contains(&"project_count"), "Missing project_count column");
        assert!(column_names.contains(&"total_duration_secs"), "Missing total_duration_secs column");
        assert!(column_names.contains(&"total_cost_cents"), "Missing total_cost_cents column");
        assert!(column_names.contains(&"generation_ms"), "Missing generation_ms column");
        assert!(column_names.contains(&"created_at"), "Missing created_at column");
    }

    #[tokio::test]
    async fn test_migration25_reports_check_constraints() {
        let pool = setup_db().await;

        // Valid report_type should work
        let result = sqlx::query(
            "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('daily', '2026-02-21', '2026-02-21', '- Shipped search', 8, 3, 15120, 680)"
        )
        .execute(&pool)
        .await;
        assert!(result.is_ok(), "Valid report_type 'daily' should be accepted");

        // Invalid report_type should fail
        let result = sqlx::query(
            "INSERT INTO reports (report_type, date_start, date_end, content_md, session_count, project_count, total_duration_secs, total_cost_cents) VALUES ('invalid', '2026-02-21', '2026-02-21', 'test', 0, 0, 0, 0)"
        )
        .execute(&pool)
        .await;
        assert!(result.is_err(), "Invalid report_type should be rejected");
    }

    #[tokio::test]
    async fn test_migration25_reports_indexes() {
        let pool = setup_db().await;

        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_reports%'"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = indexes.iter().map(|(n,)| n.as_str()).collect();
        assert!(index_names.contains(&"idx_reports_date"), "Missing idx_reports_date index");
        assert!(index_names.contains(&"idx_reports_type"), "Missing idx_reports_type index");
    }
}
