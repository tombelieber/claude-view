/// Inline SQL migrations for vibe-recall database schema.
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
];
