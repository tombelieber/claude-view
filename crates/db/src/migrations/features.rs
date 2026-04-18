//! Feature-and-cleanup migrations 49–63.
//!
//! Covers the classification-jobs estimate-field drop (49), session
//! archival (50), hook-event source/dedup (51–53), plan slug (54),
//! live-session timestamps (55–56), model-catalog metadata + SDK-support
//! flag (57–58), session entrypoint (59), the project-dir status cache
//! and its later removal (60, 62), the git_root re-resolve fix (61),
//! and the IRREVERSIBLE CQRS Phase 0 Step 5 drops (63).
//!
//! Migration ordering is load-bearing. Append-only — never insert in the
//! middle and never reorder.

pub const MIGRATIONS: &[&str] = &[
    // Migration 49: Remove deprecated pre-run estimate field from classification_jobs.
    // Data-preserving table rebuild to stay compatible across SQLite versions.
    r#"BEGIN;
CREATE TABLE classification_jobs_v2 (
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
    actual_cost_cents INTEGER,
    tokens_used INTEGER,
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
);
INSERT INTO classification_jobs_v2 (
    id,
    started_at,
    completed_at,
    total_sessions,
    classified_count,
    skipped_count,
    failed_count,
    provider,
    model,
    status,
    error_message,
    actual_cost_cents,
    tokens_used
)
SELECT
    id,
    started_at,
    completed_at,
    total_sessions,
    classified_count,
    skipped_count,
    failed_count,
    provider,
    model,
    status,
    error_message,
    actual_cost_cents,
    tokens_used
FROM classification_jobs;
DROP TABLE classification_jobs;
ALTER TABLE classification_jobs_v2 RENAME TO classification_jobs;
CREATE INDEX IF NOT EXISTS idx_classification_jobs_status ON classification_jobs(status);
CREATE INDEX IF NOT EXISTS idx_classification_jobs_started ON classification_jobs(started_at DESC);
COMMIT;"#,
    // Migration 50: Add archived_at column for session archiving
    r#"BEGIN;
ALTER TABLE sessions ADD COLUMN archived_at TEXT;
DROP VIEW IF EXISTS valid_sessions;
CREATE VIEW valid_sessions AS SELECT * FROM sessions WHERE is_sidechain = 0 AND archived_at IS NULL;
COMMIT;"#,
    // Migration 51: Add source column to hook_events.
    // Existing rows get 'hook' (they came from Channel B HTTP POST).
    r#"ALTER TABLE hook_events ADD COLUMN source TEXT NOT NULL DEFAULT 'hook';"#,
    // Migration 52: Dedup existing hook_events before adding unique constraint.
    r#"DELETE FROM hook_events WHERE id NOT IN (
        SELECT MIN(id) FROM hook_events
        GROUP BY session_id, timestamp, event_name, COALESCE(tool_name, ''), source
    );"#,
    // Migration 53: Unique constraint for self-dedup within each source.
    r#"CREATE UNIQUE INDEX IF NOT EXISTS idx_hook_events_dedup ON hook_events(session_id, timestamp, event_name, COALESCE(tool_name, ''), source);"#,
    // Migration 54: Add slug column for plan file association
    r#"ALTER TABLE sessions ADD COLUMN slug TEXT;"#,
    // Migration 55: closed_at — set when a live session's process exits (NULL = never tracked as live).
    r#"ALTER TABLE sessions ADD COLUMN closed_at INTEGER;"#,
    // Migration 56: dismissed_at — set when user explicitly dismisses from recently-closed list.
    r#"ALTER TABLE sessions ADD COLUMN dismissed_at INTEGER;"#,
    // Migration 57: model catalog columns — display metadata + context window.
    r#"BEGIN;
ALTER TABLE models ADD COLUMN display_name TEXT;
ALTER TABLE models ADD COLUMN description TEXT;
ALTER TABLE models ADD COLUMN max_input_tokens INTEGER;
ALTER TABLE models ADD COLUMN max_output_tokens INTEGER;
ALTER TABLE models ADD COLUMN updated_at INTEGER DEFAULT 0;
COMMIT;"#,
    // Migration 58: sdk_supported flag — true only for models reported by Agent SDK.
    // Chat model selector filters on this to show only usable models.
    // Default 0 — only set to 1 when SDK actually reports via upsert_sdk_models().
    r#"ALTER TABLE models ADD COLUMN sdk_supported INTEGER NOT NULL DEFAULT 0;"#,
    // Migration 59: entrypoint — how the session was launched (cli, claude-vscode, sdk-ts).
    // Extracted from the JSONL "entrypoint" field on the first line that has it.
    r#"ALTER TABLE sessions ADD COLUMN entrypoint TEXT;"#,
    // Migration 60: project_dir_status — caches directory existence check per effective project path.
    // Populated by backfill at startup. Used by list_project_summaries() to flag archived projects.
    r#"CREATE TABLE IF NOT EXISTS project_dir_status (
        effective_path TEXT PRIMARY KEY,
        dir_exists INTEGER NOT NULL DEFAULT 1,
        checked_at INTEGER NOT NULL
    );"#,
    // Migration 61: Clear incorrect git_root for sessions where infer_git_root_from_worktree_path
    // returned a subdirectory instead of the real git root. Backfill will re-resolve with fixed logic.
    // Matches any git_root ending in /private/config (the submodule subdirectory pattern).
    r#"UPDATE sessions SET git_root = NULL WHERE git_root LIKE '%/private/config';"#,
    // Migration 62: Drop project_dir_status — orphaned after the JSONL-first
    // hardcut. /api/projects now derives `is_archived` by stat()'ing the
    // project dir at request time (see routes/projects.rs::project_dir_exists),
    // so the table + its backfill are dead weight.
    r#"DROP TABLE IF EXISTS project_dir_status;"#,
    // Migration 63: CQRS Phase 0 Step 5 — IRREVERSIBLE drops.
    //
    // Drops 4 tables and 8 columns that were confirmed dead by the Wave 1/2
    // audits (plan §1.1, §10 Phase 0):
    //   - 4 tables with zero writers AND zero readers: turn_metrics,
    //     api_errors, fluency_scores, pricing_cache.
    //   - 7 sessions columns with zero writers AND zero readers: closed_at,
    //     dismissed_at, session_kind, start_type, prompt_word_count,
    //     correction_count, same_file_edit_count.
    //   - 1 models column with zero live readers: sdk_supported.
    //
    // Important: `closed_at` and `dismissed_at` remain as IN-MEMORY fields on
    // `ActiveSession` (server/live/manager/reaper.rs and server-live-state).
    // Only the DB columns are dropped.
    //
    // Rollback: restore from `~/.claude-view/claude-view.db.pre-phase0` +
    // `git checkout pre-phase0`. No forward rollback migration exists.
    r#"BEGIN;
DROP TABLE IF EXISTS turn_metrics;
DROP TABLE IF EXISTS api_errors;
DROP TABLE IF EXISTS fluency_scores;
DROP TABLE IF EXISTS pricing_cache;
ALTER TABLE sessions DROP COLUMN closed_at;
ALTER TABLE sessions DROP COLUMN dismissed_at;
ALTER TABLE sessions DROP COLUMN session_kind;
ALTER TABLE sessions DROP COLUMN start_type;
ALTER TABLE sessions DROP COLUMN prompt_word_count;
ALTER TABLE sessions DROP COLUMN correction_count;
ALTER TABLE sessions DROP COLUMN same_file_edit_count;
ALTER TABLE models DROP COLUMN sdk_supported;
COMMIT;"#,
    // Migration 64: session_stats — Layer-2 typed read-side mirror of the
    // existing `sessions` row, populated by the Phase 2 indexer_v2 writer
    // (PR 2.2). STRICT mode enforces declared types; no FK to `sessions`
    // because shadow mode tolerates transient inconsistency. Schema mirrors
    // `claude_view_session_parser::SessionStats` — any new SessionStats
    // field requires a matching ALTER in a future migration (writer
    // ownership registry, design §10.2).
    //
    // Field grouping (32 columns total = 8 staleness/header + 24 stats —
    // design §3.1 said "9 header + 25 stats = 34" but miscounted):
    //   header (8)   : session_id PK + content_hash/size/inode/mid_hash
    //                  staleness + parser/stats versions + indexed_at
    //   tokens (6)   : input/output/cache_read/cache_creation + 5m/1hr breakdown
    //   counts (6)   : turns/prompts/lines/tools/thinking/api_errors
    //   tools  (4)   : files_read/files_edited/bash/agent_spawn
    //   times  (3)   : first/last message + duration_seconds (unix seconds)
    //   strings(4)   : primary_model + git_branch + preview + last_message
    //   per-model(1) : per_model_tokens_json (JSON, normalized at write)
    //
    // Indexes target Phase 3 read-cutover queries:
    //   last_ts + indexed_at: list ordering / "what's new"
    //   total_tokens (expr) : sort by token cost — uses an expression index
    //                         on (input + output) since there is no stored
    //                         total_tokens column. Design §3.1 referenced
    //                         a non-existent `total_tokens` column; the
    //                         sum-expression is the minimum-surprise
    //                         substitute that delivers the same ORDER BY.
    //   primary_model       : "tokens by model" filter
    //   git_branch          : per-branch grouping
    r#"BEGIN;
CREATE TABLE session_stats (
    session_id          TEXT PRIMARY KEY,
    source_content_hash BLOB NOT NULL,
    source_size         INTEGER NOT NULL,
    source_inode        INTEGER,
    source_mid_hash     BLOB,
    parser_version      INTEGER NOT NULL,
    stats_version       INTEGER NOT NULL,
    indexed_at          INTEGER NOT NULL,

    -- Token counts (mirror SessionStats token fields).
    total_input_tokens          INTEGER NOT NULL DEFAULT 0,
    total_output_tokens         INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens           INTEGER NOT NULL DEFAULT 0,
    cache_creation_tokens       INTEGER NOT NULL DEFAULT 0,
    cache_creation_5m_tokens    INTEGER NOT NULL DEFAULT 0,
    cache_creation_1hr_tokens   INTEGER NOT NULL DEFAULT 0,

    -- Counts (mirror SessionStats turn / prompt / line / tool / thinking / error).
    turn_count                  INTEGER NOT NULL DEFAULT 0,
    user_prompt_count           INTEGER NOT NULL DEFAULT 0,
    line_count                  INTEGER NOT NULL DEFAULT 0,
    tool_call_count             INTEGER NOT NULL DEFAULT 0,
    thinking_block_count        INTEGER NOT NULL DEFAULT 0,
    api_error_count             INTEGER NOT NULL DEFAULT 0,

    -- Tool breakdown.
    files_read_count            INTEGER NOT NULL DEFAULT 0,
    files_edited_count          INTEGER NOT NULL DEFAULT 0,
    bash_count                  INTEGER NOT NULL DEFAULT 0,
    agent_spawn_count           INTEGER NOT NULL DEFAULT 0,

    -- Timestamps (unix seconds; RFC3339 string normalized at write time).
    first_message_at            INTEGER,
    last_message_at             INTEGER,
    duration_seconds            INTEGER NOT NULL DEFAULT 0,

    -- Model + git + display strings.
    primary_model               TEXT,
    git_branch                  TEXT,
    preview                     TEXT NOT NULL DEFAULT '',
    last_message                TEXT NOT NULL DEFAULT '',

    -- Per-model token breakdown (JSON object {model_id: {input, output, ...}}).
    per_model_tokens_json       TEXT NOT NULL DEFAULT '{}'
) STRICT;
CREATE INDEX idx_session_stats_last_ts ON session_stats(last_message_at DESC);
CREATE INDEX idx_session_stats_indexed_at ON session_stats(indexed_at DESC);
CREATE INDEX idx_session_stats_total_tokens ON session_stats((total_input_tokens + total_output_tokens) DESC);
CREATE INDEX idx_session_stats_primary_model ON session_stats(primary_model);
CREATE INDEX idx_session_stats_git_branch ON session_stats(git_branch);
COMMIT;"#,
    // Migration 65: session_flags — append-only fold target for
    // archive/dismiss/classify mutations. Phase 5 will add the fold writer;
    // Phase 2 lands the table now so shadow-mode rollups have a stable join
    // target. All rows start with applied_seq=0; the Phase 5 writer
    // increments applied_seq INSIDE the fold transaction.
    //
    // Indexes target Phase 5 + Phase 6 reads:
    //   archived: WHERE archived_at IS NOT NULL — partial keeps it sparse.
    //   category: WHERE category_l1 IS NOT NULL — same sparsity argument.
    r#"BEGIN;
CREATE TABLE session_flags (
    session_id            TEXT PRIMARY KEY,
    archived_at           INTEGER,
    dismissed_at          INTEGER,
    category_l1           TEXT,
    category_l2           TEXT,
    category_l3           TEXT,
    category_confidence   REAL,
    category_source       TEXT,
    classified_at         INTEGER,
    applied_seq           INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX idx_session_flags_archived ON session_flags(archived_at) WHERE archived_at IS NOT NULL;
CREATE INDEX idx_session_flags_category ON session_flags(category_l1) WHERE category_l1 IS NOT NULL;
COMMIT;"#,
];
