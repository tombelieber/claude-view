//! Schema bootstrap migrations 1–20.
//!
//! Defines the original `sessions` / `indexer_state` / `invocables` /
//! `turns` / `models` / `commits` / `session_commits` / `index_metadata`
//! / `contribution_snapshots` / `classification_jobs` / `index_runs`
//! tables, the dashboard analytics columns, the file-hash drop
//! (Migration 18 — table-rebuild), and the Theme-4 classification
//! foundation.
//!
//! Migration ordering is load-bearing. Append-only — never insert in the
//! middle and never reorder. Migrations 21+ live in `indexer.rs`.

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
    r#"ALTER TABLE sessions ADD COLUMN work_type TEXT;"#, // 'deep_work', 'quick_ask', 'planning', 'bug_fix', 'standard'
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
];
