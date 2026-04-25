//! Event-log migrations — append-only event-sourcing tables.
//!
//! Starts at migration 82 (CQRS Phase 5 PR 5.1). Placed AFTER `rollups`
//! in the canonical apply order so the proc-macro-generated rollup slice
//! keeps its contiguous version range and this module becomes the new
//! trailing append point.
//!
//! Migration ordering is load-bearing. Append-only — never insert in the
//! middle and never reorder. A `_migrations`-tracked production database
//! at version 81 (rollups tail) applies migration 82 only; an empty DB
//! applies every prior migration first, then lands here at v82.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §7.1`
//! for the Phase 5 event-log design. Fold writer + dual-write handlers
//! ship in subsequent Phase 5 PRs (5.2 / 5.3) on top of this schema.

pub const MIGRATIONS: &[&str] = &[
    // Migration 82: session_action_log — append-only event log for
    // session mutations. Every archive / unarchive / classify / dismiss /
    // reclassify mutation lands here as a single immutable row inside
    // the same transaction as the legacy `sessions` column write
    // (PR 5.2 adds dual-write). PR 5.3's fold task consumes this log
    // into `session_flags` and advances `applied_seq` atomically.
    //
    // Column design (per §7.1):
    //   seq         — monotonic sequence. Primary key + AUTOINCREMENT so
    //                 `applied_seq` advances strictly and a fold-task
    //                 restart can resume from `SELECT MAX(applied_seq)`
    //                 without scanning by timestamp (timestamps collide
    //                 at second granularity under burst writes; see
    //                 memory entry `feedback_check_existing_before_adding`).
    //   session_id  — TEXT (matches `sessions.id` / `session_flags.session_id`).
    //                 No FK — §10 of the SOTA plan treats the legacy
    //                 `sessions` table as Phase-6 retirement scope, so a
    //                 hard FK here would block Phase 6. Cross-table
    //                 integrity is enforced by the fold writer + a
    //                 Phase 7 drift detector.
    //   action      — 'archive' | 'unarchive' | 'classify' | 'dismiss' |
    //                 'reclassify'. Plain TEXT (not CHECK-constrained)
    //                 so Phase 5.5+ can introduce new actions without a
    //                 migration; Rust-side enum serialisation is the
    //                 authoritative validator.
    //   payload     — JSON blob. `{}` for archive/unarchive/dismiss;
    //                 `ClassifyPayload { l1, l2, l3, confidence, source }`
    //                 for classify / reclassify.
    //   actor       — 'user' | 'classifier:<model-id>' | 'system:<reason>'.
    //                 Keeps audit-trail free-form so a new classifier
    //                 model or a one-off admin script needs no migration.
    //   at          — unix ms. INTEGER per STRICT-mode convention.
    //
    // Indexes:
    //   idx_action_session  : per-session history queries (UI "why was
    //                         this session archived?" trail, regression
    //                         investigation). Ordered (session_id, at)
    //                         so the common case (one session, sort by
    //                         time) is a zero-sort range scan.
    //   idx_action_actor_at : actor-filtered audit queries ("what did
    //                         classifier:haiku-4.5 do over the last
    //                         hour?"). Used by the Phase 5 classifier
    //                         retry / idempotency check and by Phase 7
    //                         observability dashboards.
    //
    // STRICT enforces the INTEGER / TEXT column types at INSERT time;
    // without it SQLite silently widens to ANY and a bad serialiser
    // could land a string `at`, which would corrupt the fold ordering.
    r#"BEGIN;
CREATE TABLE session_action_log (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT    NOT NULL,
    action      TEXT    NOT NULL,
    payload     TEXT    NOT NULL,
    actor       TEXT    NOT NULL,
    at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_action_session ON session_action_log(session_id, at);
CREATE INDEX idx_action_actor_at ON session_action_log(actor, at);
COMMIT;"#,
    // Migration 83: fold_state — single-row watermark for PR 5.3's
    // `spawn_flags_fold` task. Stores the max `session_action_log.seq`
    // the fold has applied to `session_flags`. Advanced in the SAME
    // transaction as the `session_flags` UPSERT so a kill-9 mid-batch
    // cannot leave the fold further ahead than the watermark (§7.2
    // kill-9 property: fold is resumable and byte-identical to a
    // one-shot replay).
    //
    // Why a single row instead of a COUNT()-derived watermark:
    //   - SELECT COUNT is O(log N) with sqlite's B-tree, but reading a
    //     single scalar is O(1) and cheaper by a constant factor.
    //   - Atomic UPDATE .. WHERE id = 0 keeps the watermark contract
    //     inside the same TX as the fold UPSERTs; COUNT-derived
    //     watermarks race with concurrent INSERTs to the log.
    //   - The `CHECK (id = 0)` constraint prevents accidental
    //     multi-row inserts that would split the watermark.
    //
    // Seeded at `applied_seq = 0`, which matches the default of the
    // `session_flags.applied_seq` column (migration 65). A fresh DB
    // that has never applied a fold reads the watermark as 0 and
    // starts from `seq > 0` — i.e. the first row of the log.
    r#"BEGIN;
CREATE TABLE fold_state (
    id          INTEGER PRIMARY KEY CHECK (id = 0),
    applied_seq INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO fold_state (id, applied_seq) VALUES (0, 0);
COMMIT;"#,
    // Migration 84: stage_c_outbox — durable delivery channel for
    // `FlagDelta` events produced by the PR 5.3 fold writer. The fold
    // writer INSERTs one row here in the SAME TX as each
    // `session_flags` UPSERT; Stage C's drainer task reads pending
    // rows (WHERE applied_at IS NULL), applies compensating rollup
    // UPDATEs, then marks applied_at. Crash-safe by construction —
    // neither side can acknowledge work the other didn't commit.
    //
    // Design: `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §6.2`.
    //
    // Columns:
    //   seq          — AUTOINCREMENT, same rationale as session_action_log
    //   delta_type   — 'flag_delta' today; future-proofed for other
    //                  compensating delta kinds (e.g. project rename)
    //   payload_json — serialized FlagDelta { session_id, kind,
    //                  before, after, at_ms }
    //   applied_at   — NULL until drainer commits the apply
    //
    // Indexes:
    //   idx_stage_c_outbox_pending — partial on (seq) WHERE
    //                  applied_at IS NULL. Keeps the drainer's scan
    //                  O(pending) instead of O(total).
    r#"BEGIN;
CREATE TABLE stage_c_outbox (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    delta_type   TEXT    NOT NULL,
    payload_json TEXT    NOT NULL,
    applied_at   INTEGER
) STRICT;
CREATE INDEX idx_stage_c_outbox_pending ON stage_c_outbox(seq) WHERE applied_at IS NULL;
COMMIT;"#,
    // Migration 85: IRREVERSIBLE — drop the legacy archive/category
    // columns from `sessions` and rewire the `valid_sessions` view to
    // filter on `session_flags.archived_at`.
    //
    // Gated by `scripts/rehearse-phase-5-6.sh` (manual run): the script
    // greps for any reader still touching `sessions.{archived_at,
    // category_*, classified_at}` and fails hard if it finds one. This
    // migration can only be committed once the grep is clean.
    //
    // Post-migration:
    //   - `session_flags` is the SOLE source of truth for archive /
    //     classification state (authoritative reads + the fold writer
    //     are already in place from PR 5.3 + Phase C).
    //   - `valid_sessions` still carries every non-flag column from
    //     `sessions.*`; the join adds the flag-side filter without
    //     reshaping the row type.
    //   - `sessions.archived_at | category_l1 | category_l2 |
    //     category_l3 | category_confidence | category_source |
    //     classified_at` are removed. Any forgotten reader fails
    //     loudly instead of silently returning stale data.
    //
    // `DROP INDEX` on `idx_sessions_category_l1` and
    // `idx_sessions_classified` is load-bearing: SQLite refuses
    // `ALTER TABLE DROP COLUMN` while an index references the column.
    //
    // IRREVERSIBLE: SQLite does not undo `ALTER TABLE DROP COLUMN`.
    // Back up `~/.claude-view/claude-view.db` before running.
    r#"BEGIN;
DROP VIEW IF EXISTS valid_sessions;
CREATE VIEW valid_sessions AS
  SELECT s.* FROM sessions s
  LEFT JOIN session_flags sf ON sf.session_id = s.id
  WHERE s.is_sidechain = 0 AND sf.archived_at IS NULL;
DROP INDEX IF EXISTS idx_sessions_category_l1;
DROP INDEX IF EXISTS idx_sessions_classified;
ALTER TABLE sessions DROP COLUMN archived_at;
ALTER TABLE sessions DROP COLUMN category_l1;
ALTER TABLE sessions DROP COLUMN category_l2;
ALTER TABLE sessions DROP COLUMN category_l3;
ALTER TABLE sessions DROP COLUMN category_confidence;
ALTER TABLE sessions DROP COLUMN category_source;
ALTER TABLE sessions DROP COLUMN classified_at;
COMMIT;"#,
    // Migration 86 (CQRS PR 6.2): add `invocation_counts` JSON column to
    // `session_stats`. The indexer_v2 writer populates it from every
    // `tool_use` block at parse time (keyed by tool name with a `:sub`
    // suffix for `Skill` / `Task` / `Agent`). Replaces the `invocations`
    // table — readers aggregate per-session JSON blobs directly.
    //
    // `STATS_VERSION` is bumped to 2 so the indexer re-extracts any row
    // whose stats_version is older than 2 on the next scan.
    r#"ALTER TABLE session_stats ADD COLUMN invocation_counts TEXT NOT NULL DEFAULT '{}';"#,
    // Migration 87 (CQRS PR 6.4): IRREVERSIBLE drop of the `turns` and
    // `invocations` tables. Their data has been folded into
    // `session_stats.per_model_tokens_json` (migration 64) and
    // `session_stats.invocation_counts` (migration 86). All readers were
    // cut over in PR 6.1 + PR 6.2; `scripts/rehearse-phase-6-4.sh` grep-
    // gates the cutover before this migration runs.
    //
    // Writers retired in PR 6.4: `batch_insert_turns_tx`,
    // `batch_insert_invocations_tx`, and their `Database` wrappers are
    // gone from the source tree — indexer_v2 is the only remaining
    // producer of the wide-row aggregates.
    //
    // IRREVERSIBLE: SQLite does not undo `DROP TABLE`. Back up
    // `~/.claude-view/claude-view.db` before running.
    r#"BEGIN;
DROP TABLE IF EXISTS turns;
DROP TABLE IF EXISTS invocations;
COMMIT;"#,
    // Migration 88 (CQRS Phase 7.c): extend `session_stats` with four new
    // columns required by the remaining `FROM sessions` readers (analytics,
    // contributions, stats dashboard, trends, enrichment).
    //
    // New columns:
    //   - `is_sidechain` — derives from JSONL is_sidechain flag
    //   - `commit_count` — mined from git history (will backfill via separate
    //     batch job); 0 for now
    //   - `reedited_files_count` — derives from JSONL file edit patterns
    //   - `skills_used` — JSON array of unique skill names invoked in session
    //
    // All columns default to 0 / '[]' so existing code and migrations continue
    // to work. The indexer_v2 writer will populate them for new parses;
    // a follow-up pass will backfill older sessions.
    //
    // `STATS_VERSION` bumps to 3 so the indexer re-extracts any row whose
    // stats_version < 3 on the next scan.
    r#"BEGIN;
ALTER TABLE session_stats ADD COLUMN is_sidechain INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN commit_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN reedited_files_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN skills_used TEXT NOT NULL DEFAULT '[]';
COMMIT;"#,
    // Migration 89 (CQRS Phase 7.h.1): extend `session_stats` to carry every
    // remaining column from the legacy `sessions` table. Column additions
    // only — the `valid_sessions` view rebuild and the IRREVERSIBLE DROP
    // land in subsequent migrations (90: rebuild view on session_stats,
    // 91: DROP TABLE sessions) once writers + readers have been rewired.
    //
    // Why this shape:
    //   - The operator directive was "data is rebuilt from JSONL; drop the
    //     table instead of a wide backfill". Extending `session_stats` with
    //     every column the readers use lets the indexer_v2 writer repopulate
    //     everything on the next scan after the DROP.
    //   - Exact sessions column names are preserved (e.g. `parse_version`,
    //     `tool_counts_bash`) so the reader SQL changes are file+table swaps,
    //     not column renames. The pre-existing session_stats parallel names
    //     (`parser_version`, `bash_count`) stay for compatibility with the
    //     Phase 2 writer; the Phase 7 writer populates both.
    //   - `STATS_VERSION` bumps to 4 so the indexer_v2 writer re-extracts
    //     any row whose stats_version < 4 on the next scan — this is the
    //     mechanism that populates the new columns for pre-migration rows.
    //
    // Column list (42 additions). All match sessions' exact names/types.
    r#"BEGIN;
ALTER TABLE session_stats ADD COLUMN project_display_name TEXT NOT NULL DEFAULT '';
ALTER TABLE session_stats ADD COLUMN project_path TEXT NOT NULL DEFAULT '';
ALTER TABLE session_stats ADD COLUMN summary TEXT;
ALTER TABLE session_stats ADD COLUMN message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN files_touched TEXT NOT NULL DEFAULT '[]';
ALTER TABLE session_stats ADD COLUMN tool_counts_edit INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN tool_counts_read INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN tool_counts_bash INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN tool_counts_write INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN deep_indexed_at INTEGER;
ALTER TABLE session_stats ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN file_size_at_index INTEGER;
ALTER TABLE session_stats ADD COLUMN file_mtime_at_index INTEGER;
ALTER TABLE session_stats ADD COLUMN api_call_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN files_read TEXT NOT NULL DEFAULT '[]';
ALTER TABLE session_stats ADD COLUMN files_edited TEXT NOT NULL DEFAULT '[]';
ALTER TABLE session_stats ADD COLUMN turn_duration_avg_ms INTEGER;
ALTER TABLE session_stats ADD COLUMN turn_duration_max_ms INTEGER;
ALTER TABLE session_stats ADD COLUMN turn_duration_total_ms INTEGER;
ALTER TABLE session_stats ADD COLUMN api_retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN compaction_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN hook_blocked_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN bash_progress_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN hook_progress_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN mcp_progress_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN summary_text TEXT;
ALTER TABLE session_stats ADD COLUMN lines_added INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN lines_removed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN loc_source INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN ai_lines_added INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN ai_lines_removed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE session_stats ADD COLUMN work_type TEXT;
ALTER TABLE session_stats ADD COLUMN total_task_time_seconds INTEGER;
ALTER TABLE session_stats ADD COLUMN longest_task_seconds INTEGER;
ALTER TABLE session_stats ADD COLUMN longest_task_preview TEXT;
ALTER TABLE session_stats ADD COLUMN total_cost_usd REAL;
ALTER TABLE session_stats ADD COLUMN slug TEXT;
ALTER TABLE session_stats ADD COLUMN entrypoint TEXT;
ALTER TABLE session_stats ADD COLUMN git_root TEXT;
ALTER TABLE session_stats ADD COLUMN session_cwd TEXT;
ALTER TABLE session_stats ADD COLUMN parent_session_id TEXT;
COMMIT;"#,
    // Migration 90 (CQRS Phase 7.h.4a): rebuild `valid_sessions` on top of
    // `session_stats JOIN session_flags`. The legacy `sessions` table is
    // still present until migration 91, but the view no longer references it.
    //
    // Phase 7.h.3 landed the dual-write contract: every production writer
    // that touched `sessions` also touched `session_stats`. The
    // `session_id AS id` alias preserves the row shape used by existing
    // `FROM valid_sessions s` readers.
    r#"BEGIN;
DROP VIEW IF EXISTS valid_sessions;
CREATE VIEW valid_sessions AS
  SELECT
    ss.session_id AS id,
    ss.project_id,
    ss.project_display_name,
    ss.project_path,
    ss.file_path,
    ss.preview,
    ss.summary,
    ss.message_count,
    ss.last_message_at,
    ss.first_message_at,
    ss.git_branch,
    ss.is_sidechain,
    ss.size_bytes,
    ss.indexed_at,
    ss.last_message,
    ss.files_touched,
    ss.skills_used,
    ss.tool_counts_edit,
    ss.tool_counts_read,
    ss.tool_counts_bash,
    ss.tool_counts_write,
    ss.turn_count,
    ss.deep_indexed_at,
    ss.parse_version,
    ss.file_size_at_index,
    ss.file_mtime_at_index,
    ss.user_prompt_count,
    ss.api_call_count,
    ss.tool_call_count,
    ss.files_read,
    ss.files_edited,
    ss.files_read_count,
    ss.files_edited_count,
    ss.reedited_files_count,
    ss.duration_seconds,
    ss.commit_count,
    ss.total_input_tokens,
    ss.total_output_tokens,
    ss.cache_read_tokens,
    ss.cache_creation_tokens,
    ss.thinking_block_count,
    ss.turn_duration_avg_ms,
    ss.turn_duration_max_ms,
    ss.turn_duration_total_ms,
    ss.api_error_count,
    ss.api_retry_count,
    ss.compaction_count,
    ss.hook_blocked_count,
    ss.agent_spawn_count,
    ss.bash_progress_count,
    ss.hook_progress_count,
    ss.mcp_progress_count,
    ss.summary_text,
    ss.lines_added,
    ss.lines_removed,
    ss.loc_source,
    ss.ai_lines_added,
    ss.ai_lines_removed,
    ss.work_type,
    ss.primary_model,
    ss.total_task_time_seconds,
    ss.longest_task_seconds,
    ss.longest_task_preview,
    ss.total_cost_usd,
    ss.slug,
    ss.entrypoint,
    ss.git_root,
    ss.session_cwd,
    ss.parent_session_id
  FROM session_stats ss
  LEFT JOIN session_flags sf ON sf.session_id = ss.session_id
  WHERE ss.is_sidechain = 0 AND sf.archived_at IS NULL;
COMMIT;"#,
    // Migration 91 (CQRS Phase 7.h.6): IRREVERSIBLE drop of the legacy
    // `sessions` table. Before the DROP, rebuild `session_commits` so its
    // cascade FK points at `session_stats(session_id)`; otherwise FK-enabled
    // databases would keep a dangling child-table reference to `sessions`.
    //
    // The insert keeps only links whose session and commit parents exist in
    // the post-CQRS tables. A link without a `session_stats` parent cannot be
    // queried correctly after the legacy table is removed.
    //
    // IRREVERSIBLE: SQLite does not undo `DROP TABLE`. Back up
    // `~/.claude-view/claude-view.db` before running.
    r#"PRAGMA foreign_keys=OFF;
BEGIN;
CREATE TABLE session_commits_new (
    session_id      TEXT NOT NULL REFERENCES session_stats(session_id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL REFERENCES commits(hash) ON DELETE CASCADE,
    tier            INTEGER NOT NULL CHECK (tier IN (1, 2)),
    evidence        TEXT NOT NULL DEFAULT '{}',
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (session_id, commit_hash)
);
INSERT INTO session_commits_new (session_id, commit_hash, tier, evidence, created_at)
SELECT sc.session_id, sc.commit_hash, sc.tier, sc.evidence, sc.created_at
FROM session_commits sc
JOIN session_stats ss ON ss.session_id = sc.session_id
JOIN commits c ON c.hash = sc.commit_hash;
DROP TABLE session_commits;
ALTER TABLE session_commits_new RENAME TO session_commits;
CREATE INDEX idx_session_commits_session ON session_commits(session_id);
CREATE INDEX idx_session_commits_commit ON session_commits(commit_hash);
CREATE INDEX IF NOT EXISTS idx_session_stats_project_branch ON session_stats(project_id, git_branch);
CREATE INDEX IF NOT EXISTS idx_session_stats_sidechain ON session_stats(is_sidechain);
CREATE INDEX IF NOT EXISTS idx_session_stats_commit_count ON session_stats(commit_count) WHERE commit_count > 0;
CREATE INDEX IF NOT EXISTS idx_session_stats_reedit ON session_stats(reedited_files_count) WHERE reedited_files_count > 0;
CREATE INDEX IF NOT EXISTS idx_session_stats_duration ON session_stats(duration_seconds);
CREATE INDEX IF NOT EXISTS idx_session_stats_needs_reindex ON session_stats(session_id, file_path) WHERE parse_version < 1;
CREATE INDEX IF NOT EXISTS idx_session_stats_first_message ON session_stats(first_message_at);
CREATE INDEX IF NOT EXISTS idx_session_stats_project_first_message ON session_stats(project_id, first_message_at);
DROP TABLE IF EXISTS sessions;
COMMIT;
PRAGMA foreign_keys=ON;"#,
];
