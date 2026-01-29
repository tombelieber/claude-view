---
status: draft
date: 2026-01-29
---

# JSONL Parser Specification

> **For Claude:** This is a reference spec, not an implementation plan. Use this to understand the JSONL format, extraction targets, and failure modes before modifying any parser code.

**Goal:** Define the complete extraction contract for Claude Code JSONL session files — every line type, every field, every edge case — so the parser can be hardened to production grade with full observability.

**Performance target:** 10GB under 10s (Rust serde_json, mmap, parallel by session).

**Architecture:** Full JSON parse every line. Profiling on 1.1GB real data shows Rust can full-parse 10GB in ~0.6s — well within budget. No need to sacrifice completeness for speed.

---

## 1. JSONL Format Overview

Claude Code writes one JSON object per line to `~/.claude/projects/<encoded-project>/<session-id>.jsonl`.

**Invariants:**
- One complete JSON object per line (no multi-line JSON)
- Lines are append-only (Claude Code never truncates)
- Files can exceed 100MB for long sessions (largest observed: 143MB)
- Line sizes vary from 200B to 780KB (progress lines with agent history)

**7 line types, by frequency in 1.1GB real dataset (1012 files, 104K lines):**

| Line Type | % Lines | % Bytes | Description |
|-----------|---------|---------|-------------|
| `progress` | 37.6% | 63.7% | Sub-agent, bash, hook, MCP progress events |
| `assistant` | 26.7% | 4.9% | Model responses with tool calls |
| `user` | 20.3% | 30.9% | User prompts and tool results |
| `queue-operation` | 11.4% | 0.2% | Message queue enqueue/dequeue |
| `system` | 1.6% | 0.1% | Turn durations, errors, compaction |
| `file-history-snapshot` | 1.6% | 0.1% | File backup checkpoints |
| `summary` | 0.8% | 0.0% | Auto-generated session summaries |

---

## 2. Two-Pass Parser Architecture

The codebase has two independent parsers:

| Parser | Location | When | Strategy |
|--------|----------|------|----------|
| **Pass 1: Discovery** | `crates/core/src/discovery.rs` | Initial scan | Async line-by-line, regex, fast but shallow |
| **Pass 2: Deep indexer** | `crates/db/src/indexer_parallel.rs` | Background | mmap + SIMD pre-filter + JSON parse |

**Pass 1** extracts: preview, message count, tool counts (approximate via string matching), files_touched, skills_used.

**Pass 2** extracts: everything Pass 1 does plus token usage, model IDs, raw tool invocations, files_read/edited, re-edit counts, duration, commit skill invocations.

**Overlap:** Both extract `files_touched`, `skills_used`, `tool_counts`. Pass 2 is authoritative — Pass 1 values are overwritten on deep index.

**Gap:** Neither parser currently extracts `system`, `progress`, `summary`, `queue-operation`, or `file-history-snapshot` line types.

---

## 3. Extraction Spec — Line Type `user`

20.3% of lines, 30.9% of bytes.

```
{"type":"user", "message":{"role":"user","content":...}, ...}
```

### Top-level fields

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `uuid` | `.uuid` | `string` | Unique message ID |
| `parentUuid` | `.parentUuid` | `string?` | Conversation tree parent |
| `timestamp` | `.timestamp` | `ISO8601` | When user sent prompt |
| `sessionId` | `.sessionId` | `string` | Session linkage |
| `version` | `.version` | `string` | Claude Code version |
| `gitBranch` | `.gitBranch` | `string?` | Active git branch |
| `cwd` | `.cwd` | `string` | Working directory |
| `isSidechain` | `.isSidechain` | `bool` | Branched conversation |
| `userType` | `.userType` | `string` | "external" / "internal" |
| `permissionMode` | `.permissionMode` | `string?` | Permission level |
| `isMeta` | `.isMeta` | `bool?` | Meta-message flag |
| `isCompactSummary` | `.isCompactSummary` | `bool?` | Compaction summary |
| `slug` | `.slug` | `string?` | Session slug |
| `agentId` | `.agentId` | `string?` | If from sub-agent |
| `isVisibleInTranscriptOnly` | `.isVisibleInTranscriptOnly` | `bool?` | Hidden from model context |

### Message content

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| **Content** | `.message.content` | `string\|array` | Prompt text or tool result blocks |
| **Tool results** | `.message.content[].type=="tool_result"` | `array` | Results returned to Claude |
| `toolUseResult` | `.toolUseResult` | `object?` | Tool result metadata |
| `sourceToolUseID` | `.sourceToolUseID` | `string?` | Which tool_use this responds to |
| `sourceToolAssistantUUID` | `.sourceToolAssistantUUID` | `string?` | Which assistant msg spawned the tool |
| `thinkingMetadata` | `.thinkingMetadata` | `object?` | Thinking budget/config |
| `todos` | `.todos` | `array?` | Todo items attached |
| `imagePasteIds` | `.imagePasteIds` | `array?` | Pasted image references |

### Derived metrics

- Prompt count (count of user lines)
- Prompt length distribution (content string lengths)
- Tool result count (content blocks with `type=="tool_result"`)
- Branch detection (isSidechain tracking)
- Claude Code version over time
- Permission mode usage
- Image paste frequency

---

## 4. Extraction Spec — Line Type `assistant`

26.7% of lines, 4.9% of bytes. Richest for tool metrics.

```
{"type":"assistant", "message":{"role":"assistant","content":[...],"model":"...","usage":{...}}, ...}
```

### Top-level fields

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `uuid` | `.uuid` | `string` | Unique message ID |
| `parentUuid` | `.parentUuid` | `string?` | Conversation tree parent |
| `timestamp` | `.timestamp` | `ISO8601` | When assistant responded |
| `sessionId` | `.sessionId` | `string` | Session linkage |
| `version` | `.version` | `string` | Claude Code version |
| `gitBranch` | `.gitBranch` | `string?` | Active git branch |
| `cwd` | `.cwd` | `string` | Working directory |
| `isSidechain` | `.isSidechain` | `bool` | Branched conversation |
| `userType` | `.userType` | `string` | "external" / "internal" |
| `slug` | `.slug` | `string?` | Session slug |
| `agentId` | `.agentId` | `string?` | If from sub-agent |
| `requestId` | `.requestId` | `string?` | API request ID |
| `isApiErrorMessage` | `.isApiErrorMessage` | `bool?` | Error response flag |
| `error` | `.error` | `object?` | Error details |

### Message-level fields

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `model` | `.message.model` | `string` | Model ID (opus, sonnet, haiku) |
| `stopReason` | `.message.stop_reason` | `string?` | Why generation stopped |
| `input_tokens` | `.message.usage.input_tokens` | `u64` | Tokens consumed |
| `output_tokens` | `.message.usage.output_tokens` | `u64` | Tokens generated |
| `cache_read_input_tokens` | `.message.usage.cache_read_input_tokens` | `u64?` | Cache hit tokens |
| `cache_creation_input_tokens` | `.message.usage.cache_creation_input_tokens` | `u64?` | Cache write tokens |

### Content blocks (`.message.content[]`)

| Block Type | Key Fields | Description |
|------------|-----------|-------------|
| `text` | `.text` | Response text |
| `thinking` | `.thinking` | Thinking content |
| `tool_use` | `.id`, `.name`, `.input` | Tool invocation |
| `tool_result` | `.tool_use_id`, `.content` | Tool execution result |

### Per `tool_use` block extraction

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `id` | `.id` | `string` | Tool call ID |
| `name` | `.name` | `string` | Tool name: Read, Edit, Write, Bash, Grep, Glob, Task, Skill, etc. |
| `input.file_path` | `.input.file_path` | `string?` | File target (Read/Edit/Write) |
| `input.command` | `.input.command` | `string?` | Bash command |
| `input.pattern` | `.input.pattern` | `string?` | Grep/Glob pattern |
| `input.skill` | `.input.skill` | `string?` | Skill name (Skill tool) |
| `input.prompt` | `.input.prompt` | `string?` | Sub-agent prompt (Task tool) |
| `input.old_string` | `.input.old_string` | `string?` | Edit before text |
| `input.new_string` | `.input.new_string` | `string?` | Edit after text |
| `input.content` | `.input.content` | `string?` | Write file content |
| `input.url` | `.input.url` | `string?` | WebFetch/WebSearch target |
| `input.query` | `.input.query` | `string?` | Search query |

### Derived metrics

- API call count
- Token usage per turn (input, output, cache read, cache creation)
- Model distribution per session
- Tool call count per type
- Files read/edited/written (from `tool_use.input.file_path`)
- Re-edit rate (same `file_path` in Edit/Write 2+ times)
- Bash command inventory
- Thinking usage (count/length of thinking blocks)
- Error rate (`isApiErrorMessage`)
- Text response length distribution

---

## 5. Extraction Spec — Line Type `system`

1.6% of lines, 0.1% of bytes. Unique operational data available nowhere else.

```
{"type":"system", "subtype":"...", ...}
```

### Common fields (all subtypes)

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `uuid` | `.uuid` | `string` | Unique message ID |
| `parentUuid` | `.parentUuid` | `string?` | Conversation tree parent |
| `logicalParentUuid` | `.logicalParentUuid` | `string?` | Logical parent (post-compaction) |
| `timestamp` | `.timestamp` | `ISO8601` | When event occurred |
| `sessionId` | `.sessionId` | `string` | Session linkage |
| `subtype` | `.subtype` | `string` | **Discriminator** |
| `level` | `.level` | `string?` | "info" / "error" |
| `isMeta` | `.isMeta` | `bool` | Meta-message flag |
| `isSidechain` | `.isSidechain` | `bool` | Branched conversation |

### Subtypes

**`turn_duration`** — Precise per-turn timing:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `durationMs` | `.durationMs` | `u64` | Exact turn latency in ms |

**`api_error`** — API failure tracking:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `error` | `.error` | `object` | Error details |
| `retryAttempt` | `.retryAttempt` | `u32` | Which retry (1-based) |
| `maxRetries` | `.maxRetries` | `u32` | Retry limit |
| `retryInMs` | `.retryInMs` | `f64` | Backoff delay |

**`compact_boundary`** — Context window compaction:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `content` | `.content` | `string` | "Conversation compacted" |
| `compactMetadata.trigger` | `.compactMetadata.trigger` | `string` | "auto" / "manual" |
| `compactMetadata.preTokens` | `.compactMetadata.preTokens` | `u64` | Token count before compaction |

**`microcompact_boundary`** — Lightweight compaction:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `microcompactMetadata` | `.microcompactMetadata` | `object?` | Compaction details |

**`stop_hook_summary`** — Hook execution report:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `hookCount` | `.hookCount` | `u32` | Hooks run |
| `hookInfos` | `.hookInfos` | `array` | Hook commands |
| `hookErrors` | `.hookErrors` | `array?` | Hook failures |
| `hasOutput` | `.hasOutput` | `bool?` | Hooks produced output |
| `preventedContinuation` | `.preventedContinuation` | `bool?` | Hook blocked the turn |
| `durationMs` | `.durationMs` | `u64?` | Hook execution time |

**`local_command`** — Local CLI commands:

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `content` | `.content` | `string` | Command description |

### Derived metrics

- Per-turn latency distribution (`turn_duration.durationMs`)
- API error rate and retry patterns
- Compaction frequency and token thresholds
- Hook reliability (errors, blocked continuations)
- Session health score (errors / total turns)

---

## 6. Extraction Spec — Line Type `progress`

37.6% of lines, 63.7% of bytes. Discriminated by `data.type`.

```
{"type":"progress", "data":{"type":"...", ...}, ...}
```

### Common fields (all subtypes)

| Field | JSON Path | Type | Description |
|-------|-----------|------|-------------|
| `uuid` | `.uuid` | `string` | Unique event ID |
| `parentUuid` | `.parentUuid` | `string?` | Parent message |
| `timestamp` | `.timestamp` | `ISO8601` | When event occurred |
| `sessionId` | `.sessionId` | `string` | Session linkage |
| `toolUseID` | `.toolUseID` | `string` | Which tool_use spawned this |
| `parentToolUseID` | `.parentToolUseID` | `string?` | Parent tool |
| `agentId` | `.agentId` | `string?` | Sub-agent ID |
| `data.type` | `.data.type` | `string` | **Discriminator** |

### Subtypes

**`agent_progress`** — Sub-agent execution (7,459 events observed):

| Field | JSON Path | Extract? | Notes |
|-------|-----------|----------|-------|
| `data.prompt` | `.data.prompt` | Yes | Sub-agent task description |
| `data.agentId` | `.data.agentId` | Yes | Sub-agent identifier |
| `data.message.message.usage` | `.data.message.message.usage` | Yes | Token usage from sub-agent turn |
| `data.message.message.model` | `.data.message.message.model` | Yes | Model used by sub-agent |
| `data.normalizedMessages` | `.data.normalizedMessages` | **Metadata only** | Count + length. Content duplicated in sub-agent's own JSONL. |

**`bash_progress`** — Bash execution (2,064 events):

| Field | JSON Path | Extract? | Notes |
|-------|-----------|----------|-------|
| `data.*` | `.data.*` | Yes | Command progress, partial output |

**`hook_progress`** — Hook lifecycle (6,660 events):

| Field | JSON Path | Extract? | Notes |
|-------|-----------|----------|-------|
| `data.hookEvent` | `.data.hookEvent` | Yes | "SessionStart", "PreToolUse", etc. |
| `data.hookName` | `.data.hookName` | Yes | Hook identifier |
| `data.command` | `.data.command` | Yes | Shell command |

**`mcp_progress`** — MCP tool calls (274 events):

| Field | JSON Path | Extract? | Notes |
|-------|-----------|----------|-------|
| `data.*` | `.data.*` | Yes | MCP server interaction data |

**`waiting_for_task`** — Task queue waiting (19 events):

| Field | JSON Path | Extract? | Notes |
|-------|-----------|----------|-------|
| `data.*` | `.data.*` | Yes | Queue wait metadata |

### Performance note

`agent_progress` lines contain `data.message` (326KB) and `data.normalizedMessages` (337KB) — 99% of progress bytes. The full line is JSON-parsed (we have the budget at ~0.6s/10GB), but only metadata is extracted from these blobs. The full message history already exists in the sub-agent's own session JSONL file.

### Derived metrics

- Sub-agent spawn count and task distribution
- Bash command frequency
- Hook execution frequency by event type
- MCP tool usage patterns
- Task queue wait times

---

## 7. Extraction Spec — Remaining Types

### `queue-operation` — 11.4% of lines, 0.2% of bytes

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `operation` | `.operation` | `string` | "enqueue" / "dequeue" |
| `timestamp` | `.timestamp` | `ISO8601` | Queue event time |
| `sessionId` | `.sessionId` | `string` | Session linkage |
| `content` | `.content` | `string?` | Queued message content |

Derived: Queue wait time (dequeue - enqueue for paired operations).

### `summary` — 0.8% of lines

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `summary` | `.summary` | `string` | Auto-generated session summary |
| `leafUuid` | `.leafUuid` | `string` | Last message UUID at summary time |

Derived: Session summary text for search indexing and display.

### `file-history-snapshot` — 1.6% of lines

| Field | JSON Path | Type | Metric |
|-------|-----------|------|--------|
| `messageId` | `.messageId` | `string` | Associated message |
| `snapshot.trackedFileBackups` | `.snapshot.trackedFileBackups` | `object` | File backup map |
| `snapshot.timestamp` | `.snapshot.timestamp` | `ISO8601` | Snapshot time |
| `isSnapshotUpdate` | `.isSnapshotUpdate` | `bool` | Incremental vs full |

Derived: File checkpoint frequency, files under version control.

---

## 8. Silent Failure Inventory

Every place the current parser silently drops data.

### Pass 2 (`indexer_parallel.rs`) — 8 silent failures

| # | Location | Code | Drops | Severity |
|---|----------|------|-------|----------|
| 1 | `parse_bytes:200` | `line.is_empty() → continue` | Empty lines | Low |
| 2 | `extract_raw_invocations:416` | `Err(_) => return` | Entire assistant line on JSON parse failure | **Critical** |
| 3 | `extract_raw_invocations:433` | `None => return` | Lines where `.message.content` is not array | **High** |
| 4 | `extract_raw_invocations:442` | `None => continue` | tool_use blocks missing `name` | Medium |
| 5 | `extract_files_read_edited:347` | `_ => continue` | tool_use with empty/missing `file_path` | Low |
| 6 | `extract_turn_data:468` | `Err(_) => return` | Entire line on JSON parse failure | **High** |
| 7 | `extract_timestamp_from_line:318` | `.ok()?` | Unparseable timestamps | Medium |
| 8 | `pass_2_deep_index:813` | `Err(_) → default()` | Entire file on open failure | **Critical** |

### Pass 1 (`discovery.rs`) — 4 silent failures

| # | Location | Code | Drops | Severity |
|---|----------|------|-------|----------|
| 9 | `extract_session_metadata:510` | `Err(_) → default()` | Entire file on open failure | **Critical** |
| 10 | `get_project_sessions:442` | `Err(_) => continue` | Session entry on metadata failure | High |
| 11 | `extract_content_quick` | Returns `None` | Content if structure differs | Medium |
| 12 | Skill regex | Pattern miss | Unexpected skill naming | Medium |

### SIMD pre-filter false negatives

| # | Pattern | Misses when... | Impact |
|---|---------|---------------|--------|
| 13 | `"type\":\"user\""` | Spaced format: `"type": "user"` | **Critical** |
| 14 | `"type\":\"assistant\""` | Same spacing issue | **Critical** |
| 15 | `"tool_use"` | Different type name | **Critical** |
| 16 | `"input_tokens":` | Usage field renamed | **High** |

**Note:** #13-14 are real risks. `discovery.rs:540` checks both spacings. `indexer_parallel.rs:172` only checks compact form.

---

## 9. Observability Design

Per-session counters aggregated at end of `parse_bytes`. Zero per-line overhead — stack-allocated integers incremented in existing branches.

```rust
/// Parse diagnostic counters — per session, not per line.
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
```

### Logging strategy

**Per session (only on anomaly):**

```rust
if diag.json_parse_failures > 0 || diag.lines_unknown_type > 0 {
    tracing::warn!(
        session_id = %id,
        parse_failures = diag.json_parse_failures,
        unknown_types = diag.lines_unknown_type,
        "Parse anomalies detected"
    );
}
```

**Per indexing run (always):**

```
INFO  Indexing complete: 541 sessions, 104261 lines
      Parse failures: 0 (0.00%)
      Unknown line types: 0
      Tool blocks: 12847 | File paths: 8234 | Turns: 6102
```

If `json_parse_failures > 0` or `lines_unknown_type > 0` → format change alert.

---

## 10. Golden Test Fixtures

### Fixture 1: Complete session (all 7 line types)

File: `crates/db/tests/golden_fixtures/complete_session.jsonl`

Contains one example of every line type with realistic field values. Tests assert exact extraction output:

```rust
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

// Core metrics
assert_eq!(result.user_prompt_count, 2);
assert_eq!(result.api_call_count, 2);
assert_eq!(result.tool_call_count, 3);  // 1 Read + 2 Edit
assert_eq!(result.files_read, vec!["/project/src/auth.rs"]);
assert_eq!(result.files_read_count, 1);
assert_eq!(result.files_edited_count, 1);  // unique
assert_eq!(result.reedited_files_count, 1); // auth.rs edited 2x

// Tokens
assert_eq!(result.total_input_tokens, 3500);
assert_eq!(result.total_output_tokens, 350);
assert_eq!(result.cache_read_tokens, 500);
assert_eq!(result.cache_creation_tokens, 100);

// System
assert_eq!(result.turn_durations, vec![5000]);
assert_eq!(result.compaction_count, 0);
assert_eq!(result.api_error_count, 0);

// Progress
assert_eq!(result.hook_events, 1);
assert_eq!(result.agent_spawns, 0);

// Summary
assert_eq!(result.summary, Some("Fixed authentication bug in auth.rs"));
```

### Fixture 2: Edge cases (silent failure triggers)

File: `crates/db/tests/golden_fixtures/edge_cases.jsonl`

Covers: string content (not array), tool_use missing name, tool_use missing file_path, api_error, compact_boundary, agent_progress, unknown line type, invalid JSON.

```rust
assert_eq!(diag.lines_unknown_type, 1);      // "totally_new_type"
assert_eq!(diag.json_parse_failures, 1);      // invalid JSON line
assert_eq!(diag.content_not_array, 1);        // string content
assert_eq!(diag.tool_use_missing_name, 1);    // nameless tool_use
assert_eq!(result.api_error_count, 1);
assert_eq!(result.compaction_count, 1);
assert_eq!(result.agent_spawns, 1);
```

### Fixture 3: Spacing variants (SIMD pre-filter test)

File: `crates/db/tests/golden_fixtures/spacing_variants.jsonl`

Three user lines with different JSON spacing styles.

```rust
// ALL THREE must be counted
assert_eq!(diag.lines_user, 3);
assert_eq!(result.user_prompt_count, 3);
```

**This test currently fails** for the deep indexer — it only matches compact `"type\":\"user\""`, not spaced `"type": "user"`.

### Fixture 4: Large agent progress

File: `crates/db/tests/golden_fixtures/large_agent_progress.jsonl`

Single progress line with 300KB+ `data.normalizedMessages`. Verifies extraction completes without OOM and extracts metadata without storing the full blob.

### Fixture 5: Empty session

File: `crates/db/tests/golden_fixtures/empty_session.jsonl`

Zero lines. Returns all-zeros `ParseResult` without error.

### Fixture 6: Text-only session

File: `crates/db/tests/golden_fixtures/text_only_session.jsonl`

Multiple user/assistant exchanges, zero tool_use blocks. Verifies `files_read_count = 0` is correct (not a bug).

### Test organization

```
crates/db/tests/
├── golden_fixtures/
│   ├── complete_session.jsonl
│   ├── edge_cases.jsonl
│   ├── spacing_variants.jsonl
│   ├── large_agent_progress.jsonl
│   ├── empty_session.jsonl
│   └── text_only_session.jsonl
└── golden_test.rs
```

---

## 11. Performance Profile (Real Data)

Benchmarked on 1.1GB across 1012 files (Python, Rust estimates at 60x speedup):

| Approach | Python (750MB) | Rust (10GB est.) | Data Coverage |
|----------|---------------|------------------|---------------|
| Full parse every line | 2.69s | **~0.6s** | 100% |
| SIMD pre-filter + full parse on match | 0.98s | ~0.25s | ~95% |
| Current (SIMD selective, skip 5 types) | 0.16s | ~0.04s | ~40% |

**Decision:** Full parse. 0.6s at 10GB is 6% of the 10s budget. No reason to sacrifice completeness.

### Line size distribution (143MB session)

| Percentile | Size |
|------------|------|
| Min | 235 bytes |
| Median | 200KB |
| P95 | 651KB |
| Max | 780KB |

Large lines are `progress` → `agent_progress` with duplicated sub-agent message histories.

---

## 12. Timestamp Parsing Contract

JSONL timestamps appear in two formats. The parser must handle both.

### Format 1: ISO8601 / RFC3339 string (most common)

```json
"timestamp": "2026-01-28T15:59:32.066Z"
```

Parse with `chrono::DateTime::parse_from_rfc3339`. Convert to Unix seconds (i64).

### Format 2: Unix integer (rare, older sessions)

```json
"timestamp": 1706472000
```

Already an i64, use directly.

### Fallback

If neither format parses, increment `diag.timestamps_unparseable` and skip. Never panic, never default to 0 (which would corrupt duration calculations). Use `Option<i64>` — absence is safer than a wrong value.

### Edge cases

- Subsecond precision: truncate to seconds (we don't need ms for duration)
- Missing `timestamp` field entirely: valid for some line types (e.g., old `summary` lines). Not an error.
- Timezone: all observed timestamps are UTC (`Z` suffix). If non-UTC appears, `parse_from_rfc3339` handles it correctly.

---

## 13. Content Field Polymorphism

The `.message.content` field is **polymorphic** — it can be a string or an array depending on context. This is silent failure #3.

### Rules

| Line type | `.message.content` type | When |
|-----------|------------------------|------|
| `user` | `string` | Simple text prompts |
| `user` | `array` | Tool results being returned to Claude |
| `assistant` | `array` | Normal responses (text, thinking, tool_use blocks) |
| `assistant` | `string` | Rare — error messages, simple text responses |

### Handling

```rust
match content {
    Value::String(s) => {
        // Count as a message, extract text length, but no tool_use blocks
        diag.content_is_string += 1;
    }
    Value::Array(arr) => {
        // Normal path: iterate blocks, extract tool_use, text, thinking
        for block in arr { ... }
    }
    _ => {
        // null, number, bool, object — unexpected
        diag.content_unexpected_type += 1;
    }
}
```

**Never** treat string content as an error. It's valid — just means no tool_use blocks in that message. Increment a counter, extract what you can (text length, message count), move on.

---

## 14. Forward Compatibility Contract

Claude Code is actively developed. New line types, subtypes, content block types, and tool names will appear without notice. The parser must handle unknowns gracefully.

### Unknown line types

```rust
match line_type {
    "user" => { ... }
    "assistant" => { ... }
    "system" => { ... }
    "progress" => { ... }
    "queue-operation" => { ... }
    "summary" => { ... }
    "file-history-snapshot" => { ... }
    other => {
        diag.lines_unknown_type += 1;
        // Still extract common fields: timestamp, sessionId, uuid
        // Store raw type string for future analysis
    }
}
```

### Unknown system subtypes

```rust
match subtype {
    "turn_duration" => { ... }
    "api_error" => { ... }
    "compact_boundary" => { ... }
    // ... known subtypes
    other => {
        diag.unknown_system_subtypes += 1;
        // Still extract common system fields (timestamp, level, etc.)
    }
}
```

### Unknown progress data.types

Same pattern — extract common progress fields, count unknown.

### Unknown content block types

```rust
match block_type {
    "text" => { ... }
    "thinking" => { ... }
    "tool_use" => { ... }
    "tool_result" => { ... }
    other => {
        diag.unknown_content_block_types += 1;
        // Don't skip — still contributes to block count
    }
}
```

### Unknown tool names

New tools (e.g., `"NotebookEdit"`, `"AskUserQuestion"`) should still be counted in `tool_call_count` and stored as `RawInvocation`. Only tool-specific extraction (file_path for Read/Edit/Write) is gated on known names.

### Principle

**Parse what you can, count what you can't, never crash.** Every unknown increments a diagnostic counter. Zero unknowns = format hasn't changed. Non-zero unknowns = Claude Code shipped something new → investigate and update spec.

---

## 15. Data Model for New Extractions

The current `ExtendedMetadata` struct holds Pass 2 output. It needs to grow for the new line types. Rather than bloating one struct, split into domain groups.

### Proposed struct hierarchy

```rust
/// Complete parse output for one session.
pub struct ParseResult {
    /// Core metrics (user/assistant extraction) — existing
    pub core: CoreMetrics,
    /// System line extractions — NEW
    pub system: SystemMetrics,
    /// Progress line extractions — NEW
    pub progress: ProgressMetrics,
    /// Session metadata from misc line types — NEW
    pub metadata: SessionMetadata,
    /// Raw invocations for downstream classification — existing
    pub raw_invocations: Vec<RawInvocation>,
    /// Turn-level data — existing
    pub turns: Vec<RawTurn>,
    /// Models seen — existing
    pub models_seen: Vec<String>,
    /// Parse diagnostics — NEW
    pub diagnostics: ParseDiagnostics,
}

/// Core metrics from user + assistant lines.
pub struct CoreMetrics {
    // --- Existing fields (from ExtendedMetadata) ---
    pub tool_counts: ToolCounts,
    pub skills_used: Vec<String>,
    pub files_touched: Vec<String>,
    pub last_message: String,
    pub turn_count: usize,
    pub user_prompt_count: u32,
    pub api_call_count: u32,
    pub tool_call_count: u32,
    pub files_read: Vec<String>,
    pub files_edited: Vec<String>,
    pub files_read_count: u32,
    pub files_edited_count: u32,
    pub reedited_files_count: u32,
    pub duration_seconds: u32,
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    // --- New fields ---
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_block_count: u32,
    pub text_block_count: u32,
    pub error_response_count: u32,  // isApiErrorMessage == true
}

/// Metrics from system lines.
pub struct SystemMetrics {
    /// Per-turn durations in milliseconds (from turn_duration subtype)
    pub turn_durations_ms: Vec<u64>,
    /// API errors with retry metadata
    pub api_error_count: u32,
    pub api_total_retries: u32,
    /// Compaction events
    pub compaction_count: u32,
    pub compaction_pre_tokens: Vec<u64>,
    pub compaction_triggers: Vec<String>,  // "auto" / "manual"
    /// Microcompaction events
    pub microcompaction_count: u32,
    /// Hook summaries
    pub hook_summary_count: u32,
    pub hook_total_run: u32,
    pub hook_error_count: u32,
    pub hook_blocked_count: u32,  // preventedContinuation == true
}

/// Metrics from progress lines.
pub struct ProgressMetrics {
    /// Sub-agent spawns (agent_progress)
    pub agent_spawn_count: u32,
    pub agent_prompts: Vec<String>,  // task descriptions
    /// Bash progress events
    pub bash_progress_count: u32,
    /// Hook progress events by hook event type
    pub hook_progress_count: u32,
    pub hook_event_types: Vec<String>,  // "SessionStart", "PreToolUse", etc.
    /// MCP tool call progress
    pub mcp_progress_count: u32,
    /// Task queue waits
    pub waiting_for_task_count: u32,
}

/// Metadata from misc line types.
pub struct SessionMetadata {
    /// Auto-generated session summary (from summary lines)
    pub summary: Option<String>,
    /// Queue operations count
    pub queue_enqueue_count: u32,
    pub queue_dequeue_count: u32,
    /// File history snapshots
    pub file_snapshot_count: u32,
    /// Files tracked in snapshots
    pub snapshot_tracked_files: Vec<String>,
}
```

### Migration from `ExtendedMetadata`

`ExtendedMetadata` remains as an alias or wrapper during migration. Existing code that reads `result.deep.user_prompt_count` continues to work. New code uses `result.core.user_prompt_count`. Once migration is complete, remove `ExtendedMetadata`.

---

## 16. Re-Index Strategy

Sessions indexed before this spec have zeros for new fields. Two problems:

1. **New fields added to ExtendedMetadata** — existing sessions have default values (0, empty)
2. **Previously-ignored line types** — system, progress, summary data was never extracted

### Strategy: Schema-versioned re-index

Add a `parse_version` column to the `sessions` table:

```sql
ALTER TABLE sessions ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;
```

Each spec revision bumps the version:

| Version | What changed |
|---------|-------------|
| 0 | Pre-spec (current state) |
| 1 | Full 7-type extraction, ParseDiagnostics, spacing fix |
| 2+ | Future spec revisions |

### Re-index query

```sql
-- Sessions needing re-index: either never indexed OR indexed with old parser
SELECT id, file_path FROM sessions
WHERE deep_indexed_at IS NULL OR parse_version < ?
```

Replace the current `WHERE deep_indexed_at IS NULL` with this. On startup, if `CURRENT_PARSE_VERSION > max(parse_version)`, the background indexer automatically re-indexes all sessions.

### Performance

Re-indexing 1012 files (1.1GB) takes ~2s in Rust with parallelism. Acceptable as a one-time background task on version bump. Users see stale data briefly, then it refreshes.

---

## 17. Schema Migration for New Fields

New columns needed in the `sessions` table for extracted data from sections 5-7.

> **Best practices applied (Supabase Postgres guidelines adapted for SQLite):**
> - Use `INTEGER` for counts (SQLite has no bigint distinction — INTEGER is 64-bit)
> - Use `TEXT` for strings, never length-limited VARCHAR
> - Use `INTEGER` for timestamps (Unix seconds, not datetime strings)
> - Index all foreign key columns (SQLite doesn't auto-index FKs)
> - Batch all writes in transactions (50x faster than individual inserts)
> - Use partial indexes for re-index queries (only scan stale sessions)
> - Use `NOT NULL DEFAULT 0` for counters to avoid null-checking in queries

### From system lines

```sql
-- Turn timing (aggregate per session)
-- Use INTEGER — SQLite INTEGER is 64-bit, handles millisecond totals fine
ALTER TABLE sessions ADD COLUMN turn_duration_avg_ms INTEGER;
ALTER TABLE sessions ADD COLUMN turn_duration_max_ms INTEGER;
ALTER TABLE sessions ADD COLUMN turn_duration_total_ms INTEGER;

-- API reliability
ALTER TABLE sessions ADD COLUMN api_error_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN api_retry_count INTEGER NOT NULL DEFAULT 0;

-- Compaction
ALTER TABLE sessions ADD COLUMN compaction_count INTEGER NOT NULL DEFAULT 0;

-- Hooks
ALTER TABLE sessions ADD COLUMN hook_blocked_count INTEGER NOT NULL DEFAULT 0;
```

### From progress lines

```sql
-- Sub-agents
ALTER TABLE sessions ADD COLUMN agent_spawn_count INTEGER NOT NULL DEFAULT 0;

-- Bash/Hook/MCP activity
ALTER TABLE sessions ADD COLUMN bash_progress_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN hook_progress_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN mcp_progress_count INTEGER NOT NULL DEFAULT 0;
```

### From other types

```sql
-- Summary (TEXT, no length limit — same performance as VARCHAR in SQLite)
ALTER TABLE sessions ADD COLUMN summary_text TEXT;

-- Parse version for re-index strategy
ALTER TABLE sessions ADD COLUMN parse_version INTEGER NOT NULL DEFAULT 0;
```

### Indexes for re-indexing

```sql
-- Partial index: only sessions needing re-index (parse_version < current)
-- This is the query hot path on every startup — must be fast
-- Partial index keeps it small (only stale rows, not all 541+)
CREATE INDEX IF NOT EXISTS idx_sessions_needs_reindex
    ON sessions (id, file_path)
    WHERE parse_version < 1;  -- Bump the literal with each parse_version increment
```

**Note:** SQLite supports partial indexes (since 3.8.0). Rebuild this index on each `parse_version` bump via the migration that increments the version.

### Detail tables (optional, for drill-down)

For per-turn and per-event data that doesn't fit in aggregate columns.

```sql
-- Turn-level timing (one row per assistant turn)
-- Composite PK (session_id, turn_seq) — natural key, no surrogate needed
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
-- FK index: SQLite does NOT auto-index foreign keys
-- The PK already covers session_id (leftmost column), so no separate index needed here

-- API errors (one row per error event)
CREATE TABLE IF NOT EXISTS api_errors (
    id INTEGER PRIMARY KEY,  -- SQLite AUTOINCREMENT is unnecessary overhead; INTEGER PRIMARY KEY auto-increments
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,  -- Unix seconds, not datetime string
    retry_attempt INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    retry_in_ms REAL NOT NULL DEFAULT 0.0
);
-- FK index: required for fast CASCADE deletes and JOINs
CREATE INDEX IF NOT EXISTS idx_api_errors_session_id ON api_errors (session_id);

-- Hook events (one row per hook execution)
CREATE TABLE IF NOT EXISTS hook_events (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,
    hook_event_type TEXT NOT NULL,  -- "SessionStart", "PreToolUse", etc.
    hook_name TEXT,
    command TEXT,
    had_error INTEGER NOT NULL DEFAULT 0,  -- SQLite has no BOOLEAN; use 0/1
    prevented_continuation INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_hook_events_session_id ON hook_events (session_id);

-- Agent spawns (one row per sub-agent invocation)
CREATE TABLE IF NOT EXISTS agent_spawns (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    timestamp_unix INTEGER NOT NULL,
    agent_id TEXT,
    prompt TEXT,  -- Task description given to sub-agent
    model TEXT    -- Model used by sub-agent (from data.message.message.model)
);
CREATE INDEX IF NOT EXISTS idx_agent_spawns_session_id ON agent_spawns (session_id);
```

### Batch insert strategy

Per Supabase/Postgres best practice: never insert rows one-by-one. Batch all detail table writes in a single transaction per session.

```rust
// WRONG — N individual implicit transactions
for turn in turns {
    db.insert_turn_metric(&session_id, turn).await;
}

// RIGHT — single transaction for all turns in a session
db.batch_insert_turn_metrics(&session_id, &turns).await;
// Internally: BEGIN; INSERT INTO turn_metrics VALUES (...), (...), ...; COMMIT;
```

This is already the pattern used for `batch_upsert_commits` and `batch_insert_session_commits` in `git_correlation.rs`.

### Migration ordering

All schema changes go in a single numbered migration (e.g., migration 9):

```
1. ALTER TABLE sessions — add new aggregate columns
2. ALTER TABLE sessions — add parse_version
3. CREATE TABLE turn_metrics
4. CREATE TABLE api_errors
5. CREATE TABLE hook_events
6. CREATE TABLE agent_spawns
7. CREATE INDEX — FK indexes + partial re-index index
```

Single migration ensures atomicity — either all tables exist or none do.

---

## 18. Implementation Priority (Updated)

Reordered to account for gaps 1-6:

1. **Schema migration** — Add `parse_version` + new columns (section 17)
2. **Add `ParseDiagnostics`** — Zero-cost observability counters (section 9)
3. **Data model structs** — `CoreMetrics`, `SystemMetrics`, `ProgressMetrics`, `SessionMetadata` (section 15)
4. **Timestamp parsing** — Handle both ISO8601 and Unix integer (section 12)
5. **Content polymorphism** — Handle string vs array content (section 13)
6. **Fix SIMD spacing bug** — Add spaced variants to pre-filters
7. **Forward compatibility** — Unknown type/subtype counters (section 14)
8. **Golden test fixtures** — Create fixtures 1-6, write assertion tests (section 10)
9. **Extract system lines** — turn_duration, api_error, compact_boundary (section 5)
10. **Extract progress lines** — agent metadata, hook/bash/mcp events (section 6)
11. **Extract remaining types** — queue-operation, summary, file-history-snapshot (section 7)
12. **Full parse migration** — Replace SIMD-selective with full-parse-every-line
13. **Re-index trigger** — `parse_version` check on startup (section 16)
