// crates/db/src/indexer_parallel.rs
// Fast JSONL parsing with memory-mapped I/O and SIMD-accelerated scanning.
// Also contains the two-pass indexing pipeline: Pass 1 (index JSON) and Pass 2 (deep JSONL).

use memchr::memmem;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use vibe_recall_core::{
    classify_work_type, count_ai_lines, discover_orphan_sessions, read_all_session_indexes,
    resolve_project_path, resolve_worktree_parent, ClassificationInput, ClassifyResult, Registry,
    ToolCounts,
};

use crate::Database;

/// Current parse version. Bump when parser logic changes to trigger re-indexing.
/// Version 1: Full 7-type extraction, ParseDiagnostics, spacing fix.
/// Version 2: File modification detection (file_size_at_index, file_mtime_at_index).
/// Version 3: Deep indexing now updates last_message_at from parsed timestamps.
/// Version 4: LOC estimation, AI contribution tracking, work type classification.
/// Version 5: Git branch extraction, primary model detection.
/// Version 6: Wall-clock task time fields (total_task_time_seconds, longest_task_seconds).
pub const CURRENT_PARSE_VERSION: i32 = 6;

// ---------------------------------------------------------------------------
// SQL constants for rusqlite write-phase prepared statements.
// Must match the sqlx `_tx` function in queries.rs exactly (54 params).
// ---------------------------------------------------------------------------

const UPDATE_SESSION_DEEP_SQL: &str = r#"
    UPDATE sessions SET
        last_message = ?2,
        turn_count = ?3,
        tool_counts_edit = ?4,
        tool_counts_read = ?5,
        tool_counts_bash = ?6,
        tool_counts_write = ?7,
        files_touched = ?8,
        skills_used = ?9,
        deep_indexed_at = ?10,
        user_prompt_count = ?11,
        api_call_count = ?12,
        tool_call_count = ?13,
        files_read = ?14,
        files_edited = ?15,
        files_read_count = ?16,
        files_edited_count = ?17,
        reedited_files_count = ?18,
        duration_seconds = ?19,
        commit_count = ?20,
        first_message_at = ?21,
        total_input_tokens = ?22,
        total_output_tokens = ?23,
        cache_read_tokens = ?24,
        cache_creation_tokens = ?25,
        thinking_block_count = ?26,
        turn_duration_avg_ms = ?27,
        turn_duration_max_ms = ?28,
        turn_duration_total_ms = ?29,
        api_error_count = ?30,
        api_retry_count = ?31,
        compaction_count = ?32,
        hook_blocked_count = ?33,
        agent_spawn_count = ?34,
        bash_progress_count = ?35,
        hook_progress_count = ?36,
        mcp_progress_count = ?37,
        summary_text = ?38,
        parse_version = ?39,
        file_size_at_index = ?40,
        file_mtime_at_index = ?41,
        lines_added = ?42,
        lines_removed = ?43,
        loc_source = ?44,
        ai_lines_added = ?45,
        ai_lines_removed = ?46,
        work_type = ?47,
        git_branch = COALESCE(NULLIF(TRIM(?48), ''), git_branch),
        primary_model = ?49,
        last_message_at = COALESCE(?50, last_message_at),
        preview = CASE WHEN (preview IS NULL OR preview = '') AND ?51 IS NOT NULL THEN ?51 ELSE preview END,
        total_task_time_seconds = ?52,
        longest_task_seconds = ?53,
        longest_task_preview = ?54
    WHERE id = ?1
"#;

const INSERT_TURN_SQL: &str = r#"
    INSERT OR IGNORE INTO turns (
        session_id, uuid, seq, model_id, parent_uuid,
        content_type, input_tokens, output_tokens,
        cache_read_tokens, cache_creation_tokens,
        service_tier, timestamp
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5,
        ?6, ?7, ?8,
        ?9, ?10,
        ?11, ?12
    )
"#;

const INSERT_INVOCATION_SQL: &str = r#"
    INSERT OR IGNORE INTO invocations
        (source_file, byte_offset, invocable_id, session_id, project, timestamp)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
"#;

const UPSERT_MODEL_SQL: &str = r#"
    INSERT INTO models (id, provider, family, first_seen, last_seen)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(id) DO UPDATE SET
        last_seen = MAX(models.last_seen, excluded.last_seen)
"#;

/// Compute the primary model for a session: the model_id with the most turns.
fn compute_primary_model(turns: &[vibe_recall_core::RawTurn]) -> Option<String> {
    if turns.is_empty() {
        return None;
    }
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for turn in turns {
        *counts.entry(&turn.model_id).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(model, _)| model.to_string())
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

// ---------------------------------------------------------------------------
// Typed structs for fast dispatched parsing (avoid full serde_json::Value).
// Private to this file — only used inside parse_bytes().
// ---------------------------------------------------------------------------

/// Handles both integer and ISO8601 string timestamps.
#[derive(Deserialize)]
#[serde(untagged)]
enum TimestampValue {
    Integer(i64),
    Iso(String),
}

impl TimestampValue {
    fn to_unix(&self) -> Option<i64> {
        match self {
            TimestampValue::Integer(v) => Some(*v),
            TimestampValue::Iso(s) => chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp()),
        }
    }
}

#[derive(Deserialize)]
struct AssistantLine {
    timestamp: Option<TimestampValue>,
    uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    parent_uuid: Option<String>,
    message: Option<AssistantMessage>,
}

#[derive(Deserialize)]
struct AssistantMessage {
    model: Option<String>,
    usage: Option<UsageBlock>,
    #[serde(default, deserialize_with = "deserialize_content")]
    content: ContentResult,
}

#[derive(Deserialize)]
struct UsageBlock {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    service_tier: Option<String>,
}

/// Custom visitor result — avoids #[serde(untagged)] buffering overhead.
#[derive(Default)]
enum ContentResult {
    Blocks(Vec<FlatContentBlock>),
    NotArray,
    #[default]
    Missing,
}

/// Custom deserializer for message content that avoids `#[serde(untagged)]` buffering.
/// Directly deserializes array elements as `FlatContentBlock` — no intermediate Value tree.
fn deserialize_content<'de, D: Deserializer<'de>>(d: D) -> Result<ContentResult, D::Error> {
    struct ContentVisitor;

    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = ContentResult;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an array of content blocks, a string, or null")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ContentResult, A::Error> {
            let mut blocks = Vec::new();
            while let Some(block) = seq.next_element::<FlatContentBlock>()? {
                blocks.push(block);
            }
            Ok(ContentResult::Blocks(blocks))
        }

        fn visit_str<E: de::Error>(self, _: &str) -> Result<ContentResult, E> {
            Ok(ContentResult::NotArray)
        }

        fn visit_string<E: de::Error>(self, _: String) -> Result<ContentResult, E> {
            Ok(ContentResult::NotArray)
        }

        fn visit_none<E: de::Error>(self) -> Result<ContentResult, E> {
            Ok(ContentResult::Missing)
        }

        fn visit_unit<E: de::Error>(self) -> Result<ContentResult, E> {
            Ok(ContentResult::Missing)
        }
    }

    d.deserialize_any(ContentVisitor)
}

/// Flat content block — only declares fields we actually read.
/// Serde skips undeclared fields (text content, thinking content) without allocating them.
#[derive(Deserialize)]
struct FlatContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct SystemLine {
    timestamp: Option<TimestampValue>,
    subtype: Option<String>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    #[serde(rename = "retryAttempt")]
    retry_attempt: Option<u64>,
    #[serde(rename = "preventedContinuation")]
    prevented_continuation: Option<bool>,
}

#[derive(Deserialize)]
struct SummaryLine {
    timestamp: Option<TimestampValue>,
    summary: Option<String>,
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

/// Extract commit skill invocations from raw tool_use invocations.
///
/// Filters for invocations where:
/// - `name == "Skill"`
/// - `input.skill` is one of the commit-related skill names
///
/// Returns a list of `CommitSkillInvocation` with skill name and timestamp.
pub fn extract_commit_skill_invocations(raw_invocations: &[RawInvocation]) -> Vec<CommitSkillInvocation> {
    raw_invocations
        .iter()
        .filter_map(|inv| {
            // Only process Skill tool invocations
            if inv.name != "Skill" {
                return None;
            }

            // Extract the skill name from input.skill
            let skill_name = inv
                .input
                .as_ref()?
                .get("skill")?
                .as_str()?;

            // Check if it's a commit-related skill
            if COMMIT_SKILL_NAMES.contains(&skill_name) {
                Some(CommitSkillInvocation {
                    skill_name: skill_name.to_string(),
                    timestamp_unix: inv.timestamp,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Result of parse_bytes(): deep metadata plus raw tool invocations.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub deep: ExtendedMetadata,
    pub raw_invocations: Vec<RawInvocation>,
    pub turns: Vec<vibe_recall_core::RawTurn>,
    pub models_seen: Vec<String>,
    pub diagnostics: ParseDiagnostics,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub git_branch: Option<String>,
}

/// Collected results from one session's parse phase, to be written in a single transaction.
///
/// This struct holds everything needed to persist a session's deep index data.
/// Used by the collect-then-write pattern in `pass_2_deep_index`: parallel tasks
/// return these results, then one transaction writes them all.
pub struct DeepIndexResult {
    pub session_id: String,
    pub file_path: String,
    pub parse_result: ParseResult,
    /// Pre-classified invocations: (source_file, byte_offset, invocable_id, session_id, project, timestamp)
    pub classified_invocations: Vec<(String, i64, String, String, String, i64)>,
    /// File size in bytes at the time of this parse (for change detection on next startup).
    pub file_size: i64,
    /// File mtime as Unix seconds at the time of this parse (for change detection on next startup).
    pub file_mtime: i64,
}

/// Parse diagnostic counters — per session, not per line.
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
    pub lines_summary: u32,
    pub lines_file_snapshot: u32,
    pub lines_hook_context: u32,
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

/// Extract lines added/removed from Edit or Write tool_use blocks.
///
/// For Edit: counts lines in old_string (removed) and new_string (added).
/// For Write: counts lines in content (all added, none removed).
///
/// Returns (lines_added, lines_removed).
fn extract_loc_from_tool_use(line: &[u8], is_edit: bool) -> (u32, u32) {
    // Parse as JSON Value to access input fields
    let value: serde_json::Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return (0, 0),
    };

    let input = match value.get("input") {
        Some(inp) => inp,
        None => return (0, 0),
    };

    if is_edit {
        // Edit tool: old_string → removed, new_string → added
        let old = input.get("old_string").and_then(|s| s.as_str()).unwrap_or("");
        let new = input.get("new_string").and_then(|s| s.as_str()).unwrap_or("");
        let old_lines = old.lines().count() as u32;
        let new_lines = new.lines().count() as u32;
        (new_lines, old_lines)
    } else {
        // Write tool: all lines are added
        let content = input.get("content").and_then(|c| c.as_str()).unwrap_or("");
        let lines = content.lines().count() as u32;
        (lines, 0)
    }
}

/// Full JSON parser that extracts all 7 JSONL line types.
///
/// Returns a `ParseResult` containing deep metadata (tool counts, skills, token usage,
/// system metrics, progress counts, etc.) and raw tool_use invocations for downstream
/// classification.
///
/// Every non-empty line is parsed with `serde_json::from_slice::<Value>`, then dispatched
/// on the `type` field: user, assistant, system, progress, queue-operation, summary,
/// file-history-snapshot. This replaces the previous SIMD-selective parser that only
/// handled user and assistant lines.
pub fn parse_bytes(data: &[u8]) -> ParseResult {
    let mut result = ParseResult::default();
    let diag = &mut result.diagnostics;
    diag.bytes_total = data.len() as u64;

    let mut user_count = 0u32;
    let mut assistant_count = 0u32;
    let mut last_user_content: Option<String> = None;
    let mut first_user_content: Option<String> = None;
    let mut tool_call_count = 0u32;
    let mut files_read_all: Vec<String> = Vec::new();
    let mut files_edited_all: Vec<String> = Vec::new();
    let mut first_timestamp: Option<i64> = None;
    let mut last_timestamp: Option<i64> = None;

    // Keep SIMD finders for string-level extraction within already-classified lines
    let content_finder = memmem::Finder::new(b"\"content\":\"");
    let text_finder = memmem::Finder::new(b"\"text\":\"");
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");

    // SIMD type detectors — check raw bytes before JSON parse
    let type_progress = memmem::Finder::new(b"\"type\":\"progress\"");
    let type_queue_op = memmem::Finder::new(b"\"type\":\"queue-operation\"");
    let type_file_snap = memmem::Finder::new(b"\"type\":\"file-history-snapshot\"");
    let type_hook_ctx = memmem::Finder::new(b"\"type\":\"saved_hook_context\"");

    // Progress subtype detectors
    let subtype_agent = memmem::Finder::new(b"\"type\":\"agent_progress\"");
    let subtype_bash = memmem::Finder::new(b"\"type\":\"bash_progress\"");
    let subtype_hook = memmem::Finder::new(b"\"type\":\"hook_progress\"");
    let subtype_mcp = memmem::Finder::new(b"\"type\":\"mcp_progress\"");

    // Queue operation detectors
    let op_enqueue = memmem::Finder::new(b"\"operation\":\"enqueue\"");
    let op_dequeue = memmem::Finder::new(b"\"operation\":\"dequeue\"");

    // Type-dispatched parsing: SIMD pre-filter before typed struct deserialization
    let type_user = memmem::Finder::new(b"\"type\":\"user\"");
    let type_assistant = memmem::Finder::new(b"\"type\":\"assistant\"");
    let type_system = memmem::Finder::new(b"\"type\":\"system\"");
    let type_summary = memmem::Finder::new(b"\"type\":\"summary\"");
    let timestamp_finder = memmem::Finder::new(b"\"timestamp\":");

    // Phase C: LOC estimation - SIMD finders for Edit/Write tool_use blocks
    let tool_use_finder = memmem::Finder::new(b"\"type\":\"tool_use\"");
    let edit_finder = memmem::Finder::new(b"\"name\":\"Edit\"");
    let write_finder = memmem::Finder::new(b"\"name\":\"Write\"");

    // Turn detection: tool_result user messages are continuations, not real turns
    let tool_result_finder = memmem::Finder::new(b"\"tool_result\"");

    // gitBranch extraction
    let git_branch_finder = memmem::Finder::new(b"\"gitBranch\":\"");

    for (byte_offset, line) in split_lines_with_offsets(data) {
        if line.is_empty() {
            diag.lines_empty += 1;
            continue;
        }

        diag.lines_total += 1;

        // Extract gitBranch (appears on every JSONL line, grab from the first match)
        if result.git_branch.is_none() {
            if let Some(pos) = git_branch_finder.find(line) {
                let start = pos + b"\"gitBranch\":\"".len();
                if let Some(branch) = extract_quoted_string(&line[start..]) {
                    if !branch.is_empty() {
                        result.git_branch = Some(branch);
                    }
                }
            }
        }

        // SIMD fast path: lightweight types that don't need full JSON parse
        if type_progress.find(line).is_some() {
            diag.lines_progress += 1;
            if subtype_agent.find(line).is_some() {
                result.deep.agent_spawn_count += 1;
            } else if subtype_bash.find(line).is_some() {
                result.deep.bash_progress_count += 1;
            } else if subtype_hook.find(line).is_some() {
                result.deep.hook_progress_count += 1;
            } else if subtype_mcp.find(line).is_some() {
                result.deep.mcp_progress_count += 1;
            }
            continue;
        }

        if type_queue_op.find(line).is_some() {
            diag.lines_queue_op += 1;
            if op_enqueue.find(line).is_some() {
                result.deep.queue_enqueue_count += 1;
            } else if op_dequeue.find(line).is_some() {
                result.deep.queue_dequeue_count += 1;
            }
            continue;
        }

        if type_file_snap.find(line).is_some() {
            diag.lines_file_snapshot += 1;
            result.deep.file_snapshot_count += 1;
            continue;
        }

        if type_hook_ctx.find(line).is_some() {
            diag.lines_hook_context += 1;
            continue;
        }

        // ── Type-dispatched parsing ─────────────────────────────────────
        // SIMD pre-filter on raw bytes determines which typed struct to use.
        // User lines: raw bytes only (no JSON). Assistant/system/summary:
        // typed structs (skip unneeded fields). Fallback: Value for unknowns.

        // User lines: raw-byte path (no JSON parse at all)
        if type_user.find(line).is_some() {
            diag.lines_user += 1;
            user_count += 1;
            // Check if this is a tool_result continuation (not a real user turn)
            let is_tool_result = tool_result_finder.find(line).is_some();
            if let Some(content) = extract_first_text_content(line, &content_finder, &text_finder) {
                if first_user_content.is_none() && !is_system_user_content(&content) && !is_tool_result {
                    first_user_content = Some(content.clone());
                }
                last_user_content = Some(content.clone());

                // Turn detection: real user prompts (not system prefixes, not tool_result)
                if !is_tool_result && !is_system_user_content(&content) {
                    let current_ts = extract_timestamp_from_bytes(line, &timestamp_finder);
                    // Close previous turn if one was open
                    if let (Some(start_ts), Some(end_ts)) = (result.deep.current_turn_start_ts, last_timestamp) {
                        let wall_secs = (end_ts - start_ts).max(0) as u32;
                        result.deep.total_task_time_seconds += wall_secs;
                        if result.deep.longest_task_seconds.map_or(true, |prev| wall_secs > prev) {
                            result.deep.longest_task_seconds = Some(wall_secs);
                            result.deep.longest_task_preview = result.deep.current_turn_prompt.take();
                        }
                    }
                    // Start new turn
                    result.deep.current_turn_start_ts = current_ts;
                    result.deep.current_turn_prompt = Some(content.chars().take(60).collect());
                }
            }
            extract_skills_from_line(line, &skill_name_finder, &mut result.deep.skills_used);
            if let Some(ts) = extract_timestamp_from_bytes(line, &timestamp_finder) {
                diag.timestamps_extracted += 1;
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }
            continue;
        }

        // Assistant lines: typed struct parse (skips text/thinking content allocation)
        if type_assistant.find(line).is_some() {
            diag.json_parse_attempts += 1;
            match serde_json::from_slice::<AssistantLine>(line) {
                Ok(parsed) => {
                    handle_assistant_line(
                        parsed,
                        byte_offset,
                        &mut result.deep,
                        &mut result.raw_invocations,
                        &mut result.turns,
                        &mut result.models_seen,
                        diag,
                        &mut assistant_count,
                        &mut tool_call_count,
                        &mut files_read_all,
                        &mut files_edited_all,
                        &mut first_timestamp,
                        &mut last_timestamp,
                    );

                    // Phase C: LOC estimation from Edit/Write tool_use blocks
                    // SIMD pre-filter: only parse lines that are tool_use AND (Edit OR Write)
                    if tool_use_finder.find(line).is_some() {
                        let is_edit = edit_finder.find(line).is_some();
                        let is_write = write_finder.find(line).is_some();
                        if is_edit || is_write {
                            let (added, removed) = extract_loc_from_tool_use(line, is_edit);
                            result.lines_added = result.lines_added.saturating_add(added);
                            result.lines_removed = result.lines_removed.saturating_add(removed);
                        }
                    }
                }
                Err(_) => {
                    diag.json_parse_failures += 1;
                }
            }
            continue;
        }

        // System lines: small flat struct
        if type_system.find(line).is_some() {
            diag.json_parse_attempts += 1;
            match serde_json::from_slice::<SystemLine>(line) {
                Ok(parsed) => {
                    handle_system_line(parsed, &mut result.deep, diag, &mut first_timestamp, &mut last_timestamp);
                }
                Err(_) => {
                    diag.json_parse_failures += 1;
                }
            }
            continue;
        }

        // Summary lines: tiny flat struct
        if type_summary.find(line).is_some() {
            diag.json_parse_attempts += 1;
            match serde_json::from_slice::<SummaryLine>(line) {
                Ok(parsed) => {
                    handle_summary_line(parsed, &mut result.deep, diag, &mut first_timestamp, &mut last_timestamp);
                }
                Err(_) => {
                    diag.json_parse_failures += 1;
                }
            }
            continue;
        }

        // ── Fallback: full Value parse for spacing variants and unknown types ──
        diag.json_parse_attempts += 1;

        let value: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => {
                diag.json_parse_failures += 1;
                continue;
            }
        };

        if let Some(ts) = extract_timestamp_from_value(&value) {
            diag.timestamps_extracted += 1;
            if first_timestamp.is_none() {
                first_timestamp = Some(ts);
            }
            last_timestamp = Some(ts);
        }

        let line_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match line_type {
            "user" => {
                // Spacing variant that SIMD missed
                diag.lines_user += 1;
                user_count += 1;
                let is_tool_result = tool_result_finder.find(line).is_some();
                if let Some(content) = extract_first_text_content(line, &content_finder, &text_finder) {
                    if first_user_content.is_none() && !is_system_user_content(&content) && !is_tool_result {
                        first_user_content = Some(content.clone());
                    }
                    last_user_content = Some(content.clone());

                    // Turn detection: real user prompts (not system prefixes, not tool_result)
                    if !is_tool_result && !is_system_user_content(&content) {
                        // Timestamp was already extracted above (before the match)
                        let current_ts = extract_timestamp_from_value(&value);
                        // Close previous turn if one was open
                        if let (Some(start_ts), Some(end_ts)) = (result.deep.current_turn_start_ts, last_timestamp) {
                            let wall_secs = (end_ts - start_ts).max(0) as u32;
                            result.deep.total_task_time_seconds += wall_secs;
                            if result.deep.longest_task_seconds.map_or(true, |prev| wall_secs > prev) {
                                result.deep.longest_task_seconds = Some(wall_secs);
                                result.deep.longest_task_preview = result.deep.current_turn_prompt.take();
                            }
                        }
                        // Start new turn
                        result.deep.current_turn_start_ts = current_ts;
                        result.deep.current_turn_prompt = Some(content.chars().take(60).collect());
                    }
                }
                extract_skills_from_line(line, &skill_name_finder, &mut result.deep.skills_used);
            }
            "assistant" => {
                // Spacing variant — fall back to Value-based extraction
                diag.lines_assistant += 1;
                assistant_count += 1;
                handle_assistant_value(
                    &value,
                    byte_offset,
                    &mut result.deep,
                    &mut result.raw_invocations,
                    &mut result.turns,
                    &mut result.models_seen,
                    diag,
                    assistant_count,
                    &mut tool_call_count,
                    &mut files_read_all,
                    &mut files_edited_all,
                );

                // Phase C: LOC estimation from Edit/Write tool_use blocks
                // SIMD pre-filter: only parse lines that are tool_use AND (Edit OR Write)
                if tool_use_finder.find(line).is_some() {
                    let is_edit = edit_finder.find(line).is_some();
                    let is_write = write_finder.find(line).is_some();
                    if is_edit || is_write {
                        let (added, removed) = extract_loc_from_tool_use(line, is_edit);
                        result.lines_added = result.lines_added.saturating_add(added);
                        result.lines_removed = result.lines_removed.saturating_add(removed);
                    }
                }
            }
            "system" => {
                diag.lines_system += 1;
                let subtype = value.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                match subtype {
                    "turn_duration" => {
                        if let Some(ms) = value.get("durationMs").and_then(|v| v.as_u64()) {
                            result.deep.turn_durations_ms.push(ms);
                        }
                    }
                    "api_error" => {
                        result.deep.api_error_count += 1;
                        if let Some(retries) = value.get("retryAttempt").and_then(|v| v.as_u64()) {
                            result.deep.api_retry_count += retries as u32;
                        }
                    }
                    "compact_boundary" | "microcompact_boundary" => {
                        result.deep.compaction_count += 1;
                    }
                    "stop_hook_summary" => {
                        if value.get("preventedContinuation").and_then(|v| v.as_bool()) == Some(true) {
                            result.deep.hook_blocked_count += 1;
                        }
                    }
                    _ => {}
                }
            }
            "progress" => {
                diag.lines_progress += 1;
                let data_type = value.get("data").and_then(|d| d.get("type")).and_then(|t| t.as_str()).unwrap_or("");
                match data_type {
                    "agent_progress" => result.deep.agent_spawn_count += 1,
                    "bash_progress" => result.deep.bash_progress_count += 1,
                    "hook_progress" => result.deep.hook_progress_count += 1,
                    "mcp_progress" => result.deep.mcp_progress_count += 1,
                    _ => {}
                }
            }
            "queue-operation" => {
                diag.lines_queue_op += 1;
                match value.get("operation").and_then(|o| o.as_str()) {
                    Some("enqueue") => result.deep.queue_enqueue_count += 1,
                    Some("dequeue") => result.deep.queue_dequeue_count += 1,
                    _ => {}
                }
            }
            "summary" => {
                diag.lines_summary += 1;
                if let Some(summary) = value.get("summary").and_then(|s| s.as_str()) {
                    result.deep.summary_text = Some(summary.to_string());
                }
            }
            "file-history-snapshot" => {
                diag.lines_file_snapshot += 1;
                result.deep.file_snapshot_count += 1;
            }
            "saved_hook_context" => {
                diag.lines_hook_context += 1;
            }
            _ => {
                diag.lines_unknown_type += 1;
            }
        }
    }

    // Close the final turn at EOF using the last known timestamp
    if let (Some(start_ts), Some(end_ts)) = (result.deep.current_turn_start_ts, last_timestamp) {
        let wall_secs = (end_ts - start_ts).max(0) as u32;
        result.deep.total_task_time_seconds += wall_secs;
        if result.deep.longest_task_seconds.map_or(true, |prev| wall_secs > prev) {
            result.deep.longest_task_seconds = Some(wall_secs);
            result.deep.longest_task_preview = result.deep.current_turn_prompt.take();
        }
    }

    // Finalize metrics
    result.deep.turn_count = (user_count as usize).min(assistant_count as usize);
    result.deep.last_message = last_user_content.map(|c| truncate(&c, 200)).unwrap_or_default();
    result.deep.first_user_prompt = first_user_content.map(|c| truncate(&c, 500));
    result.deep.user_prompt_count = user_count;
    result.deep.api_call_count = assistant_count;
    result.deep.tool_call_count = tool_call_count;

    // files_read: deduplicated
    let mut files_read_unique = files_read_all.clone();
    files_read_unique.sort();
    files_read_unique.dedup();
    result.deep.files_read_count = files_read_unique.len() as u32;
    result.deep.files_read = files_read_unique;

    // files_edited: store all occurrences
    result.deep.files_edited = files_edited_all.clone();
    let mut files_edited_unique = files_edited_all.clone();
    files_edited_unique.sort();
    files_edited_unique.dedup();
    result.deep.files_edited_count = files_edited_unique.len() as u32;
    result.deep.reedited_files_count = count_reedited_files(&files_edited_all);

    // AI contribution tracking: count lines added/removed from Edit/Write tool_use
    let ai_line_count = count_ai_lines(
        result.raw_invocations.iter().map(|inv| (inv.name.as_str(), &inv.input))
            .filter_map(|(name, input)| input.as_ref().map(|i| (name, i)))
    );
    result.deep.ai_lines_added = ai_line_count.lines_added;
    result.deep.ai_lines_removed = ai_line_count.lines_removed;

    // Duration
    result.deep.first_timestamp = first_timestamp;
    result.deep.last_timestamp = last_timestamp;
    result.deep.duration_seconds = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) if last >= first => (last - first) as u32,
        _ => 0,
    };

    // Deduplicate
    result.deep.skills_used.sort();
    result.deep.skills_used.dedup();
    result.deep.files_touched.sort();
    result.deep.files_touched.dedup();
    result.models_seen.sort();
    result.models_seen.dedup();

    result
}

// ---------------------------------------------------------------------------
// Typed handler functions for dispatched parsing
// ---------------------------------------------------------------------------

/// Handle an assistant line parsed via typed `AssistantLine` struct.
/// Produces identical output to the Value-based path.
///
/// Takes individual mutable refs to avoid borrowing `ParseResult` while `diag`
/// (which borrows `result.diagnostics`) is also live.
#[allow(clippy::too_many_arguments)]
fn handle_assistant_line(
    parsed: AssistantLine,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<vibe_recall_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: &mut u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
) {
    diag.lines_assistant += 1;
    *assistant_count += 1;

    // Extract timestamp
    let ts = parsed.timestamp.as_ref().and_then(|t| t.to_unix());
    if let Some(ts) = ts {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() {
            *first_timestamp = Some(ts);
        }
        *last_timestamp = Some(ts);
    }

    if let Some(message) = parsed.message {
        // Extract token usage
        if let Some(ref usage) = message.usage {
            if let Some(v) = usage.input_tokens {
                deep.total_input_tokens += v;
            }
            if let Some(v) = usage.output_tokens {
                deep.total_output_tokens += v;
            }
            if let Some(v) = usage.cache_read_input_tokens {
                deep.cache_read_tokens += v;
            }
            if let Some(v) = usage.cache_creation_input_tokens {
                deep.cache_creation_tokens += v;
            }
        }

        // Extract turn data (model + tokens) for turns table
        if let Some(ref model) = message.model {
            let model_id = model.clone();
            if !models_seen.contains(&model_id) {
                models_seen.push(model_id.clone());
            }

            let uuid = parsed.uuid.unwrap_or_default();
            let parent_uuid = parsed.parent_uuid;

            // Determine content_type from first block
            let content_type = match &message.content {
                ContentResult::Blocks(blocks) => blocks
                    .first()
                    .and_then(|b| b.block_type.as_deref())
                    .unwrap_or("text")
                    .to_string(),
                _ => "text".to_string(),
            };

            turns.push(vibe_recall_core::RawTurn {
                uuid,
                parent_uuid,
                seq: *assistant_count - 1,
                model_id,
                content_type,
                input_tokens: message.usage.as_ref().and_then(|u| u.input_tokens),
                output_tokens: message.usage.as_ref().and_then(|u| u.output_tokens),
                cache_read_tokens: message.usage.as_ref().and_then(|u| u.cache_read_input_tokens),
                cache_creation_tokens: message.usage.as_ref().and_then(|u| u.cache_creation_input_tokens),
                service_tier: message.usage.as_ref().and_then(|u| u.service_tier.clone()),
                timestamp: ts,
            });
            diag.turns_extracted += 1;
        }

        // Extract tool_use blocks from content
        match message.content {
            ContentResult::Blocks(blocks) => {
                for block in blocks {
                    let block_type = block.block_type.as_deref().unwrap_or("");
                    match block_type {
                        "tool_use" => {
                            diag.tool_use_blocks_found += 1;
                            let name = match block.name {
                                Some(ref n) => n.as_str(),
                                None => {
                                    diag.tool_use_missing_name += 1;
                                    continue;
                                }
                            };

                            *tool_call_count += 1;

                            match name {
                                "Read" => deep.tool_counts.read += 1,
                                "Edit" => deep.tool_counts.edit += 1,
                                "Write" => deep.tool_counts.write += 1,
                                "Bash" => deep.tool_counts.bash += 1,
                                _ => {}
                            }

                            // Extract file paths
                            if let Some(fp) = block.input.as_ref()
                                .and_then(|i| i.get("file_path"))
                                .and_then(|v| v.as_str())
                            {
                                if !fp.is_empty() {
                                    diag.file_paths_extracted += 1;
                                    deep.files_touched.push(fp.to_string());
                                    match name {
                                        "Read" => files_read_all.push(fp.to_string()),
                                        "Edit" | "Write" => files_edited_all.push(fp.to_string()),
                                        _ => {}
                                    }
                                }
                            }

                            // Store raw invocation (move input, no clone needed)
                            raw_invocations.push(RawInvocation {
                                name: name.to_string(),
                                input: block.input,
                                byte_offset,
                                timestamp: ts.unwrap_or(0),
                            });
                        }
                        "thinking" => {
                            deep.thinking_block_count += 1;
                        }
                        _ => {}
                    }
                }
            }
            ContentResult::NotArray => {
                diag.content_not_array += 1;
            }
            ContentResult::Missing => {}
        }
    }
}

/// Handle an assistant line via the fallback Value path (for spacing variants).
#[allow(clippy::too_many_arguments)]
fn handle_assistant_value(
    value: &serde_json::Value,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<vibe_recall_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
) {
    // Extract token usage from .message.usage
    if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
        if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
            deep.total_input_tokens += v;
        }
        if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
            deep.total_output_tokens += v;
        }
        if let Some(v) = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()) {
            deep.cache_read_tokens += v;
        }
        if let Some(v) = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()) {
            deep.cache_creation_tokens += v;
        }
    }

    // Extract turn data (model + tokens) for turns table
    if let Some(message) = value.get("message") {
        if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
            let model_id = model.to_string();
            if !models_seen.contains(&model_id) {
                models_seen.push(model_id.clone());
            }

            let usage = message.get("usage");
            let uuid = value.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let parent_uuid = value.get("parentUuid").and_then(|v| v.as_str()).map(|s| s.to_string());
            let content_type = message.get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|block| block.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("text")
                .to_string();
            let timestamp = extract_timestamp_from_value(value);

            turns.push(vibe_recall_core::RawTurn {
                uuid,
                parent_uuid,
                seq: assistant_count - 1,
                model_id,
                content_type,
                input_tokens: usage.and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()),
                output_tokens: usage.and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()),
                cache_read_tokens: usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()),
                cache_creation_tokens: usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()),
                service_tier: usage.and_then(|u| u.get("service_tier")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                timestamp,
            });
            diag.turns_extracted += 1;
        }
    }

    // Extract tool_use blocks from content
    let content = value.get("message").and_then(|m| m.get("content"));
    match content {
        Some(serde_json::Value::Array(arr)) => {
            for block in arr {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "tool_use" => {
                        diag.tool_use_blocks_found += 1;
                        let name = match block.get("name").and_then(|n| n.as_str()) {
                            Some(n) => n,
                            None => {
                                diag.tool_use_missing_name += 1;
                                continue;
                            }
                        };

                        *tool_call_count += 1;

                        match name {
                            "Read" => deep.tool_counts.read += 1,
                            "Edit" => deep.tool_counts.edit += 1,
                            "Write" => deep.tool_counts.write += 1,
                            "Bash" => deep.tool_counts.bash += 1,
                            _ => {}
                        }

                        let input = block.get("input");
                        if let Some(fp) = input.and_then(|i| i.get("file_path")).and_then(|v| v.as_str()) {
                            if !fp.is_empty() {
                                diag.file_paths_extracted += 1;
                                deep.files_touched.push(fp.to_string());
                                match name {
                                    "Read" => files_read_all.push(fp.to_string()),
                                    "Edit" | "Write" => files_edited_all.push(fp.to_string()),
                                    _ => {}
                                }
                            }
                        }

                        let ts = value.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
                        raw_invocations.push(RawInvocation {
                            name: name.to_string(),
                            input: input.cloned(),
                            byte_offset,
                            timestamp: ts,
                        });
                    }
                    "thinking" => {
                        deep.thinking_block_count += 1;
                    }
                    _ => {}
                }
            }
        }
        Some(serde_json::Value::String(_)) => {
            diag.content_not_array += 1;
        }
        _ => {}
    }
}

/// Handle a system line parsed via typed `SystemLine` struct.
fn handle_system_line(
    parsed: SystemLine,
    deep: &mut ExtendedMetadata,
    diag: &mut ParseDiagnostics,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
) {
    diag.lines_system += 1;

    if let Some(ts) = parsed.timestamp.as_ref().and_then(|t| t.to_unix()) {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() {
            *first_timestamp = Some(ts);
        }
        *last_timestamp = Some(ts);
    }

    let subtype = parsed.subtype.as_deref().unwrap_or("");
    match subtype {
        "turn_duration" => {
            if let Some(ms) = parsed.duration_ms {
                deep.turn_durations_ms.push(ms);
            }
        }
        "api_error" => {
            deep.api_error_count += 1;
            if let Some(retries) = parsed.retry_attempt {
                deep.api_retry_count += retries as u32;
            }
        }
        "compact_boundary" | "microcompact_boundary" => {
            deep.compaction_count += 1;
        }
        "stop_hook_summary" => {
            if parsed.prevented_continuation == Some(true) {
                deep.hook_blocked_count += 1;
            }
        }
        _ => {}
    }
}

/// Handle a summary line parsed via typed `SummaryLine` struct.
fn handle_summary_line(
    parsed: SummaryLine,
    deep: &mut ExtendedMetadata,
    diag: &mut ParseDiagnostics,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
) {
    diag.lines_summary += 1;

    if let Some(ts) = parsed.timestamp.as_ref().and_then(|t| t.to_unix()) {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() {
            *first_timestamp = Some(ts);
        }
        *last_timestamp = Some(ts);
    }

    if let Some(summary) = parsed.summary {
        deep.summary_text = Some(summary);
    }
}

/// Extract timestamp from an already-parsed JSON value.
/// Handles both ISO8601 strings and Unix integers.
fn extract_timestamp_from_value(value: &serde_json::Value) -> Option<i64> {
    value.get("timestamp").and_then(|v| {
        v.as_i64().or_else(|| {
            v.as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
        })
    })
}

/// Extract timestamp from raw JSONL bytes without JSON parsing.
/// Uses memmem::Finder to locate `"timestamp":` then parses the value inline.
fn extract_timestamp_from_bytes(line: &[u8], finder: &memmem::Finder) -> Option<i64> {
    let pos = finder.find(line)?;
    let rest = &line[pos + b"\"timestamp\":".len()..];
    // Skip whitespace
    let skip = rest.iter().position(|&b| b != b' ' && b != b'\t')?;
    let rest = &rest[skip..];
    match rest.first()? {
        b'-' | b'0'..=b'9' => {
            // Integer timestamp
            let end = rest
                .iter()
                .position(|&b| !(b == b'-' || b.is_ascii_digit()))
                .unwrap_or(rest.len());
            std::str::from_utf8(&rest[..end]).ok()?.parse().ok()
        }
        b'"' => {
            // ISO8601 string timestamp
            let s = extract_quoted_string(&rest[1..])?;
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.timestamp())
        }
        _ => None,
    }
}

/// Count files that appear 2+ times in the files_edited list.
fn count_reedited_files(files_edited: &[String]) -> u32 {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for path in files_edited {
        *counts.entry(path.as_str()).or_insert(0) += 1;
    }

    counts.values().filter(|&&count| count >= 2).count() as u32
}

/// Split data into lines using SIMD-accelerated newline search.
#[cfg(test)]
fn split_lines_simd(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let line = &data[start..end];
            start = end + 1;
            line
        })
    })
}

/// Split data into lines with byte offsets using SIMD-accelerated newline search.
/// Returns `(byte_offset, line_slice)` for each line.
fn split_lines_with_offsets(data: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut start = 0;
    let mut positions = memchr::memchr_iter(b'\n', data).chain(std::iter::once(data.len()));

    std::iter::from_fn(move || {
        if start > data.len() {
            return None;
        }
        positions.next().map(|end| {
            let offset = start;
            let line = &data[start..end];
            start = end + 1;
            (offset, line)
        })
    })
}

/// Extract the first text content from a JSONL line (best-effort, no full JSON parse).
fn extract_first_text_content(
    line: &[u8],
    content_finder: &memmem::Finder,
    text_finder: &memmem::Finder,
) -> Option<String> {
    // Look for "content":"..." pattern (simple string content)
    if let Some(pos) = content_finder.find(line) {
        let start = pos + b"\"content\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    // or "text":"..." in content blocks
    if let Some(pos) = text_finder.find(line) {
        let start = pos + b"\"text\":\"".len();
        return extract_quoted_string(&line[start..]);
    }

    None
}

/// Extract a JSON-escaped string value starting from after the opening quote.
fn extract_quoted_string(data: &[u8]) -> Option<String> {
    let mut end = 0;
    let mut escaped = false;
    for &b in data {
        if escaped {
            escaped = false;
            end += 1;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            end += 1;
            continue;
        }
        if b == b'"' {
            break;
        }
        end += 1;
    }

    if end > 0 {
        String::from_utf8(data[..end].to_vec()).ok()
    } else {
        None
    }
}

/// Returns true if the extracted user message content looks like a system/hook message
/// rather than a real user prompt. These include local-command caveats, slash-command
/// wrappers, tool_result blocks, and empty/whitespace-only content.
fn is_system_user_content(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.is_empty()
        || trimmed.starts_with("<local-command-caveat>")
        || trimmed.starts_with("<command-name>")
        || trimmed.starts_with("<command-message>")
        || trimmed.starts_with("<local-command-stdout>")
        || trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("{\"type\":\"tool_result\"")
}

/// Extract skill names from a user message line (looking for "skill":"..." patterns).
fn extract_skills_from_line(
    line: &[u8],
    skill_name_finder: &memmem::Finder,
    skills: &mut Vec<String>,
) {
    let mut start = 0;
    while start < line.len() {
        if let Some(pos) = skill_name_finder.find(&line[start..]) {
            let begin = start + pos + b"\"skill\":\"".len();
            if let Some(name) = extract_quoted_string(&line[begin..]) {
                if !name.is_empty() {
                    skills.push(name);
                }
            }
            start = begin;
        } else {
            break;
        }
    }
}

/// Truncate a string to at most `max_len` characters (not bytes).
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

/// Pass 1: Read sessions-index.json files and insert/update sessions in DB.
///
/// This is extremely fast (<10ms) because it reads pre-computed JSON indexes
/// that Claude Code maintains. No JSONL parsing is needed.
///
/// Worktree consolidation: sessions from worktree project directories
/// (e.g. `project--worktrees-branch`) are reparented under the main project
/// so they appear together in the UI.
///
/// Orphan discovery: project directories that lack a `sessions-index.json`
/// (common for worktree dirs) are scanned for `.jsonl` files and included.
///
/// Returns `(num_projects, num_sessions)`.
pub async fn pass_1_read_indexes(
    claude_dir: &Path,
    db: &Database,
) -> Result<(usize, usize), String> {
    let all_indexes = read_all_session_indexes(claude_dir).map_err(|e| e.to_string())?;

    let mut total_projects = 0usize;
    let mut total_sessions = 0usize;

    // Helper: insert sessions for one project (handles worktree consolidation)
    async fn insert_project_sessions(
        claude_dir: &Path,
        db: &Database,
        project_encoded: &str,
        entries: &[vibe_recall_core::SessionIndexEntry],
        total_sessions: &mut usize,
    ) -> Result<(), String> {
        // Worktree consolidation — reparent under the main project
        let (effective_encoded, effective_resolved) =
            if let Some(parent_encoded) = resolve_worktree_parent(project_encoded) {
                let resolved = resolve_project_path(&parent_encoded);
                (parent_encoded, resolved)
            } else {
                (
                    project_encoded.to_string(),
                    resolve_project_path(project_encoded),
                )
            };

        let project_display_name = &effective_resolved.display_name;
        let project_path = &effective_resolved.full_path;

        for entry in entries {
            // Parse modified ISO string to unix timestamp
            let modified_at = entry
                .modified
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            // Derive file_path from full_path or construct from session_id
            let file_path = entry.full_path.clone().unwrap_or_else(|| {
                claude_dir
                    .join("projects")
                    .join(project_encoded)
                    .join(format!("{}.jsonl", &entry.session_id))
                    .to_string_lossy()
                    .to_string()
            });

            // Stat the file for size_bytes if the path exists
            let size_bytes = std::fs::metadata(&file_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let preview = entry.first_prompt.as_deref().unwrap_or("");
            let summary = entry.summary.as_deref();
            let message_count = entry.message_count.unwrap_or(0) as i32;
            let git_branch = entry.git_branch.as_deref();
            let is_sidechain = entry.is_sidechain.unwrap_or(false);

            // Use project_path from the entry if available, else from resolved path
            let entry_project_path = entry.project_path.as_deref().unwrap_or(project_path);

            db.insert_session_from_index(
                &entry.session_id,
                &effective_encoded,
                project_display_name,
                entry_project_path,
                &file_path,
                preview,
                summary,
                message_count,
                modified_at,
                git_branch,
                is_sidechain,
                size_bytes,
            )
            .await
            .map_err(|e| format!("Failed to insert session {}: {}", entry.session_id, e))?;

            *total_sessions += 1;
        }

        Ok(())
    }

    // Loop 1: Sessions from sessions-index.json files
    for (project_encoded, entries) in &all_indexes {
        if entries.is_empty() {
            continue;
        }
        total_projects += 1;
        insert_project_sessions(claude_dir, db, project_encoded, entries, &mut total_sessions)
            .await?;
    }

    // Loop 2: Orphan sessions from dirs without sessions-index.json
    let orphans = discover_orphan_sessions(claude_dir).map_err(|e| e.to_string())?;

    for (project_encoded, entries) in &orphans {
        if entries.is_empty() {
            continue;
        }
        total_projects += 1;
        insert_project_sessions(
            claude_dir,
            db,
            project_encoded,
            entries,
            &mut total_sessions,
        )
        .await?;
    }

    Ok((total_projects, total_sessions))
}

/// Pass 2: Parallel deep JSONL parsing for extended metadata.
///
/// Uses a collect-then-write pattern:
/// 1. **Parse phase** (parallel): Each session is parsed in a tokio task with
///    semaphore-bounded parallelism. Tasks return `DeepIndexResult` structs
///    instead of writing to the DB. The `on_file_done` callback fires per
///    session during this phase so the TUI progress spinner still works.
/// 2. **Write phase** (sequential): After all parse tasks complete, one
///    transaction writes all results. This eliminates ~N*3 implicit fsyncs
///    (one per session per write call) and replaces them with a single COMMIT.
///
/// Uses zero-copy mmap: the file is memory-mapped and `parse_bytes` runs
/// directly on the mapped pages. The mmap stays alive for the duration of
/// parsing, then drops — no heap copy is ever made.
///
/// If `registry` is `Some`, raw tool_use invocations are classified during the
/// parse phase and included in the result struct for batch insertion.
///
/// Calls `on_file_done(indexed_so_far, total, file_bytes)` after each file's parse completes.
/// Calls `on_start(total_bytes)` once before processing begins with the total size
/// of all JSONL files to be parsed.
///
/// Returns `(indexed_count, total_bytes)` on success, where `total_bytes` is the
/// sum of all JSONL file sizes that were eligible for deep indexing.
pub async fn pass_2_deep_index<F>(
    db: &Database,
    registry: Option<&Registry>,
    on_start: impl FnOnce(u64),
    on_file_done: F,
) -> Result<(usize, u64), String>
where
    F: Fn(usize, usize, u64) + Send + Sync + 'static,
{
    let all_sessions = db
        .get_sessions_needing_deep_index()
        .await
        .map_err(|e| format!("Failed to query sessions needing deep index: {}", e))?;

    // Filter sessions that actually need (re-)indexing:
    // 1. deep_indexed_at IS NULL → new session, never indexed
    // 2. parse_version < CURRENT → parser upgraded
    // 3. file_size_at_index or file_mtime_at_index is NULL → never had metadata stored
    // 4. Otherwise: stat() the file, compare size+mtime. Different → re-index.
    let all_sessions_count = all_sessions.len();
    let sessions: Vec<(String, String)> = all_sessions
        .into_iter()
        .filter_map(|(id, file_path, stored_size, stored_mtime, deep_indexed_at, parse_version)| {
            let needs_index = if deep_indexed_at.is_none() {
                // Never deep-indexed → must index
                true
            } else if parse_version < CURRENT_PARSE_VERSION {
                // Parser upgraded → must re-index
                true
            } else if let (Some(sz), Some(mt)) = (stored_size, stored_mtime) {
                // Has stored metadata → stat the file and compare size + mtime
                match std::fs::metadata(&file_path) {
                    Ok(meta) => {
                        let current_size = meta.len() as i64;
                        let current_mtime = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        current_size != sz || current_mtime != mt
                    }
                    Err(_) => false, // File doesn't exist or can't be read, skip
                }
            } else {
                // No stored file metadata → must re-index to populate it
                true
            };
            if needs_index {
                Some((id, file_path))
            } else {
                None
            }
        })
        .collect();

    let phase_start = std::time::Instant::now();
    let skipped = all_sessions_count - sessions.len();

    if sessions.is_empty() {
        return Ok((0, 0));
    }

    // Pre-compute total bytes of all JSONL files to process.
    let total_bytes: u64 = sessions
        .iter()
        .filter_map(|(_, path)| std::fs::metadata(path).ok())
        .map(|m| m.len())
        .sum();

    on_start(total_bytes);

    // Clone registry into an Arc so we can share across spawned tasks.
    let registry: Option<Arc<Registry>> = registry.map(|r| Arc::new(r.clone()));

    let total = sessions.len();
    let counter = Arc::new(AtomicUsize::new(0));
    let on_file_done = Arc::new(on_file_done);

    // Limit parallelism to available CPU cores
    let parallelism = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(parallelism));

    // ── Parse phase (parallel) ──────────────────────────────────────────
    // Each task returns Option<DeepIndexResult>. None means the file was
    // missing or empty (nothing to write).
    let mut handles = Vec::with_capacity(total);

    for (id, file_path) in sessions {
        let sem = semaphore.clone();
        let counter = counter.clone();
        let on_done = on_file_done.clone();
        let registry = registry.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|e| format!("Semaphore error: {}", e))?;

            let path = std::path::PathBuf::from(&file_path);

            // Zero-copy mmap + parse in a blocking thread.
            // The mmap stays alive while parse_bytes runs on the mapped pages,
            // then drops after parsing — no heap allocation for the file content.
            // Returns (ParseResult, file_size, file_mtime) so we can store metadata
            // for change detection on next startup.
            let (parse_result, file_size, file_mtime) = tokio::task::spawn_blocking(move || {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return (ParseResult::default(), 0i64, 0i64),
                };
                let metadata = match file.metadata() {
                    Ok(m) => m,
                    Err(_) => return (ParseResult::default(), 0, 0),
                };
                let fsize = metadata.len() as i64;
                let fmtime = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let len = metadata.len() as usize;
                if len == 0 {
                    return (ParseResult::default(), fsize, fmtime);
                }
                // Small files: regular read (mmap overhead not worth it)
                if len < 64 * 1024 {
                    match std::fs::read(&path) {
                        Ok(data) => return (parse_bytes(&data), fsize, fmtime),
                        Err(_) => return (ParseResult::default(), fsize, fmtime),
                    }
                }
                // Large files: zero-copy mmap — parse directly from mapped pages
                // SAFETY: Read-only mapping. Claude Code appends to JSONL (never truncates).
                match unsafe { memmap2::Mmap::map(&file) } {
                    Ok(mmap) => (parse_bytes(&mmap), fsize, fmtime), // zero-copy! mmap drops after parse
                    Err(_) => match std::fs::read(&path) {
                        Ok(data) => (parse_bytes(&data), fsize, fmtime),
                        Err(_) => (ParseResult::default(), fsize, fmtime),
                    },
                }
            })
            .await
            .map_err(|e| format!("spawn_blocking join error: {}", e))?;

            let diag = &parse_result.diagnostics;

            // Log parse anomalies per session
            if diag.json_parse_failures > 0 || diag.lines_unknown_type > 0 {
                tracing::warn!(
                    session_id = %id,
                    parse_failures = diag.json_parse_failures,
                    unknown_types = diag.lines_unknown_type,
                    total_lines = diag.lines_total,
                    "Parse anomalies detected"
                );
            }

            // Classify raw invocations during parse phase (CPU work, no DB)
            let classified = if let Some(ref registry) = registry {
                parse_result
                    .raw_invocations
                    .iter()
                    .filter_map(|raw| {
                        let result = vibe_recall_core::classify_tool_use(
                            &raw.name,
                            &raw.input,
                            registry,
                        );
                        match result {
                            ClassifyResult::Valid { invocable_id, .. } => Some((
                                file_path.clone(),
                                raw.byte_offset as i64,
                                invocable_id,
                                id.clone(),
                                String::new(), // project filled by caller if needed
                                raw.timestamp,
                            )),
                            _ => None, // Rejected and Ignored are discarded
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            // Fire progress callback during parse phase so TUI spinner works
            let indexed = counter.fetch_add(1, Ordering::Relaxed) + 1;
            on_done(indexed, total, file_size.max(0) as u64);

            Ok::<Option<DeepIndexResult>, String>(Some(DeepIndexResult {
                session_id: id,
                file_path,
                parse_result,
                classified_invocations: classified,
                file_size,
                file_mtime,
            }))
        });

        handles.push(handle);
    }

    // Collect all parse results
    let mut results = Vec::with_capacity(total);
    let mut parse_errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(result))) => results.push(result),
            Ok(Ok(None)) => {} // file missing or empty, skip
            Ok(Err(e)) => parse_errors.push(e),
            Err(e) => parse_errors.push(format!("Task join error: {}", e)),
        }
    }

    let parse_elapsed = phase_start.elapsed();

    if !parse_errors.is_empty() {
        tracing::warn!(
            "pass_2_deep_index parse phase encountered {} errors: {:?}",
            parse_errors.len(),
            parse_errors
        );
    }

    // ── Write phase ─────────────────────────────────────────────────────
    // Two paths:
    //   1. File-based DB (production): synchronous rusqlite with prepared
    //      statements in spawn_blocking — avoids per-row async overhead.
    //   2. In-memory DB (tests): sqlx _tx functions via async transaction,
    //      because rusqlite can't open a separate connection to sqlx's
    //      in-memory database.
    let use_rusqlite = !db.db_path().as_os_str().is_empty();
    let indexed = if !results.is_empty() {
        if use_rusqlite {
            // ── Fast path: rusqlite with prepared statements ──────────
            let db_path = db.db_path().to_owned();
            let write_result: Result<usize, String> = tokio::task::spawn_blocking(move || {
                let conn = rusqlite::Connection::open(&db_path)
                    .map_err(|e| format!("rusqlite open error: {}", e))?;
                conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=30000;")
                    .map_err(|e| format!("rusqlite pragma error: {}", e))?;

                let mut update_stmt = conn.prepare(UPDATE_SESSION_DEEP_SQL)
                    .map_err(|e| format!("prepare UPDATE error: {}", e))?;
                let mut turn_stmt = conn.prepare(INSERT_TURN_SQL)
                    .map_err(|e| format!("prepare INSERT_TURN error: {}", e))?;
                let mut inv_stmt = conn.prepare(INSERT_INVOCATION_SQL)
                    .map_err(|e| format!("prepare INSERT_INVOC error: {}", e))?;
                let mut model_stmt = conn.prepare(UPSERT_MODEL_SQL)
                    .map_err(|e| format!("prepare UPSERT_MODEL error: {}", e))?;
                let mut del_turns_stmt = conn.prepare("DELETE FROM turns WHERE session_id = ?1")
                    .map_err(|e| format!("prepare DELETE turns error: {}", e))?;
                let mut del_inv_stmt = conn.prepare("DELETE FROM invocations WHERE session_id = ?1")
                    .map_err(|e| format!("prepare DELETE invocations error: {}", e))?;

                let tx = conn.unchecked_transaction()
                    .map_err(|e| format!("rusqlite begin error: {}", e))?;

                let seen_at = chrono::Utc::now().timestamp();
                let deep_indexed_at = seen_at;
                let count = results.len();

                for result in &results {
                    let meta = &result.parse_result.deep;

                    // Serialize vec fields to JSON strings
                    let files_touched = serde_json::to_string(&meta.files_touched)
                        .unwrap_or_else(|_| "[]".to_string());
                    let skills_used = serde_json::to_string(&meta.skills_used)
                        .unwrap_or_else(|_| "[]".to_string());
                    let files_read = serde_json::to_string(&meta.files_read)
                        .unwrap_or_else(|_| "[]".to_string());
                    let files_edited = serde_json::to_string(&meta.files_edited)
                        .unwrap_or_else(|_| "[]".to_string());

                    // Count commit skills for commit_count
                    let commit_invocations =
                        extract_commit_skill_invocations(&result.parse_result.raw_invocations);
                    let commit_count = commit_invocations.len() as i32;

                    // Compute turn duration aggregates
                    let (dur_avg, dur_max, dur_total) = if meta.turn_durations_ms.is_empty() {
                        (None, None, None)
                    } else {
                        let total: u64 = meta.turn_durations_ms.iter().sum();
                        let max = *meta.turn_durations_ms.iter().max().unwrap();
                        let avg = total / meta.turn_durations_ms.len() as u64;
                        (Some(avg as i64), Some(max as i64), Some(total as i64))
                    };

                    // Classify work type
                    let work_type_input = ClassificationInput::new(
                        meta.duration_seconds,
                        meta.turn_count as u32,
                        meta.files_edited_count,
                        meta.ai_lines_added,
                        meta.skills_used.clone(),
                    );
                    let work_type = classify_work_type(&work_type_input);

                    let primary_model = compute_primary_model(&result.parse_result.turns);

                    // UPDATE session deep fields (54 params: ?1=id, ?2-?54=fields)
                    update_stmt.execute(rusqlite::params![
                        result.session_id,                // ?1
                        meta.last_message,                // ?2
                        meta.turn_count as i32,           // ?3
                        meta.tool_counts.edit as i32,     // ?4
                        meta.tool_counts.read as i32,     // ?5
                        meta.tool_counts.bash as i32,     // ?6
                        meta.tool_counts.write as i32,    // ?7
                        files_touched,                    // ?8
                        skills_used,                      // ?9
                        deep_indexed_at,                  // ?10
                        meta.user_prompt_count as i32,    // ?11
                        meta.api_call_count as i32,       // ?12
                        meta.tool_call_count as i32,      // ?13
                        files_read,                       // ?14
                        files_edited,                     // ?15
                        meta.files_read_count as i32,     // ?16
                        meta.files_edited_count as i32,   // ?17
                        meta.reedited_files_count as i32, // ?18
                        meta.duration_seconds as i32,     // ?19
                        commit_count,                     // ?20
                        meta.first_timestamp,             // ?21 (Option<i64>)
                        meta.total_input_tokens as i64,   // ?22
                        meta.total_output_tokens as i64,  // ?23
                        meta.cache_read_tokens as i64,    // ?24
                        meta.cache_creation_tokens as i64, // ?25
                        meta.thinking_block_count as i32, // ?26
                        dur_avg,                          // ?27 (Option<i64>)
                        dur_max,                          // ?28 (Option<i64>)
                        dur_total,                        // ?29 (Option<i64>)
                        meta.api_error_count as i32,      // ?30
                        meta.api_retry_count as i32,      // ?31
                        meta.compaction_count as i32,     // ?32
                        meta.hook_blocked_count as i32,   // ?33
                        meta.agent_spawn_count as i32,    // ?34
                        meta.bash_progress_count as i32,  // ?35
                        meta.hook_progress_count as i32,  // ?36
                        meta.mcp_progress_count as i32,   // ?37
                        meta.summary_text,                // ?38 (Option<String>)
                        CURRENT_PARSE_VERSION,            // ?39
                        result.file_size,                 // ?40
                        result.file_mtime,                // ?41
                        result.parse_result.lines_added as i32,   // ?42
                        result.parse_result.lines_removed as i32, // ?43
                        1_i32,                            // ?44 loc_source = 1 (tool-call estimate)
                        meta.ai_lines_added as i32,       // ?45
                        meta.ai_lines_removed as i32,     // ?46
                        work_type.as_str(),               // ?47
                        result.parse_result.git_branch.as_deref(),  // ?48
                        primary_model,                    // ?49 (Option<String>)
                        meta.last_timestamp,              // ?50 (Option<i64>)
                        meta.first_user_prompt,           // ?51 (Option<String>)
                        meta.total_task_time_seconds as i32,       // ?52
                        meta.longest_task_seconds.map(|v| v as i32), // ?53 (Option<i32>)
                        meta.longest_task_preview,        // ?54 (Option<String>)
                    ]).map_err(|e| format!("UPDATE session {} error: {}", result.session_id, e))?;

                    // DELETE stale turns/invocations before re-inserting
                    del_turns_stmt.execute(rusqlite::params![result.session_id])
                        .map_err(|e| format!("DELETE turns for {} error: {}", result.session_id, e))?;
                    del_inv_stmt.execute(rusqlite::params![result.session_id])
                        .map_err(|e| format!("DELETE invocations for {} error: {}", result.session_id, e))?;

                    // INSERT invocations
                    for (source_file, byte_offset, invocable_id, session_id, project, timestamp) in &result.classified_invocations {
                        inv_stmt.execute(rusqlite::params![
                            source_file, byte_offset, invocable_id, session_id, project, timestamp
                        ]).map_err(|e| format!("INSERT invocation error: {}", e))?;
                    }

                    // UPSERT models + INSERT turns
                    if !result.parse_result.turns.is_empty() {
                        for model_id in &result.parse_result.models_seen {
                            let (provider, family) = vibe_recall_core::parse_model_id(model_id);
                            model_stmt.execute(rusqlite::params![
                                model_id, provider, family, seen_at, seen_at
                            ]).map_err(|e| format!("UPSERT model error: {}", e))?;
                        }

                        for turn in &result.parse_result.turns {
                            turn_stmt.execute(rusqlite::params![
                                result.session_id,
                                turn.uuid,
                                turn.seq,
                                turn.model_id,
                                turn.parent_uuid,
                                turn.content_type,
                                turn.input_tokens.map(|v| v as i64),
                                turn.output_tokens.map(|v| v as i64),
                                turn.cache_read_tokens.map(|v| v as i64),
                                turn.cache_creation_tokens.map(|v| v as i64),
                                turn.service_tier,
                                turn.timestamp,
                            ]).map_err(|e| format!("INSERT turn error: {}", e))?;
                        }
                    }
                }

                tx.commit().map_err(|e| format!("rusqlite commit error: {}", e))?;
                Ok(count)
            })
            .await
            .map_err(|e| format!("spawn_blocking join error: {}", e))?;

            write_result?
        } else {
            // ── Fallback path: sqlx async transaction (in-memory DBs) ──
            write_results_sqlx(db, &results).await?
        }
    } else {
        0
    };

    let write_elapsed = phase_start.elapsed() - parse_elapsed;

    if !parse_errors.is_empty() {
        tracing::warn!(
            "pass_2_deep_index encountered {} parse errors (still wrote {} results)",
            parse_errors.len(),
            indexed
        );
    }

    tracing::info!(
        sessions_indexed = indexed,
        sessions_total = total,
        parse_version = CURRENT_PARSE_VERSION,
        "Pass 2 deep indexing complete"
    );

    {
        let total_elapsed = phase_start.elapsed();
        eprintln!(
            "    [perf] deep index: {} indexed, {} skipped, {} errors",
            indexed,
            skipped,
            parse_errors.len()
        );
        eprintln!(
            "    [perf]   parse phase: {}",
            vibe_recall_core::format_duration(parse_elapsed)
        );
        eprintln!(
            "    [perf]   write phase: {}",
            vibe_recall_core::format_duration(write_elapsed)
        );
        eprintln!(
            "    [perf]   total:       {}",
            vibe_recall_core::format_duration(total_elapsed)
        );
    }

    Ok((indexed, total_bytes))
}

/// Fallback write path using sqlx async transactions.
/// Used for in-memory databases (tests) where rusqlite can't open a separate connection.
async fn write_results_sqlx(
    db: &Database,
    results: &[DeepIndexResult],
) -> Result<usize, String> {
    let mut tx = db
        .pool()
        .begin()
        .await
        .map_err(|e| format!("Failed to begin write transaction: {}", e))?;

    let seen_at = chrono::Utc::now().timestamp();

    for result in results {
        let meta = &result.parse_result.deep;

        let files_touched = serde_json::to_string(&meta.files_touched)
            .unwrap_or_else(|_| "[]".to_string());
        let skills_used = serde_json::to_string(&meta.skills_used)
            .unwrap_or_else(|_| "[]".to_string());
        let files_read = serde_json::to_string(&meta.files_read)
            .unwrap_or_else(|_| "[]".to_string());
        let files_edited = serde_json::to_string(&meta.files_edited)
            .unwrap_or_else(|_| "[]".to_string());

        let commit_invocations =
            extract_commit_skill_invocations(&result.parse_result.raw_invocations);
        let commit_count = commit_invocations.len() as i32;

        let (dur_avg, dur_max, dur_total) = if meta.turn_durations_ms.is_empty() {
            (None, None, None)
        } else {
            let total: u64 = meta.turn_durations_ms.iter().sum();
            let max = *meta.turn_durations_ms.iter().max().unwrap();
            let avg = total / meta.turn_durations_ms.len() as u64;
            (Some(avg as i64), Some(max as i64), Some(total as i64))
        };

        // Classify work type for Theme 3
        let work_type_input = ClassificationInput::new(
            meta.duration_seconds,
            meta.turn_count as u32,
            meta.files_edited_count,
            meta.ai_lines_added,
            meta.skills_used.clone(),
        );
        let work_type = classify_work_type(&work_type_input);

        let primary_model = compute_primary_model(&result.parse_result.turns);

        crate::queries::update_session_deep_fields_tx(
            &mut tx,
            &result.session_id,
            &meta.last_message,
            meta.turn_count as i32,
            meta.tool_counts.edit as i32,
            meta.tool_counts.read as i32,
            meta.tool_counts.bash as i32,
            meta.tool_counts.write as i32,
            &files_touched,
            &skills_used,
            meta.user_prompt_count as i32,
            meta.api_call_count as i32,
            meta.tool_call_count as i32,
            &files_read,
            &files_edited,
            meta.files_read_count as i32,
            meta.files_edited_count as i32,
            meta.reedited_files_count as i32,
            meta.duration_seconds as i32,
            commit_count,
            meta.first_timestamp,
            meta.total_input_tokens as i64,
            meta.total_output_tokens as i64,
            meta.cache_read_tokens as i64,
            meta.cache_creation_tokens as i64,
            meta.thinking_block_count as i32,
            dur_avg,
            dur_max,
            dur_total,
            meta.api_error_count as i32,
            meta.api_retry_count as i32,
            meta.compaction_count as i32,
            meta.hook_blocked_count as i32,
            meta.agent_spawn_count as i32,
            meta.bash_progress_count as i32,
            meta.hook_progress_count as i32,
            meta.mcp_progress_count as i32,
            meta.summary_text.as_deref(),
            CURRENT_PARSE_VERSION,
            result.file_size,
            result.file_mtime,
            result.parse_result.lines_added as i32,
            result.parse_result.lines_removed as i32,
            1, // loc_source = 1 (tool-call estimate)
            meta.ai_lines_added as i32,
            meta.ai_lines_removed as i32,
            Some(work_type.as_str()),
            result.parse_result.git_branch.as_deref(),
            primary_model.as_deref(),
            meta.last_timestamp,
            meta.first_user_prompt.as_deref(),
            meta.total_task_time_seconds as i32,
            meta.longest_task_seconds.map(|v| v as i32),
            meta.longest_task_preview.as_deref(),
        )
        .await
        .map_err(|e| {
            format!(
                "Failed to update deep fields for {}: {}",
                result.session_id, e
            )
        })?;

        // DELETE stale turns/invocations before re-inserting
        sqlx::query("DELETE FROM turns WHERE session_id = ?1")
            .bind(&result.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("DELETE turns for {} error: {}", result.session_id, e))?;

        sqlx::query("DELETE FROM invocations WHERE session_id = ?1")
            .bind(&result.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("DELETE invocations for {} error: {}", result.session_id, e))?;

        if !result.classified_invocations.is_empty() {
            crate::queries::batch_insert_invocations_tx(
                &mut tx,
                &result.classified_invocations,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to insert invocations for {}: {}",
                    result.session_id, e
                )
            })?;
        }

        if !result.parse_result.turns.is_empty() {
            crate::queries::batch_upsert_models_tx(
                &mut tx,
                &result.parse_result.models_seen,
                seen_at,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to upsert models for {}: {}",
                    result.session_id, e
                )
            })?;

            crate::queries::batch_insert_turns_tx(
                &mut tx,
                &result.session_id,
                &result.parse_result.turns,
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to insert turns for {}: {}",
                    result.session_id, e
                )
            })?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit write transaction: {}", e))?;

    Ok(results.len())
}

/// Type alias for the shared registry holder used by the server.
pub type RegistryHolder = Arc<RwLock<Option<Registry>>>;

/// Full background indexing orchestrator: Pass 1 (index JSON) + Registry build
/// in parallel, then Pass 2 (deep JSONL) with registry available.
///
/// This is the main entry point for background indexing. It runs Pass 1 and
/// registry construction concurrently via `tokio::join!`, then Pass 2
/// sequentially with the registry available for invocation classification.
///
/// If `registry_holder` is provided, the built registry is stored in it so
/// API routes can access it after indexing completes.

/// Prune sessions from the database whose JSONL files no longer exist on disk.
///
/// Queries all session file_paths from the DB, checks each one for existence
/// on the filesystem, then deletes sessions whose files are gone. This handles
/// the case where Claude Code deletes a session file but the DB still has a
/// record for it.
///
/// Uses a batch transaction for the delete (via `remove_stale_sessions`).
/// Returns the number of pruned sessions.
pub async fn prune_stale_sessions(db: &Database) -> Result<u64, String> {
    let all_paths = db
        .get_all_session_file_paths()
        .await
        .map_err(|e| format!("Failed to query session file paths: {}", e))?;

    if all_paths.is_empty() {
        return Ok(0);
    }

    // Check which files still exist on disk (blocking I/O, but just stat calls)
    let valid_paths: Vec<String> = all_paths
        .into_iter()
        .filter(|path| Path::new(path).exists())
        .collect();

    let pruned = db
        .remove_stale_sessions(&valid_paths)
        .await
        .map_err(|e| format!("Failed to prune stale sessions: {}", e))?;

    if pruned > 0 {
        tracing::info!(
            "Pruned {} stale sessions (JSONL files deleted from disk)",
            pruned
        );
    }

    Ok(pruned)
}

pub async fn run_background_index<F>(
    claude_dir: &Path,
    db: &Database,
    registry_holder: Option<RegistryHolder>,
    on_pass1_done: impl FnOnce(usize, usize),
    on_pass2_start: impl FnOnce(u64),
    on_file_done: F,
    on_complete: impl FnOnce(usize),
) -> Result<(), String>
where
    F: Fn(usize, usize, u64) + Send + Sync + 'static,
{
    // Pass 1 and Registry build are independent — run in parallel.
    let claude_dir_owned = claude_dir.to_path_buf();
    let (pass1_result, registry) = tokio::join!(
        pass_1_read_indexes(claude_dir, db),
        vibe_recall_core::build_registry(&claude_dir_owned),
    );

    let (projects, sessions) = pass1_result?;
    on_pass1_done(projects, sessions);

    // Seed invocables into the DB so invocations can reference them (FK constraint).
    let invocable_tuples: Vec<(String, Option<String>, String, String, String)> = registry
        .all_invocables()
        .map(|info| {
            (
                info.id.clone(),
                info.plugin_name.clone(),
                info.name.clone(),
                info.kind.to_string(),
                info.description.clone(),
            )
        })
        .collect();
    if !invocable_tuples.is_empty() {
        db.batch_upsert_invocables(&invocable_tuples)
            .await
            .map_err(|e| format!("Failed to seed invocables: {}", e))?;
    }

    // Pass 2: use the registry for invocation classification
    let (indexed, _total_bytes) = pass_2_deep_index(db, Some(&registry), on_pass2_start, on_file_done).await?;

    // Backfill primary_model for sessions indexed before this field was populated
    match db.backfill_primary_models().await {
        Ok(n) if n > 0 => tracing::info!("Backfilled primary_model for {} sessions", n),
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to backfill primary_models: {}", e),
    }

    // Prune sessions whose JSONL files have been deleted from disk.
    // This runs after indexing so we don't accidentally prune sessions that
    // were just discovered by pass_1. Non-fatal: log and continue on error.
    match prune_stale_sessions(db).await {
        Ok(n) if n > 0 => tracing::info!("Pruned {} stale sessions from DB", n),
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to prune stale sessions: {}", e),
    }

    // Store registry in shared holder for API routes to use
    if let Some(holder) = registry_holder {
        if let Ok(mut guard) = holder.write() {
            *guard = Some(registry);
        }
    }

    on_complete(indexed);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_parse_bytes_empty() {
        let result = parse_bytes(b"");
        assert_eq!(result.deep.turn_count, 0);
        assert!(result.deep.last_message.is_empty());
        assert!(result.deep.tool_counts.is_empty());
        assert!(result.raw_invocations.is_empty());
    }

    #[test]
    fn test_parse_bytes_counts_tools() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}}
{"type":"user","message":{"content":"thanks"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.turn_count, 2);
        assert_eq!(result.deep.tool_counts.read, 1);
        assert_eq!(result.deep.tool_counts.edit, 1);
        assert_eq!(result.deep.tool_counts.bash, 1);
        assert_eq!(result.deep.tool_counts.write, 0);
    }

    #[test]
    fn test_parse_bytes_last_message() {
        let data = br#"{"type":"user","message":{"content":"first question"}}
{"type":"assistant","message":{"content":"answer 1"}}
{"type":"user","message":{"content":"second question"}}
{"type":"assistant","message":{"content":"answer 2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.last_message, "second question");
    }

    #[test]
    fn test_parse_bytes_extracts_raw_invocations() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","timestamp":1706200000,"message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
{"type":"user","message":{"content":"run tests"}}
{"type":"assistant","timestamp":1706200100,"message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}},{"type":"text","text":"Done!"}]}}
"#;
        let result = parse_bytes(data);

        // Should extract 3 raw invocations: Read, Edit, Bash
        assert_eq!(
            result.raw_invocations.len(),
            3,
            "Should extract 3 tool_use invocations"
        );

        // First line (user) is at offset 0, second line (assistant with Read+Edit) starts after first newline
        assert_eq!(result.raw_invocations[0].name, "Read");
        assert_eq!(
            result.raw_invocations[0]
                .input
                .as_ref()
                .and_then(|v| v.get("file_path"))
                .and_then(|v| v.as_str()),
            Some("/src/main.rs")
        );
        assert_eq!(result.raw_invocations[0].timestamp, 1706200000);

        assert_eq!(result.raw_invocations[1].name, "Edit");
        assert_eq!(result.raw_invocations[1].timestamp, 1706200000);

        assert_eq!(result.raw_invocations[2].name, "Bash");
        assert_eq!(
            result.raw_invocations[2]
                .input
                .as_ref()
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str()),
            Some("cargo test")
        );
        assert_eq!(result.raw_invocations[2].timestamp, 1706200100);

        // Byte offsets: first two invocations share the same line offset,
        // third invocation is on a different line
        assert_eq!(
            result.raw_invocations[0].byte_offset,
            result.raw_invocations[1].byte_offset,
            "Read and Edit are on the same JSONL line"
        );
        assert_ne!(
            result.raw_invocations[0].byte_offset,
            result.raw_invocations[2].byte_offset,
            "Bash is on a different JSONL line"
        );
    }

    #[test]
    fn test_parse_bytes_no_invocations_without_tool_use() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":"Just text, no tools."}}
"#;
        let result = parse_bytes(data);
        assert!(
            result.raw_invocations.is_empty(),
            "No tool_use blocks, no invocations"
        );
    }

    #[test]
    fn test_parse_bytes_timestamp_defaults_to_zero() {
        // No timestamp field on the assistant message
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.raw_invocations.len(), 1);
        assert_eq!(
            result.raw_invocations[0].timestamp, 0,
            "Missing timestamp should default to 0"
        );
    }

    #[test]
    fn test_read_file_fast_nonexistent() {
        let result = read_file_fast(std::path::Path::new("/nonexistent/file.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_fast_small_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world").unwrap();
        let data = read_file_fast(tmp.path()).unwrap();
        assert_eq!(&*data, b"hello world");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_split_lines_simd() {
        let data = b"line1\nline2\nline3";
        let lines: Vec<&[u8]> = split_lines_simd(data).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], b"line1");
        assert_eq!(lines[1], b"line2");
        assert_eq!(lines[2], b"line3");
    }

    // ============================================================================
    // Phase 3: Atomic Unit Extraction Tests (Steps 3-9)
    // ============================================================================

    // --- Step 3: user_prompt_count (A1.1) ---

    #[test]
    fn test_user_prompt_count_basic() {
        // Test case 1: 5 user messages, 10 assistant → 5
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"a2"}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":"a3"}}
{"type":"user","message":{"content":"q4"}}
{"type":"assistant","message":{"content":"a4"}}
{"type":"user","message":{"content":"q5"}}
{"type":"assistant","message":{"content":"a5"}}
{"type":"assistant","message":{"content":"a6"}}
{"type":"assistant","message":{"content":"a7"}}
{"type":"assistant","message":{"content":"a8"}}
{"type":"assistant","message":{"content":"a9"}}
{"type":"assistant","message":{"content":"a10"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 5, "Should count 5 user messages");
    }

    #[test]
    fn test_user_prompt_count_zero() {
        // Test case 2: 0 user messages → 0
        let data = br#"{"type":"assistant","message":{"content":"a1"}}
{"type":"assistant","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 0, "Should be 0 with no user messages");
    }

    #[test]
    fn test_user_prompt_count_unicode() {
        // Test case 3: Unicode/emoji content doesn't affect type detection
        let data = br#"{"type":"user","message":{"content":"Hello \u4e16\u754c"}}
{"type":"assistant","message":{"content":"Response"}}
{"type":"user","message":{"content":"\u2764\ufe0f emoji test"}}
{"type":"assistant","message":{"content":"Done"}}
{"type":"user","message":{"content":"Third with unicode: \u00e9\u00e8\u00ea"}}
{"type":"assistant","message":{"content":"Final"}}
{"type":"user","message":{"content":"Fourth"}}
{"type":"assistant","message":{"content":"Last"}}
{"type":"user","message":{"content":"Fifth"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 5, "Unicode content should not affect counting");
    }

    #[test]
    fn test_user_prompt_count_empty_file() {
        // Test case 5: Empty JSONL file → 0
        let result = parse_bytes(b"");
        assert_eq!(result.deep.user_prompt_count, 0, "Empty file should have 0 user prompts");
    }

    // --- Step 4: api_call_count (A1.6) ---

    #[test]
    fn test_api_call_count_basic() {
        // Test case 1: 5 user, 8 assistant → 8
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"a2"}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":"a3"}}
{"type":"user","message":{"content":"q4"}}
{"type":"assistant","message":{"content":"a4"}}
{"type":"user","message":{"content":"q5"}}
{"type":"assistant","message":{"content":"a5"}}
{"type":"assistant","message":{"content":"a6"}}
{"type":"assistant","message":{"content":"a7"}}
{"type":"assistant","message":{"content":"a8"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.api_call_count, 8, "Should count 8 assistant messages");
    }

    #[test]
    fn test_api_call_count_zero() {
        // Test case 2: 0 assistant messages → 0
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"user","message":{"content":"q2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.api_call_count, 0, "Should be 0 with no assistant messages");
    }

    // --- Step 5: tool_call_count (A1.7) ---

    #[test]
    fn test_tool_call_count_multiple_per_message() {
        // Test case 1: 3 assistant messages, each with 2 tools → 6
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Edit","input":{}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{}},{"type":"tool_use","name":"Write","input":{}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 6, "Should count 6 tool calls (2 per message x 3)");
    }

    #[test]
    fn test_tool_call_count_zero() {
        // Test case 2: Assistant message with no tool_use → 0
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"Just text, no tools."}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"More text."}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 0, "Should be 0 with no tool calls");
    }

    #[test]
    fn test_tool_call_count_parallel_tools() {
        // Test case 3: Assistant message with 5 parallel tool calls → 5
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 5, "Should count 5 parallel tool calls");
    }

    // --- Step 6: files_read (A1.2) ---

    #[test]
    fn test_files_read_basic() {
        // Test case 1: Read /a/foo.rs, /a/bar.rs → ["/a/foo.rs", "/a/bar.rs"], count=2
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/a/bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 2, "Should have 2 unique files read");
        assert!(result.deep.files_read.contains(&"/a/foo.rs".to_string()));
        assert!(result.deep.files_read.contains(&"/a/bar.rs".to_string()));
    }

    #[test]
    fn test_files_read_dedup() {
        // Test case 2: Read /a/foo.rs twice → ["/a/foo.rs"], count=1
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1, "Should dedup to 1 unique file");
        assert_eq!(result.deep.files_read.len(), 1);
        assert_eq!(result.deep.files_read[0], "/a/foo.rs");
    }

    #[test]
    fn test_files_read_empty() {
        // Test case 3: No Read tool calls → [], count=0
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 0, "Should have 0 files read");
        assert!(result.deep.files_read.is_empty());
    }

    #[test]
    fn test_files_read_missing_file_path() {
        // Test case 4: Read tool with missing file_path → skip
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"other":"value"}},{"type":"tool_use","name":"Read","input":{"file_path":"/valid/path.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1, "Should only count valid file_path");
        assert_eq!(result.deep.files_read[0], "/valid/path.rs");
    }

    #[test]
    fn test_files_read_with_spaces() {
        // Test case 5: Path with spaces → stored as-is
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/my file.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1);
        assert_eq!(result.deep.files_read[0], "/a/my file.rs", "Path with spaces should be preserved");
    }

    // --- Step 7: files_edited (A1.3) ---

    #[test]
    fn test_files_edited_basic() {
        // Test case 1: Edit foo.rs, Write bar.rs → ["foo.rs", "bar.rs"], count=2
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}},{"type":"tool_use","name":"Write","input":{"file_path":"bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited_count, 2, "Should have 2 unique files edited");
        assert!(result.deep.files_edited.contains(&"foo.rs".to_string()));
        assert!(result.deep.files_edited.contains(&"bar.rs".to_string()));
    }

    #[test]
    fn test_files_edited_all_occurrences_stored() {
        // Test case 2: Edit foo.rs 3 times → ["foo.rs", "foo.rs", "foo.rs"], count=1
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 3, "Should store ALL occurrences (3)");
        assert_eq!(result.deep.files_edited_count, 1, "Unique count should be 1");
    }

    #[test]
    fn test_files_edited_missing_file_path() {
        // Test case 3: Edit tool with missing file_path → skip
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{}},{"type":"tool_use","name":"Edit","input":{"file_path":"valid.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 1);
        assert_eq!(result.deep.files_edited_count, 1);
    }

    #[test]
    fn test_files_edited_write_tool() {
        // Test case 4: Write tool also counts as edited
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"new_file.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 1);
        assert_eq!(result.deep.files_edited_count, 1);
        assert_eq!(result.deep.files_edited[0], "new_file.rs");
    }

    // --- Step 8: reedited_files_count (A1.4) ---

    #[test]
    fn test_reedited_files_count_one_reedited() {
        // Test case 1: [foo.rs, foo.rs, foo.rs, bar.rs] → 1 (only foo.rs was re-edited)
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 1, "Only foo.rs was re-edited (3 times)");
    }

    #[test]
    fn test_reedited_files_count_all_unique() {
        // Test case 2: [a.rs, b.rs, c.rs] → 0 (all unique, no re-edits)
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"a.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"b.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"c.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 0, "No re-edits when all files are unique");
    }

    #[test]
    fn test_reedited_files_count_empty() {
        // Test case 3: [] → 0
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"No edits"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 0);
    }

    #[test]
    fn test_reedited_files_count_two_reedited() {
        // Test case 4: [x.rs, x.rs, y.rs, y.rs] → 2 (both files were re-edited)
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"x.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"y.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"x.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"y.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 2, "Both x.rs and y.rs were re-edited");
    }

    // --- Step 9: duration_seconds (A1.5) ---

    #[test]
    fn test_duration_seconds_basic() {
        // Test case 1: 10:00:00 to 10:15:30 → 930 seconds (15m 30s)
        let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
{"type":"assistant","timestamp":"2026-01-27T10:05:00Z","message":{"content":"a1"}}
{"type":"user","timestamp":"2026-01-27T10:10:00Z","message":{"content":"q2"}}
{"type":"assistant","timestamp":"2026-01-27T10:15:30Z","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 930, "Duration should be 930 seconds");
        assert_eq!(result.deep.first_timestamp, Some(1769508000));  // 2026-01-27T10:00:00Z
        assert_eq!(result.deep.last_timestamp, Some(1769508930));   // 2026-01-27T10:15:30Z
    }

    #[test]
    fn test_duration_seconds_single_message() {
        // Test case 2: Single message → 0
        let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 0, "Single message should have 0 duration");
    }

    #[test]
    fn test_duration_seconds_empty_file() {
        // Test case 3: Empty file → 0
        let result = parse_bytes(b"");
        assert_eq!(result.deep.duration_seconds, 0);
        assert!(result.deep.first_timestamp.is_none());
        assert!(result.deep.last_timestamp.is_none());
    }

    #[test]
    fn test_duration_seconds_no_timestamps() {
        // Test case 4: Messages with no timestamps → 0
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 0, "No timestamps means 0 duration");
    }

    #[test]
    fn test_duration_seconds_unix_timestamp() {
        // Test with Unix integer timestamps instead of ISO strings
        let data = br#"{"type":"user","timestamp":1706400000,"message":{"content":"q1"}}
{"type":"assistant","timestamp":1706400500,"message":{"content":"a1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 500, "Should handle Unix integer timestamps");
    }

    #[test]
    fn test_duration_seconds_mixed_messages() {
        // Only some messages have timestamps - should still compute from first/last
        let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","timestamp":"2026-01-27T10:10:00Z","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 600, "Should compute from first and last available timestamps");
    }

    // --- Unit test for count_reedited_files ---

    #[test]
    fn test_count_reedited_files_helper() {
        // Empty
        assert_eq!(count_reedited_files(&[]), 0);

        // All unique
        let unique = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(count_reedited_files(&unique), 0);

        // One re-edited
        let one_reedit = vec!["a".to_string(), "a".to_string(), "b".to_string()];
        assert_eq!(count_reedited_files(&one_reedit), 1);

        // Multiple re-edited
        let multi_reedit = vec![
            "a".to_string(), "a".to_string(),
            "b".to_string(), "b".to_string(), "b".to_string(),
            "c".to_string(),
        ];
        assert_eq!(count_reedited_files(&multi_reedit), 2); // a and b were re-edited
    }

    // --- Integration: All metrics together ---

    #[test]
    fn test_all_metrics_together() {
        // Comprehensive test with all metrics
        let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"Read and edit files"}}
{"type":"assistant","timestamp":"2026-01-27T10:05:00Z","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","timestamp":"2026-01-27T10:10:00Z","message":{"content":"Edit again"}}
{"type":"assistant","timestamp":"2026-01-27T10:15:00Z","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Write","input":{"file_path":"/src/new.rs"}}]}}
{"type":"user","timestamp":"2026-01-27T10:20:00Z","message":{"content":"Done"}}
{"type":"assistant","timestamp":"2026-01-27T10:25:00Z","message":{"content":"All done!"}}
"#;
        let result = parse_bytes(data);

        // user_prompt_count: 3
        assert_eq!(result.deep.user_prompt_count, 3);

        // api_call_count: 3
        assert_eq!(result.deep.api_call_count, 3);

        // tool_call_count: 5 (Read x 2, Edit x 2, Write x 1)
        assert_eq!(result.deep.tool_call_count, 5);

        // files_read: ["/src/lib.rs", "/src/main.rs"], count=2 (deduplicated)
        assert_eq!(result.deep.files_read_count, 2);

        // files_edited: ["/src/main.rs", "/src/main.rs", "/src/new.rs"], count=2 (unique)
        assert_eq!(result.deep.files_edited.len(), 3, "Should store all occurrences");
        assert_eq!(result.deep.files_edited_count, 2, "Unique count should be 2");

        // reedited_files_count: 1 (/src/main.rs was edited twice)
        assert_eq!(result.deep.reedited_files_count, 1);

        // duration_seconds: 10:00 to 10:25 = 25 minutes = 1500 seconds
        assert_eq!(result.deep.duration_seconds, 1500);
    }

    // ============================================================================
    // SIMD Pre-filter Tests
    // ============================================================================

    #[test]
    fn test_simd_prefilter_matches_full_parse() {
        let data = br#"{"type":"progress","uuid":"p1","data":{"type":"agent_progress"}}
{"type":"progress","uuid":"p2","data":{"type":"bash_progress"}}
{"type":"progress","uuid":"p3","data":{"type":"hook_progress"}}
{"type":"progress","uuid":"p4","data":{"type":"mcp_progress"}}
{"type":"progress","uuid":"p5","data":{"type":"waiting_for_task"}}
{"type":"queue-operation","uuid":"q1","operation":"enqueue"}
{"type":"queue-operation","uuid":"q2","operation":"dequeue"}
{"type":"file-history-snapshot","uuid":"f1","snapshot":{}}
{"type":"saved_hook_context","uuid":"s1","content":["ctx"]}
{"type":"user","uuid":"u1","message":{"role":"user","content":"hello"}}
{"type":"assistant","uuid":"a1","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"hi"}]}}
"#;

        let result = parse_bytes(data);
        let diag = &result.diagnostics;

        // Progress subtypes
        assert_eq!(result.deep.agent_spawn_count, 1);
        assert_eq!(result.deep.bash_progress_count, 1);
        assert_eq!(result.deep.hook_progress_count, 1);
        assert_eq!(result.deep.mcp_progress_count, 1);
        assert_eq!(diag.lines_progress, 5);

        // Queue
        assert_eq!(result.deep.queue_enqueue_count, 1);
        assert_eq!(result.deep.queue_dequeue_count, 1);

        // File snapshot
        assert_eq!(result.deep.file_snapshot_count, 1);

        // Hook context
        assert_eq!(diag.lines_hook_context, 1);

        // User + assistant still parsed correctly
        assert_eq!(diag.lines_user, 1);
        assert_eq!(diag.lines_assistant, 1);

        // JSON parse attempts should be ONLY for assistant + system + summary (not progress/queue/snapshot/hook_context).
        // User lines use raw-byte extraction (no JSON parse). In this test data: 1 line needs typed parse (assistant).
        assert_eq!(diag.json_parse_attempts, 1);
    }

    // ============================================================================
    // Pass 1 / Pass 2 / run_background_index Integration Tests
    // ============================================================================

    /// Helper: create a temp claude dir with sessions-index.json and JSONL files.
    fn setup_test_claude_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        let project_dir = claude_dir.join("projects").join("test-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create a JSONL file
        let jsonl_path = project_dir.join("sess-001.jsonl");
        let jsonl_content = br#"{"type":"user","message":{"content":"hello world"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","message":{"content":"now edit it"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
"#;
        std::fs::write(&jsonl_path, jsonl_content).unwrap();

        // Create sessions-index.json
        let index = format!(
            r#"[
            {{
                "sessionId": "sess-001",
                "fullPath": "{}",
                "firstPrompt": "hello world",
                "summary": "Test session about editing",
                "messageCount": 4,
                "modified": "2026-01-25T17:18:30.718Z",
                "gitBranch": "main",
                "isSidechain": false
            }}
        ]"#,
            jsonl_path.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_dir.join("sessions-index.json"), index).unwrap();

        (tmp, claude_dir)
    }

    #[tokio::test]
    async fn test_pass_1_reads_and_inserts() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        assert_eq!(projects, 1, "Should find 1 project");
        assert_eq!(sessions, 1, "Should find 1 session");

        // Verify the session is in the DB
        let db_projects = db.list_projects().await.unwrap();
        assert_eq!(db_projects.len(), 1);
        assert_eq!(db_projects[0].sessions.len(), 1);
        assert_eq!(db_projects[0].sessions[0].id, "sess-001");
        assert_eq!(db_projects[0].sessions[0].preview, "hello world");
        assert_eq!(
            db_projects[0].sessions[0].summary.as_deref(),
            Some("Test session about editing")
        );
    }

    #[tokio::test]
    async fn test_pass_1_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        std::fs::create_dir_all(claude_dir.join("projects")).unwrap();

        let db = Database::new_in_memory().await.unwrap();
        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        assert_eq!(projects, 0);
        assert_eq!(sessions, 0);
    }

    #[tokio::test]
    async fn test_pass_2_fills_deep_fields() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Run Pass 1 first to populate sessions
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        // Verify deep_indexed is false before Pass 2
        let projects = db.list_projects().await.unwrap();
        assert!(
            !projects[0].sessions[0].deep_indexed,
            "Should not be deep indexed yet"
        );

        // Run Pass 2 (no registry)
        let progress = Arc::new(AtomicUsize::new(0));
        let progress_clone = progress.clone();
        let (indexed, _) = pass_2_deep_index(&db, None, |_| {}, move |done, _total, _bytes| {
            progress_clone.store(done, Ordering::Relaxed);
        })
        .await
        .unwrap();

        assert_eq!(indexed, 1, "Should deep-index 1 session");
        assert_eq!(
            progress.load(Ordering::Relaxed),
            1,
            "Progress should report 1"
        );

        // Verify deep fields are populated
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert!(session.deep_indexed, "Should be deep indexed now");
        assert_eq!(session.turn_count, 2, "Should have 2 turns");
        assert_eq!(session.tool_counts.read, 1);
        assert_eq!(session.tool_counts.edit, 1);
        assert_eq!(session.last_message, "now edit it");
    }

    #[tokio::test]
    async fn test_pass_2_skips_already_indexed() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Run Pass 1 then Pass 2
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        let (first_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(first_run, 1);

        // Run Pass 2 again — should skip because deep_indexed_at is set
        let (second_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(second_run, 0, "Should skip already deep-indexed sessions");
    }

    #[tokio::test]
    async fn test_modified_session_gets_reindexed() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Run Pass 1 to populate sessions from index JSON
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        // Run Pass 2 — should deep-index the 1 session
        let (first_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(first_run, 1, "Should deep-index 1 session on first run");

        // Verify file_size_at_index and file_mtime_at_index are populated
        let sessions = db.get_sessions_needing_deep_index().await.unwrap();
        assert_eq!(sessions.len(), 1);
        let (_id, _path, stored_size, stored_mtime, deep_indexed_at, parse_version) = &sessions[0];
        assert!(
            stored_size.is_some(),
            "file_size_at_index should be populated after deep index"
        );
        assert!(
            stored_mtime.is_some(),
            "file_mtime_at_index should be populated after deep index"
        );
        assert!(
            deep_indexed_at.is_some(),
            "deep_indexed_at should be set"
        );
        assert_eq!(
            *parse_version, CURRENT_PARSE_VERSION,
            "parse_version should match current"
        );

        // Run Pass 2 again — should skip because file hasn't changed
        let (second_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(second_run, 0, "Should skip unchanged session");

        // Now append data to the JSONL file (simulates user continuing conversation)
        let jsonl_path = claude_dir
            .join("projects")
            .join("test-project")
            .join("sess-001.jsonl");

        // Sleep briefly to ensure mtime changes (filesystem granularity is 1 second)
        std::thread::sleep(std::time::Duration::from_millis(1100));

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&jsonl_path)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"one more question"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":"here is my answer"}}}}"#
        )
        .unwrap();
        drop(file);

        // Run Pass 2 again — should re-index because file size changed
        let (third_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(third_run, 1, "Should re-index session after file was modified");

        // Verify the updated metrics reflect the new content
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert_eq!(session.turn_count, 3, "Should now have 3 turns after re-index");
        assert_eq!(
            session.last_message, "one more question",
            "Last user message should reflect appended content"
        );

        // Run Pass 2 one more time — should skip again (file hasn't changed)
        let (fourth_run, _) = pass_2_deep_index(&db, None, |_| {}, |_, _, _| {}).await.unwrap();
        assert_eq!(fourth_run, 0, "Should skip after re-index completed");
    }

    #[tokio::test]
    async fn test_run_background_index_full_pipeline() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        let pass1_result = Arc::new(std::sync::Mutex::new((0usize, 0usize)));
        let complete_result = Arc::new(AtomicUsize::new(0));

        let p1 = pass1_result.clone();
        let cr = complete_result.clone();

        run_background_index(
            &claude_dir,
            &db,
            None, // no registry holder
            move |projects, sessions| {
                *p1.lock().unwrap() = (projects, sessions);
            },
            |_total_bytes| {},
            |_done, _total, _bytes| {},
            move |total| {
                cr.store(total, Ordering::Relaxed);
            },
        )
        .await
        .unwrap();

        let (projects, sessions) = *pass1_result.lock().unwrap();
        assert_eq!(projects, 1);
        assert_eq!(sessions, 1);
        assert_eq!(complete_result.load(Ordering::Relaxed), 1);

        // Verify full pipeline result
        let db_projects = db.list_projects().await.unwrap();
        assert_eq!(db_projects.len(), 1);
        let session = &db_projects[0].sessions[0];
        assert!(session.deep_indexed);
        assert_eq!(session.turn_count, 2);
        assert_eq!(session.tool_counts.read, 1);
        assert_eq!(session.tool_counts.edit, 1);
    }

    // ============================================================================
    // Commit Skill Detection Tests (A3.1 acceptance tests)
    // ============================================================================

    #[test]
    fn test_extract_commit_skill_invocations_commit() {
        // Scenario 1: skill="commit" invoked -> Detected, timestamp extracted
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit"})),
            byte_offset: 0,
            timestamp: 1706400120,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert_eq!(result.len(), 1, "Should detect 1 commit skill invocation");
        assert_eq!(result[0].skill_name, "commit");
        assert_eq!(result[0].timestamp_unix, 1706400120);
    }

    #[test]
    fn test_extract_commit_skill_invocations_commit_commands_commit() {
        // Scenario 2: skill="commit-commands:commit" invoked -> Detected
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit-commands:commit"})),
            byte_offset: 100,
            timestamp: 1706400200,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert_eq!(result.len(), 1, "Should detect commit-commands:commit");
        assert_eq!(result[0].skill_name, "commit-commands:commit");
        assert_eq!(result[0].timestamp_unix, 1706400200);
    }

    #[test]
    fn test_extract_commit_skill_invocations_commit_push_pr() {
        // Scenario 3: skill="commit-commands:commit-push-pr" invoked -> Detected
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit-commands:commit-push-pr"})),
            byte_offset: 200,
            timestamp: 1706400300,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert_eq!(result.len(), 1, "Should detect commit-commands:commit-push-pr");
        assert_eq!(result[0].skill_name, "commit-commands:commit-push-pr");
        assert_eq!(result[0].timestamp_unix, 1706400300);
    }

    #[test]
    fn test_extract_commit_skill_invocations_debug_not_detected() {
        // Scenario 4: skill="debug" invoked -> NOT detected (not commit-related)
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "debug"})),
            byte_offset: 0,
            timestamp: 1706400000,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert!(
            result.is_empty(),
            "Should NOT detect non-commit skill invocations"
        );
    }

    #[test]
    fn test_extract_commit_skill_invocations_no_skill_tool_use() {
        // Scenario 5: No Skill tool_use -> No skill invocations returned
        let raw = vec![
            RawInvocation {
                name: "Read".to_string(),
                input: Some(serde_json::json!({"file_path": "/src/main.rs"})),
                byte_offset: 0,
                timestamp: 1706400000,
            },
            RawInvocation {
                name: "Bash".to_string(),
                input: Some(serde_json::json!({"command": "cargo test"})),
                byte_offset: 100,
                timestamp: 1706400100,
            },
        ];

        let result = extract_commit_skill_invocations(&raw);

        assert!(result.is_empty(), "Should return empty for non-Skill tools");
    }

    #[test]
    fn test_extract_commit_skill_invocations_mixed() {
        // Mixed: multiple invocations, some commit-related, some not
        let raw = vec![
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "brainstorm"})),
                byte_offset: 0,
                timestamp: 1706400000,
            },
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "commit"})),
                byte_offset: 100,
                timestamp: 1706400100,
            },
            RawInvocation {
                name: "Read".to_string(),
                input: Some(serde_json::json!({"file_path": "/foo"})),
                byte_offset: 200,
                timestamp: 1706400200,
            },
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "commit-commands:commit-push-pr"})),
                byte_offset: 300,
                timestamp: 1706400300,
            },
        ];

        let result = extract_commit_skill_invocations(&raw);

        assert_eq!(result.len(), 2, "Should detect exactly 2 commit skills");
        assert_eq!(result[0].skill_name, "commit");
        assert_eq!(result[0].timestamp_unix, 1706400100);
        assert_eq!(result[1].skill_name, "commit-commands:commit-push-pr");
        assert_eq!(result[1].timestamp_unix, 1706400300);
    }

    #[test]
    fn test_extract_commit_skill_invocations_empty_input() {
        // Edge case: Skill tool with no input
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: None,
            byte_offset: 0,
            timestamp: 1706400000,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert!(result.is_empty(), "Should handle missing input gracefully");
    }

    #[test]
    fn test_extract_commit_skill_invocations_missing_skill_field() {
        // Edge case: Skill tool with input but no skill field
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"other_field": "value"})),
            byte_offset: 0,
            timestamp: 1706400000,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert!(
            result.is_empty(),
            "Should handle missing skill field gracefully"
        );
    }

    #[test]
    fn test_extract_commit_skill_invocations_skill_field_not_string() {
        // Edge case: skill field is not a string
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": 123})),
            byte_offset: 0,
            timestamp: 1706400000,
        }];

        let result = extract_commit_skill_invocations(&raw);

        assert!(
            result.is_empty(),
            "Should handle non-string skill field gracefully"
        );
    }

    #[test]
    fn test_extract_commit_skill_invocations_empty_list() {
        // Edge case: empty invocations list
        let raw: Vec<RawInvocation> = vec![];
        let result = extract_commit_skill_invocations(&raw);
        assert!(result.is_empty(), "Should return empty for empty input");
    }

    // ============================================================================
    // Golden Fixture Tests (Task 5: Full 7-type JSONL extraction)
    // ============================================================================

    #[test]
    fn test_golden_complete_session() {
        let data = include_bytes!("../tests/golden_fixtures/complete_session.jsonl");
        let result = parse_bytes(data);
        let diag = &result.diagnostics;

        // Line counts
        assert_eq!(diag.lines_total, 10);
        assert_eq!(diag.lines_user, 2);
        assert_eq!(diag.lines_assistant, 2);
        assert_eq!(diag.lines_system, 1);
        assert_eq!(diag.lines_progress, 1);
        assert_eq!(diag.lines_queue_op, 2);
        assert_eq!(diag.lines_summary, 1);
        assert_eq!(diag.lines_file_snapshot, 1);
        assert_eq!(diag.lines_unknown_type, 0);
        assert_eq!(diag.json_parse_failures, 0);

        // Core metrics
        assert_eq!(result.deep.user_prompt_count, 2);
        assert_eq!(result.deep.api_call_count, 2);
        assert_eq!(result.deep.tool_call_count, 3);
        assert_eq!(result.deep.files_read_count, 1);
        assert_eq!(result.deep.files_edited_count, 1);
        assert_eq!(result.deep.reedited_files_count, 1);

        // Tokens
        assert_eq!(result.deep.total_input_tokens, 3500);
        assert_eq!(result.deep.total_output_tokens, 350);
        assert_eq!(result.deep.cache_read_tokens, 500);
        assert_eq!(result.deep.cache_creation_tokens, 100);
        assert_eq!(result.deep.thinking_block_count, 1);

        // System
        assert_eq!(result.deep.turn_durations_ms, vec![5000]);
        assert_eq!(result.deep.compaction_count, 0);
        assert_eq!(result.deep.api_error_count, 0);

        // Progress
        assert_eq!(result.deep.hook_progress_count, 1);
        assert_eq!(result.deep.agent_spawn_count, 0);

        // Summary
        assert_eq!(result.deep.summary_text.as_deref(), Some("Fixed authentication bug in auth.rs"));

        // Queue
        assert_eq!(result.deep.queue_enqueue_count, 1);
        assert_eq!(result.deep.queue_dequeue_count, 1);

        // File snapshot
        assert_eq!(result.deep.file_snapshot_count, 1);

        // Wall-clock task time (single turn: user line 1 at 10:00:00 → last_timestamp at 10:04:00 = 240s)
        assert!(result.deep.total_task_time_seconds > 0, "total_task_time_seconds should be > 0");
        assert_eq!(result.deep.total_task_time_seconds, 240);
        assert!(result.deep.longest_task_seconds.is_some(), "longest_task_seconds should be Some");
        assert_eq!(result.deep.longest_task_seconds, Some(240));
        assert!(result.deep.longest_task_preview.is_some(), "longest_task_preview should be Some");
        assert_eq!(result.deep.longest_task_preview.as_deref(), Some("Read and fix auth.rs"));

        // Sanity check: wall-clock task time >= CC turn_duration_total_ms / 1000
        let turn_duration_total_secs = result.deep.turn_durations_ms.iter().sum::<u64>() / 1000;
        assert!(
            result.deep.total_task_time_seconds as u64 >= turn_duration_total_secs,
            "Wall-clock task time ({}) should be >= turn_duration_total_ms/1000 ({})",
            result.deep.total_task_time_seconds,
            turn_duration_total_secs,
        );
    }

    #[test]
    fn test_golden_edge_cases() {
        let data = include_bytes!("../tests/golden_fixtures/edge_cases.jsonl");
        let result = parse_bytes(data);
        let diag = &result.diagnostics;

        assert_eq!(diag.lines_unknown_type, 1);
        assert_eq!(diag.json_parse_failures, 1);
        assert_eq!(diag.content_not_array, 1);
        assert_eq!(diag.tool_use_missing_name, 1);
        assert_eq!(result.deep.api_error_count, 1);
        assert_eq!(result.deep.compaction_count, 1);
        assert_eq!(result.deep.agent_spawn_count, 1);
    }

    #[test]
    fn test_golden_spacing_variants() {
        let data = include_bytes!("../tests/golden_fixtures/spacing_variants.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.diagnostics.lines_user, 3);
        assert_eq!(result.deep.user_prompt_count, 3);
    }

    #[test]
    fn test_golden_empty_session() {
        let data = include_bytes!("../tests/golden_fixtures/empty_session.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.diagnostics.lines_total, 0);
        assert_eq!(result.deep.user_prompt_count, 0);
    }

    #[test]
    fn test_golden_text_only() {
        let data = include_bytes!("../tests/golden_fixtures/text_only_session.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 2);
        assert_eq!(result.deep.api_call_count, 2);
        assert_eq!(result.deep.tool_call_count, 0);
        assert_eq!(result.deep.files_read_count, 0);
        assert_eq!(result.deep.total_input_tokens, 300);
        assert_eq!(result.deep.total_output_tokens, 150);
    }

    // ============================================================================
    // Wall-clock task time tests
    // ============================================================================

    #[test]
    fn test_multi_turn_task_time() {
        // Fixture has 3 real user turns plus tool_result and system-prefix user messages.
        // Turn 1: ts=1700000000 "Implement the login page" → ends at last_ts=1700000100 → 100s
        // Turn 2: ts=1700000200 "Now add form validation..." → ends at last_ts=1700000295 → 95s
        // Turn 3: ts=1700000500 "Fix the CSS styling..."    → ends at last_ts=1700000540 → 40s
        let data = include_bytes!("../tests/golden_fixtures/multi_turn_task_time.jsonl");
        let result = parse_bytes(data);

        // 6 user lines total (3 real prompts + 2 tool_result + 1 system prefix)
        assert_eq!(result.deep.user_prompt_count, 6);

        // Wall-clock task time = 100 + 95 + 40 = 235s
        assert_eq!(result.deep.total_task_time_seconds, 235);

        // Longest task is turn 1 at 100 seconds
        assert_eq!(result.deep.longest_task_seconds, Some(100));
        assert_eq!(result.deep.longest_task_preview.as_deref(), Some("Implement the login page"));

        // Verify longest is the maximum, not first or last
        assert!(result.deep.longest_task_seconds.unwrap() > 40, "longest should not be the last turn (40s)");
        assert!(result.deep.longest_task_seconds.unwrap() > 95, "longest should not be the second turn (95s)");

        // Sanity check: wall-clock >= CC turn_duration_total / 1000
        let turn_duration_total_secs = result.deep.turn_durations_ms.iter().sum::<u64>() / 1000;
        assert!(
            result.deep.total_task_time_seconds as u64 >= turn_duration_total_secs,
            "Wall-clock ({}) should be >= turn_duration_total/1000 ({})",
            result.deep.total_task_time_seconds,
            turn_duration_total_secs,
        );
    }

    #[test]
    fn test_task_time_system_prefix_not_counted() {
        // A session with a system-prefix user message should not start a new turn
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"sessionId":"test","message":{"role":"user","content":"Hello world"},"version":"1.0.0","cwd":"/project"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":1700000060,"sessionId":"test","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"Hi!"}],"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"user","uuid":"u2","timestamp":1700000100,"sessionId":"test","message":{"role":"user","content":"<system-reminder>Context refresh</system-reminder>"},"version":"1.0.0","cwd":"/project"}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":1700000120,"sessionId":"test","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"Ok."}],"usage":{"input_tokens":100,"output_tokens":50}}}
"#;
        let result = parse_bytes(data);

        // Only 1 real turn: "Hello world" at ts=1700000000 → ends at ts=1700000120 (last_timestamp) = 120s
        assert_eq!(result.deep.total_task_time_seconds, 120);
        assert_eq!(result.deep.longest_task_seconds, Some(120));
        assert_eq!(result.deep.longest_task_preview.as_deref(), Some("Hello world"));
    }

    #[test]
    fn test_task_time_tool_result_not_counted() {
        // Tool result user messages should not start a new turn
        let data = br#"{"type":"user","uuid":"u1","timestamp":1700000000,"sessionId":"test","message":{"role":"user","content":"Read the file"},"version":"1.0.0","cwd":"/project"}
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":1700000030,"sessionId":"test","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"tool_use","id":"tu1","name":"Read","input":{"file_path":"/project/foo.rs"}}],"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":1700000060,"sessionId":"test","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn main() {}"}]}}
{"type":"assistant","uuid":"a2","parentUuid":"u2","timestamp":1700000090,"sessionId":"test","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"Done."}],"usage":{"input_tokens":100,"output_tokens":50}}}
"#;
        let result = parse_bytes(data);

        // Only 1 real turn: "Read the file" at ts=1700000000 → ends at ts=1700000090 = 90s
        assert_eq!(result.deep.total_task_time_seconds, 90);
        assert_eq!(result.deep.longest_task_seconds, Some(90));
        assert_eq!(result.deep.longest_task_preview.as_deref(), Some("Read the file"));
    }

    #[test]
    fn test_task_time_empty_session() {
        let data = include_bytes!("../tests/golden_fixtures/empty_session.jsonl");
        let result = parse_bytes(data);

        assert_eq!(result.deep.total_task_time_seconds, 0);
        assert_eq!(result.deep.longest_task_seconds, None);
        assert_eq!(result.deep.longest_task_preview, None);
    }

    // ============================================================================
    // ParseDiagnostics and ExtendedMetadata new fields tests
    // ============================================================================

    #[test]
    fn test_parse_diagnostics_default_zeroes() {
        let diag = ParseDiagnostics::default();
        assert_eq!(diag.lines_total, 0);
        assert_eq!(diag.lines_user, 0);
        assert_eq!(diag.lines_assistant, 0);
        assert_eq!(diag.lines_system, 0);
        assert_eq!(diag.lines_progress, 0);
        assert_eq!(diag.lines_queue_op, 0);
        assert_eq!(diag.lines_summary, 0);
        assert_eq!(diag.lines_file_snapshot, 0);
        assert_eq!(diag.lines_hook_context, 0);
        assert_eq!(diag.lines_unknown_type, 0);
        assert_eq!(diag.json_parse_failures, 0);
        assert_eq!(diag.content_not_array, 0);
    }

    #[test]
    fn test_extended_metadata_new_fields_default() {
        let meta = ExtendedMetadata::default();
        assert_eq!(meta.total_input_tokens, 0);
        assert_eq!(meta.total_output_tokens, 0);
        assert_eq!(meta.cache_read_tokens, 0);
        assert_eq!(meta.cache_creation_tokens, 0);
        assert_eq!(meta.thinking_block_count, 0);
        assert_eq!(meta.api_error_count, 0);
        assert_eq!(meta.compaction_count, 0);
        assert_eq!(meta.agent_spawn_count, 0);
        assert!(meta.summary_text.is_none());
        assert!(meta.turn_durations_ms.is_empty());
        assert_eq!(meta.queue_enqueue_count, 0);
        assert_eq!(meta.queue_dequeue_count, 0);
        assert_eq!(meta.file_snapshot_count, 0);
    }

    #[test]
    fn test_parse_bytes_extracts_skill_invocations_for_commit() {
        // Integration test: verify parse_bytes extracts Skill tool_use that can be filtered for commits
        let data = br#"{"type":"user","message":{"content":"commit this"}}
{"type":"assistant","timestamp":1706400100,"message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"commit"}}]}}
"#;
        let result = parse_bytes(data);

        // Should extract the Skill invocation
        assert_eq!(result.raw_invocations.len(), 1);
        assert_eq!(result.raw_invocations[0].name, "Skill");
        assert_eq!(
            result.raw_invocations[0]
                .input
                .as_ref()
                .and_then(|v| v.get("skill"))
                .and_then(|v| v.as_str()),
            Some("commit")
        );
        assert_eq!(result.raw_invocations[0].timestamp, 1706400100);

        // Now filter for commit skills
        let commit_skills = extract_commit_skill_invocations(&result.raw_invocations);
        assert_eq!(commit_skills.len(), 1);
        assert_eq!(commit_skills[0].skill_name, "commit");
        assert_eq!(commit_skills[0].timestamp_unix, 1706400100);
    }

    /// Helper: create a test claude dir with two sessions in two projects.
    /// Returns (TempDir, claude_dir_path, path_to_sess_002_jsonl).
    fn setup_two_session_claude_dir() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf)
    {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();

        // Project A with sess-001
        let project_a_dir = claude_dir.join("projects").join("project-a");
        std::fs::create_dir_all(&project_a_dir).unwrap();

        let jsonl_a = project_a_dir.join("sess-001.jsonl");
        std::fs::write(
            &jsonl_a,
            br#"{"type":"user","message":{"content":"hello from session 1"}}
{"type":"assistant","message":{"content":"hi back"}}
"#,
        )
        .unwrap();

        let index_a = format!(
            r#"[{{"sessionId":"sess-001","fullPath":"{}","firstPrompt":"hello from session 1","messageCount":2,"modified":"2026-01-25T10:00:00.000Z"}}]"#,
            jsonl_a.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_a_dir.join("sessions-index.json"), index_a).unwrap();

        // Project B with sess-002
        let project_b_dir = claude_dir.join("projects").join("project-b");
        std::fs::create_dir_all(&project_b_dir).unwrap();

        let jsonl_b = project_b_dir.join("sess-002.jsonl");
        std::fs::write(
            &jsonl_b,
            br#"{"type":"user","message":{"content":"hello from session 2"}}
{"type":"assistant","message":{"content":"hi again"}}
"#,
        )
        .unwrap();

        let index_b = format!(
            r#"[{{"sessionId":"sess-002","fullPath":"{}","firstPrompt":"hello from session 2","messageCount":2,"modified":"2026-01-25T11:00:00.000Z"}}]"#,
            jsonl_b.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_b_dir.join("sessions-index.json"), index_b).unwrap();

        (tmp, claude_dir, jsonl_b)
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_removes_deleted_files() {
        let (_tmp, claude_dir, jsonl_b_path) = setup_two_session_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        // Pass 1: populate DB with both sessions
        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        assert_eq!(projects, 2);
        assert_eq!(sessions, 2);

        // Verify both sessions are in the DB
        let all_paths = db.get_all_session_file_paths().await.unwrap();
        assert_eq!(all_paths.len(), 2, "Should have 2 session file paths");

        // Delete sess-002's JSONL file from disk
        std::fs::remove_file(&jsonl_b_path).unwrap();
        assert!(!jsonl_b_path.exists(), "File should be deleted");

        // Prune stale sessions
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(pruned, 1, "Should have pruned 1 stale session");

        // Verify only sess-001 remains
        let remaining_paths = db.get_all_session_file_paths().await.unwrap();
        assert_eq!(remaining_paths.len(), 1, "Should have 1 session left");

        let db_projects = db.list_projects().await.unwrap();
        let all_session_ids: Vec<&str> = db_projects
            .iter()
            .flat_map(|p| p.sessions.iter().map(|s| s.id.as_str()))
            .collect();
        assert!(
            all_session_ids.contains(&"sess-001"),
            "sess-001 should still exist"
        );
        assert!(
            !all_session_ids.contains(&"sess-002"),
            "sess-002 should be pruned"
        );
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_no_op_when_all_exist() {
        let (_tmp, claude_dir, _jsonl_b_path) = setup_two_session_claude_dir();
        let db = Database::new_in_memory().await.unwrap();

        pass_1_read_indexes(&claude_dir, &db).await.unwrap();

        // Prune when all files still exist -- should remove nothing
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(
            pruned, 0,
            "Should not prune any sessions when all files exist"
        );

        let all_paths = db.get_all_session_file_paths().await.unwrap();
        assert_eq!(all_paths.len(), 2, "Both sessions should still be present");
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_empty_db() {
        let db = Database::new_in_memory().await.unwrap();

        // Prune on an empty DB -- should be a no-op
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(pruned, 0, "Should not prune anything from empty DB");
    }

    // ============================================================================
    // first_user_prompt extraction tests
    // ============================================================================

    #[test]
    fn test_first_user_prompt_extracted() {
        // 3 user messages — first_user_prompt should capture the FIRST one
        let data = br#"{"type":"user","message":{"content":"What is Rust?"}}
{"type":"assistant","message":{"content":"Rust is a systems programming language."}}
{"type":"user","message":{"content":"How do I use iterators?"}}
{"type":"assistant","message":{"content":"Iterators in Rust..."}}
{"type":"user","message":{"content":"Thanks for the help!"}}
{"type":"assistant","message":{"content":"You're welcome!"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(
            result.deep.first_user_prompt,
            Some("What is Rust?".to_string()),
            "Should capture the first user message text"
        );
        // Verify last_message is still the LAST user content
        assert_eq!(result.deep.last_message, "Thanks for the help!");
    }

    #[test]
    fn test_first_user_prompt_none_when_no_users() {
        // Only assistant messages — no user messages at all
        let data = br#"{"type":"assistant","message":{"content":"Hello, I'm an assistant."}}
{"type":"assistant","message":{"content":"Here is more info."}}
"#;
        let result = parse_bytes(data);
        assert!(
            result.deep.first_user_prompt.is_none(),
            "Should be None when there are no user messages"
        );
    }

    #[test]
    fn test_first_user_prompt_skips_system_hook_messages() {
        // First user messages are system hooks — should be skipped
        let data = br#"{"type":"user","message":{"content":"<local-command-caveat>Caveat: The messages below were generated by the user while running local commands.</local-command-caveat>"}}
{"type":"user","message":{"content":"<command-name>/clear</command-name>"}}
{"type":"user","message":{"content":"<local-command-stdout></local-command-stdout>"}}
{"type":"user","message":{"content":"do a release for me"}}
{"type":"assistant","message":{"content":"Sure, I'll help with the release."}}
"#;
        let result = parse_bytes(data);
        assert_eq!(
            result.deep.first_user_prompt,
            Some("do a release for me".to_string()),
            "Should skip system/hook messages and capture the first real user prompt"
        );
    }

    #[test]
    fn test_first_user_prompt_none_when_only_system_messages() {
        // All user messages are system hooks — first_user_prompt should be None
        let data = br#"{"type":"user","message":{"content":"<local-command-caveat>Caveat text</local-command-caveat>"}}
{"type":"user","message":{"content":"<command-name>/help</command-name>"}}
{"type":"assistant","message":{"content":"Here is help."}}
"#;
        let result = parse_bytes(data);
        assert!(
            result.deep.first_user_prompt.is_none(),
            "Should be None when all user messages are system/hook messages"
        );
    }

    #[test]
    fn test_first_user_prompt_truncated_at_500_chars() {
        // A user message with content longer than 500 characters
        let long_msg = "x".repeat(600);
        let line = format!(
            r#"{{"type":"user","message":{{"content":"{}"}}}}"#,
            long_msg
        );
        let data = line.as_bytes();
        let result = parse_bytes(data);
        let prompt = result.deep.first_user_prompt.expect("Should have first_user_prompt");
        // truncate() adds "..." when truncating, so max is 500 chars + "..."
        assert!(
            prompt.chars().count() <= 503,
            "first_user_prompt should be truncated to ~500 chars, got {} chars",
            prompt.chars().count()
        );
    }
}
