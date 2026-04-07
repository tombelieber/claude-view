// crates/db/src/indexer_parallel/types.rs
// Public types: ParsedSession, IndexHints, ExtendedMetadata, ParseResult,
// DeepIndexResult, ParseDiagnostics, RawInvocation, CommitSkillInvocation, FileData, constants.

use claude_view_core::ToolCounts;
use std::io;
use std::path::Path;

/// Current parse version. Bump when parser logic changes to trigger re-indexing.
/// Version 1: Full 7-type extraction, ParseDiagnostics, spacing fix.
/// Version 2: File modification detection (file_size_at_index, file_mtime_at_index).
/// Version 3: Deep indexing now updates last_message_at from parsed timestamps.
/// Version 4: LOC estimation, AI contribution tracking, work type classification.
/// Version 5: Git branch extraction, primary model detection.
/// Version 6: Wall-clock task time fields (total_task_time_seconds, longest_task_seconds).
/// Version 7: Search index uses effective project identity (git_root or project_id) for sidebar filter alignment.
/// Version 8: Model field changed to TEXT for partial matching, skills piped to Tantivy.
/// Version 9: Token dedup via message.id:requestId, api_call_count from unique API calls.
/// Version 10: Persist session total_cost_usd.
/// Version 11: Unified session pipeline — single-pass parse-before-write.
/// Version 12: valid_sessions view no longer requires last_message_at > 0.
/// Version 13: Unified pipeline writes turns, models, invocations, and search index.
/// Version 14: Task-time turns split on interactive human tool_result responses.
/// Version 15: Cost computed from token counts.
/// Version 16: Per-turn cost computation (200k tiering per-API-request, not per-session).
/// Version 18: Per-turn fallback cost honors cache_creation split (5m/1h).
/// Version 19: Remove JSONL top-level cost-field parsing path; compute from token usage only.
/// Version 20: Strict cost integrity: if any turn model is unpriced, session total_cost_usd is NULL.
/// Version 21: Source-message integrity hardening (role filtering, summary exclusion, strict tool_use LOC/path extraction).
/// Version 22: Extract hook_progress events into hook_events table for backfill.
pub const CURRENT_PARSE_VERSION: i32 = 22;

/// Complete parsed session data -- the sole input to any DB write.
/// Every field is populated by the parser. No field is ever set from
/// filesystem metadata alone.
#[derive(Debug, Clone)]
pub struct ParsedSession {
    pub id: String,
    pub project_id: String,
    pub project_display_name: String,
    pub project_path: String,
    pub file_path: String,
    pub preview: String,
    pub summary: Option<String>,
    pub message_count: i32,
    pub last_message_at: i64,
    pub first_message_at: i64,
    pub git_branch: Option<String>,
    pub is_sidechain: bool,
    pub size_bytes: i64,
    // Deep parse fields
    pub last_message: String,
    pub turn_count: i32,
    pub tool_counts_edit: i32,
    pub tool_counts_read: i32,
    pub tool_counts_bash: i32,
    pub tool_counts_write: i32,
    pub files_touched: String, // JSON array
    pub skills_used: String,   // JSON array
    pub user_prompt_count: i32,
    pub api_call_count: i32,
    pub tool_call_count: i32,
    pub files_read: String,   // JSON array
    pub files_edited: String, // JSON array
    pub files_read_count: i32,
    pub files_edited_count: i32,
    pub reedited_files_count: i32,
    pub duration_seconds: i64,
    pub commit_count: i32,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub thinking_block_count: i32,
    pub turn_duration_avg_ms: Option<i64>,
    pub turn_duration_max_ms: Option<i64>,
    pub turn_duration_total_ms: Option<i64>,
    pub api_error_count: i32,
    pub api_retry_count: i32,
    pub compaction_count: i32,
    pub hook_blocked_count: i32,
    pub agent_spawn_count: i32,
    pub bash_progress_count: i32,
    pub hook_progress_count: i32,
    pub mcp_progress_count: i32,
    pub summary_text: Option<String>,
    pub parse_version: i32,
    pub file_size_at_index: i64,
    pub file_mtime_at_index: i64,
    pub lines_added: i64,
    pub lines_removed: i64,
    pub loc_source: i32,
    pub ai_lines_added: i64,
    pub ai_lines_removed: i64,
    pub work_type: Option<String>,
    pub primary_model: Option<String>,
    pub total_task_time_seconds: Option<i64>,
    pub longest_task_seconds: Option<i64>,
    pub longest_task_preview: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub slug: Option<String>,
    pub entrypoint: Option<String>,
}

/// Hints from sessions-index.json -- merged during parse, never written to DB directly.
#[derive(Debug, Clone, Default)]
pub struct IndexHints {
    pub is_sidechain: Option<bool>,
    pub project_path: Option<String>,
    pub project_display_name: Option<String>,
    pub git_branch: Option<String>,
    pub summary: Option<String>,
}

/// Extended metadata extracted from JSONL deep parsing (Pass 2 only).
/// These fields are NOT available from sessions-index.json.
#[derive(Debug, Clone, Default)]
pub struct ExtendedMetadata {
    pub tool_counts: ToolCounts,
    pub skills_used: Vec<String>,
    pub files_touched: Vec<String>,
    pub last_message: String,
    pub turn_count: usize,
    // Phase 3: Atomic unit metrics
    /// Count of JSONL lines where `.type == "user"`
    pub user_prompt_count: u32,
    /// Count of JSONL lines where `.type == "assistant"`
    pub api_call_count: u32,
    /// Count of all `tool_use` blocks across all assistant messages
    pub tool_call_count: u32,
    /// Unique file paths from Read tool invocations (deduplicated)
    pub files_read: Vec<String>,
    /// All file paths from Edit/Write tool invocations (NOT deduplicated - needed for re-edit calc)
    pub files_edited: Vec<String>,
    /// Count of unique files read
    pub files_read_count: u32,
    /// Count of unique files edited
    pub files_edited_count: u32,
    /// Count of files that were edited 2+ times
    pub reedited_files_count: u32,
    /// Duration in seconds (last timestamp - first timestamp)
    pub duration_seconds: u32,
    /// First message timestamp (Unix seconds)
    pub first_timestamp: Option<i64>,
    /// Last message timestamp (Unix seconds)
    pub last_timestamp: Option<i64>,

    // Token aggregates (from assistant lines)
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_block_count: u32,

    // System line metrics
    pub turn_durations_ms: Vec<u64>,
    pub api_error_count: u32,
    pub api_retry_count: u32,
    pub compaction_count: u32,
    pub hook_blocked_count: u32,

    // Progress line metrics
    pub agent_spawn_count: u32,
    pub bash_progress_count: u32,
    pub hook_progress_count: u32,
    pub hook_progress_events: Vec<crate::queries::hook_events::HookEventRow>,
    pub mcp_progress_count: u32,

    // Summary text (from summary lines)
    pub summary_text: Option<String>,

    // Queue operations
    pub queue_enqueue_count: u32,
    pub queue_dequeue_count: u32,

    // File history snapshots
    pub file_snapshot_count: u32,

    // Theme 3: AI contribution tracking
    /// Lines added by AI (from Edit/Write tool_use)
    pub ai_lines_added: u32,
    /// Lines removed by AI (from Edit tool_use)
    pub ai_lines_removed: u32,

    /// First user prompt text (truncated to 500 chars).
    /// Used to populate `preview` for filesystem-discovered sessions that lack index metadata.
    pub first_user_prompt: Option<String>,

    // Turn tracking for wall-clock task time
    /// Timestamp of the current (in-progress) turn start
    pub current_turn_start_ts: Option<i64>,
    /// First 60 chars of the current turn's prompt
    pub current_turn_prompt: Option<String>,
    /// Longest single turn in wall-clock seconds
    pub longest_task_seconds: Option<u32>,
    /// First 60 chars of the prompt that started the longest turn
    pub longest_task_preview: Option<String>,
    /// Sum of all turn wall-clock durations in seconds
    pub total_task_time_seconds: u32,
}

/// A raw tool_use extracted from JSONL, before classification.
#[derive(Debug, Clone)]
pub struct RawInvocation {
    pub name: String,
    pub input: Option<serde_json::Value>,
    pub byte_offset: usize,
    pub timestamp: i64,
}

/// A commit-related skill invocation detected for git correlation (Tier 1).
///
/// Detected when:
/// - Tool name is "Skill"
/// - Input contains `skill` field with one of:
///   - "commit"
///   - "commit-commands:commit"
///   - "commit-commands:commit-push-pr"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSkillInvocation {
    /// The skill name (e.g., "commit", "commit-commands:commit")
    pub skill_name: String,
    /// Unix timestamp in seconds when the skill was invoked
    pub timestamp_unix: i64,
}

/// Skill names that indicate a commit-related action.
pub const COMMIT_SKILL_NAMES: &[&str] = &[
    "commit",
    "commit-commands:commit",
    "commit-commands:commit-push-pr",
];

/// Result of parse_bytes(): deep metadata plus raw tool invocations.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub deep: ExtendedMetadata,
    pub raw_invocations: Vec<RawInvocation>,
    pub turns: Vec<claude_view_core::RawTurn>,
    pub models_seen: Vec<String>,
    pub diagnostics: ParseDiagnostics,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub git_branch: Option<String>,
    /// Working directory from the first user message's `cwd` field (authoritative source).
    pub cwd: Option<String>,
    /// Session slug from top-level JSONL field (e.g. "async-greeting-dewdrop").
    pub slug: Option<String>,
    /// Collected message content for full-text search indexing.
    /// Only user, assistant text, and tool_use inputs are included.
    pub search_messages: Vec<claude_view_core::SearchableMessage>,
    /// How the session was launched (cli, claude-vscode, sdk-ts).
    pub entrypoint: Option<String>,
}

/// Collected results from one session's parse phase, to be written in a single transaction.
///
/// This struct holds everything needed to persist a session's deep index data.
/// Used by the collect-then-write pattern in `pass_2_deep_index`: parallel tasks
/// return these results, then one transaction writes them all.
pub struct DeepIndexResult {
    pub session_id: String,
    pub file_path: String,
    /// Effective project identity (git_root or project_id), used for search indexing.
    pub project: String,
    pub parse_result: ParseResult,
    /// Pre-classified invocations: (source_file, byte_offset, invocable_id, session_id, project, timestamp)
    pub classified_invocations: Vec<(String, i64, String, String, String, i64)>,
    /// File size in bytes at the time of this parse (for change detection on next startup).
    pub file_size: i64,
    /// File mtime as Unix seconds at the time of this parse (for change detection on next startup).
    pub file_mtime: i64,
}

/// Parse diagnostic counters -- per session, not per line.
/// Zero per-line overhead: stack-allocated integers incremented in existing branches.
#[derive(Debug, Clone, Default)]
pub struct ParseDiagnostics {
    // Line classification
    pub lines_total: u32,
    pub lines_empty: u32,
    pub lines_user: u32,
    pub lines_assistant: u32,
    pub lines_system: u32,
    pub lines_progress: u32,
    pub lines_queue_op: u32,
    pub lines_file_snapshot: u32,
    pub lines_unknown_type: u32,

    // Parse outcomes
    pub json_parse_attempts: u32,
    pub json_parse_failures: u32,
    pub content_not_array: u32,
    pub tool_use_missing_name: u32,

    // Extraction outcomes
    pub tool_use_blocks_found: u32,
    pub file_paths_extracted: u32,
    pub timestamps_extracted: u32,
    pub timestamps_unparseable: u32,
    pub turns_extracted: u32,
    pub unknown_source_role_count: u32,
    pub derived_source_message_doc_count: u32,
    pub source_message_non_source_provenance_count: u32,

    // Bytes
    pub bytes_total: u64,
}

/// Zero-copy file data: holds either an mmap (large files) or a Vec (small files).
/// Derefs to `&[u8]` so callers can use it transparently as a byte slice.
/// The mmap stays alive for the lifetime of this value, enabling zero-copy parsing.
pub enum FileData {
    Mmap(memmap2::Mmap),
    Vec(Vec<u8>),
}

impl std::ops::Deref for FileData {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            FileData::Mmap(m) => m,
            FileData::Vec(v) => v,
        }
    }
}

/// Read a file using memory-mapped I/O with fallback to regular read.
/// mmap is faster for large files (>64KB) because it avoids copying data through kernel buffers.
///
/// Returns a `FileData` that derefs to `&[u8]`. For large files, the data is memory-mapped
/// (zero-copy); for small files, it's read into a Vec. The mmap stays alive as long as the
/// returned `FileData` is held, so parsing can happen directly on the mapped pages.
pub fn read_file_fast(path: &Path) -> io::Result<FileData> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len() as usize;

    if len == 0 {
        return Ok(FileData::Vec(Vec::new()));
    }

    // For small files, regular read is faster (no mmap overhead)
    if len < 64 * 1024 {
        return Ok(FileData::Vec(std::fs::read(path)?));
    }

    // Try mmap, fall back to regular read on failure
    // SAFETY: The file is read-only and we only hold the mapping briefly.
    // Claude Code appends to JSONL (never truncates), so the file won't shrink.
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(mmap) => Ok(FileData::Mmap(mmap)),
        Err(_) => Ok(FileData::Vec(std::fs::read(path)?)),
    }
}

/// Collected results from one session's parse phase, ready for batched DB + search write.
pub(crate) struct IndexedSession {
    pub parsed: ParsedSession,
    pub turns: Vec<claude_view_core::RawTurn>,
    pub models_seen: Vec<String>,
    pub classified_invocations: Vec<(String, i64, String, String, String, i64)>,
    pub search_messages: Vec<claude_view_core::SearchableMessage>,
    pub cwd: Option<String>,
    pub git_root: Option<String>,
    /// Project name for search indexing
    pub project_for_search: String,
    /// Per-session parse diagnostics for integrity counter aggregation
    pub diagnostics: ParseDiagnostics,
    /// Hook progress events extracted from JSONL for backfill into hook_events table
    pub hook_progress_events: Vec<crate::queries::hook_events::HookEventRow>,
}
