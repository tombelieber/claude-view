# Fix Token Deduplication Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix token cost calculation AND `api_call_count` by deduplicating content blocks from the same API response using `message.id:requestId`, matching how ccusage correctly counts tokens.

**Architecture:** Claude Code's JSONL writes one line per content block (thinking, text, tool_use), and each line carries the full message-level `usage`. The parser currently creates one turn row per line and accumulates session-level tokens per line, inflating costs ~1.78x. Additionally, `api_call_count` is set to `assistant_count` (line count), inflating the "API calls" metric by the same factor — this cascades into `tool_density` (`tool_call_count / api_call_count`) and `classify_work_type` (via `turn_count`). The fix adds a `HashSet<String>` tracking `"{message_id}:{request_id}"` per session parse, skipping token accumulation for already-seen API responses, and derives `api_call_count` from the unique count instead of raw line count. Subagent JSONL discovery is a separate enhancement (not in scope — tracked separately).

**Tech Stack:** Rust, serde, memchr SIMD, SQLite (sqlx)

**Root cause proven by:** Comparing full recursive dedup (using `message.id:requestId`) against ccusage output — token counts match to <0.5% across all metrics (input, output, cache_read, cache_create). Verification step included in Task 6.

---

### Task 1: Add `message.id` and `requestId` to parser structs

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — `AssistantLine` struct, `AssistantMessage` struct, test module

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `indexer_parallel.rs`:

```rust
#[test]
fn test_parse_bytes_deduplicates_content_blocks() {
    // Simulate one API response split across 3 JSONL lines (thinking, text, tool_use)
    // All share the same message.id and requestId — tokens should only count ONCE
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"thinking"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a2","parentUuid":"a1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a3","parentUuid":"a2","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo.rs"}}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
"#;
    let result = parse_bytes(data);

    // Session-level tokens: should be counted ONCE, not 3x
    assert_eq!(result.deep.total_input_tokens, 100, "input_tokens counted 3x");
    assert_eq!(result.deep.total_output_tokens, 50, "output_tokens counted 3x");
    assert_eq!(result.deep.cache_read_tokens, 1000, "cache_read counted 3x");
    assert_eq!(result.deep.cache_creation_tokens, 200, "cache_create counted 3x");

    // api_call_count: 3 JSONL lines but only 1 unique API response
    assert_eq!(result.deep.api_call_count, 1, "api_call_count inflated by content blocks");

    // Should still create 3 turns (one per content block) for UI display,
    // but only the FIRST turn should carry token counts
    assert_eq!(result.turns.len(), 3);
    assert_eq!(result.turns[0].input_tokens, Some(100));
    assert_eq!(result.turns[1].input_tokens, Some(0)); // zeroed duplicate
    assert_eq!(result.turns[2].input_tokens, Some(0)); // zeroed duplicate
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_parse_bytes_deduplicates_content_blocks -- --nocapture`

Expected: FAIL — the test won't compile yet because `requestId` isn't deserialized. Once structs are updated, the test will fail with "input_tokens counted 3x" (getting 300 instead of 100).

**Step 3: Add `requestId` and `message.id` fields to structs**

In `crates/db/src/indexer_parallel.rs`, modify the `AssistantLine` struct (search for `struct AssistantLine`):

```rust
#[derive(Deserialize)]
struct AssistantLine {
    timestamp: Option<TimestampValue>,
    uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    parent_uuid: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<AssistantMessage>,
}
```

And `AssistantMessage` (search for `struct AssistantMessage`):

```rust
#[derive(Deserialize)]
struct AssistantMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<UsageBlock>,
    #[serde(default, deserialize_with = "deserialize_content")]
    content: ContentResult,
}
```

**Step 4: Run test to verify it compiles but still fails on assertion**

Run: `cargo test -p claude-view-db test_parse_bytes_deduplicates_content_blocks -- --nocapture`

Expected: FAIL with assertion `input_tokens counted 3x` (300 != 100). This confirms the structs parse but dedup logic isn't wired yet.

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "test: add failing test for content block token deduplication

Proves that identical usage from the same API response (shared
message.id + requestId) is counted 3x — once per content block line.
Also asserts api_call_count inflation (3 instead of 1)."
```

---

### Task 2: Implement deduplication in `parse_bytes` and `handle_assistant_line`

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — `parse_bytes()`, `handle_assistant_line()`, finalization block

**Step 1: Add `seen_api_calls` HashSet and `unique_api_call_count` to `parse_bytes`**

After the existing variable declarations (after `let mut last_timestamp`), add:

```rust
    // Dedup: track seen (message_id, request_id) pairs to avoid counting
    // tokens multiple times when Claude Code splits one API response into
    // multiple JSONL lines (one per content block: thinking, text, tool_use).
    let mut seen_api_calls: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Count of unique API calls (not inflated by content block splitting).
    // For legacy data without IDs, each line counts as unique (no dedup possible).
    let mut unique_api_call_count = 0u32;
```

**Step 2: Pass `seen_api_calls` and `unique_api_call_count` to `handle_assistant_line`**

Update the call site (search for `handle_assistant_line(` in the `type_assistant` SIMD branch):

```rust
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
                        &mut seen_api_calls,
                        &mut unique_api_call_count,
                    );
```

**Step 3: Update `handle_assistant_line` signature and add dedup logic**

Add the parameters to the function signature (search for `fn handle_assistant_line`):

```rust
fn handle_assistant_line(
    parsed: AssistantLine,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<claude_view_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: &mut u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
    seen_api_calls: &mut std::collections::HashSet<String>,
    unique_api_call_count: &mut u32,
) {
```

Then, inside the `if let Some(message) = parsed.message` block, replace the existing token accumulation + turn creation (everything from "Extract token usage" through the `turns.push` block, up to but not including "Extract tool_use blocks from content") with:

```rust
        // Dedup: check if this is a duplicate content block from the same API response.
        // Claude Code writes one JSONL line per content block (thinking, text, tool_use),
        // each carrying the full message-level usage. Only count tokens for the first block.
        let is_first_block = match (message.id.as_deref(), parsed.request_id.as_deref()) {
            (Some(msg_id), Some(req_id)) => {
                let key = format!("{}:{}", msg_id, req_id);
                seen_api_calls.insert(key) // true if newly inserted
            }
            _ => true, // no IDs available — count it (legacy data)
        };

        if is_first_block {
            *unique_api_call_count += 1;
        }

        // Extract token usage — only for first content block per API response
        if is_first_block {
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

            // Zero out token fields on duplicate blocks so per-turn queries don't double-count
            let (inp, outp, cr, cc) = if is_first_block {
                (
                    message.usage.as_ref().and_then(|u| u.input_tokens),
                    message.usage.as_ref().and_then(|u| u.output_tokens),
                    message.usage.as_ref().and_then(|u| u.cache_read_input_tokens),
                    message.usage.as_ref().and_then(|u| u.cache_creation_input_tokens),
                )
            } else {
                (Some(0), Some(0), Some(0), Some(0))
            };

            turns.push(claude_view_core::RawTurn {
                uuid,
                parent_uuid,
                seq: *assistant_count - 1,
                model_id,
                content_type,
                input_tokens: inp,
                output_tokens: outp,
                cache_read_tokens: cr,
                cache_creation_tokens: cc,
                service_tier: message.usage.as_ref().and_then(|u| u.service_tier.clone()),
                timestamp: ts,
            });
            diag.turns_extracted += 1;
        }
```

**Step 4: Fix `api_call_count` finalization**

In `parse_bytes`, find the finalization block (search for `result.deep.api_call_count = assistant_count`). Replace:

```rust
    result.deep.api_call_count = assistant_count;
```

with:

```rust
    result.deep.api_call_count = unique_api_call_count;
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p claude-view-db test_parse_bytes_deduplicates_content_blocks -- --nocapture`

Expected: PASS

**Step 6: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "fix: deduplicate content block tokens using message.id:requestId

Claude Code writes one JSONL line per content block (thinking, text,
tool_use), each carrying the full message-level usage. This caused
token counts to be inflated ~1.78x on average (up to 5x for complex
responses). api_call_count was also inflated (same root cause),
cascading into tool_density and work type classification.

Now tracks seen message_id:request_id pairs and only counts tokens
for the first content block per API response. api_call_count is
derived from unique API call count instead of raw assistant line count."
```

---

### Task 3: Apply same dedup to `handle_assistant_value` (fallback path)

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — `handle_assistant_value()`, its call site in the `"assistant"` match arm of the Value-dispatch block

**Step 1: Write a failing test for the fallback path**

The fallback path triggers when the SIMD byte finder for `"type":"assistant"` (no spaces) doesn't match — e.g., `"type" : "assistant"` (space before colon). The line falls through to the full `serde_json::from_slice::<Value>` parse at the bottom of the loop, which dispatches on `value.get("type")`. Add to tests:

```rust
#[test]
fn test_parse_bytes_deduplicates_fallback_path() {
    // SIMD finder looks for exact bytes "type":"assistant" — a space before
    // the colon causes the SIMD check to miss, falling through to the
    // full Value parse + type dispatch. This exercises handle_assistant_value.
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type" : "assistant","uuid":"b1","parentUuid":"u1","requestId":"req_002","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_002","model":"claude-opus-4-6","content":[{"type":"thinking"}],"usage":{"input_tokens":200,"output_tokens":80,"cache_read_input_tokens":5000,"cache_creation_input_tokens":300}}}
{"type" : "assistant","uuid":"b2","parentUuid":"b1","requestId":"req_002","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_002","model":"claude-opus-4-6","content":[{"type":"text","text":"world"}],"usage":{"input_tokens":200,"output_tokens":80,"cache_read_input_tokens":5000,"cache_creation_input_tokens":300}}}
"#;
    let result = parse_bytes(data);

    // Should count tokens only once despite 2 blocks
    assert_eq!(result.deep.total_input_tokens, 200, "fallback path: input_tokens counted 2x");
    assert_eq!(result.deep.total_output_tokens, 80, "fallback path: output_tokens counted 2x");
    assert_eq!(result.deep.cache_read_tokens, 5000, "fallback path: cache_read counted 2x");
    assert_eq!(result.deep.cache_creation_tokens, 300, "fallback path: cache_create counted 2x");

    // api_call_count: 2 JSONL lines but only 1 unique API response
    assert_eq!(result.deep.api_call_count, 1, "fallback path: api_call_count inflated");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_parse_bytes_deduplicates_fallback_path -- --nocapture`

Expected: FAIL (400 != 200 for input_tokens, 2 != 1 for api_call_count)

**Step 3: Pass `seen_api_calls` and `unique_api_call_count` to `handle_assistant_value` and add dedup**

Update the call site (search for `handle_assistant_value(` in the `"assistant" =>` match arm of the Value-dispatch block):

```rust
            "assistant" => {
                diag.lines_assistant += 1;
                assistant_count += 1;
                let fallback_assistant_ts = extract_timestamp_from_value(&value);
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
                    &mut seen_api_calls,
                    &mut unique_api_call_count,
                );
```

Update `handle_assistant_value` signature to accept the new params:

```rust
fn handle_assistant_value(
    value: &serde_json::Value,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<claude_view_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
    seen_api_calls: &mut std::collections::HashSet<String>,
    unique_api_call_count: &mut u32,
) {
```

Add dedup check at the top of the function body (before the existing token accumulation block):

```rust
    // Dedup check — same logic as handle_assistant_line
    let msg_id = value.get("message").and_then(|m| m.get("id")).and_then(|v| v.as_str());
    let req_id = value.get("requestId").and_then(|v| v.as_str());
    let is_first_block = match (msg_id, req_id) {
        (Some(mid), Some(rid)) => {
            let key = format!("{}:{}", mid, rid);
            seen_api_calls.insert(key)
        }
        _ => true,
    };

    if is_first_block {
        *unique_api_call_count += 1;
    }
```

Then wrap the existing token accumulation with `if is_first_block`:

```rust
    // Extract token usage from .message.usage — only for first block
    if is_first_block {
        if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
            // ... existing token accumulation unchanged ...
        }
    }
```

And zero out tokens on duplicate turns in the turn-creation block (search for `turns.push` inside `handle_assistant_value`):

```rust
            let (inp, outp, cr, cc) = if is_first_block {
                (
                    usage.and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()),
                    usage.and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()),
                    usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()),
                    usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()),
                )
            } else {
                (Some(0), Some(0), Some(0), Some(0))
            };

            turns.push(claude_view_core::RawTurn {
                uuid,
                parent_uuid,
                seq: assistant_count - 1,
                model_id,
                content_type,
                input_tokens: inp,
                output_tokens: outp,
                cache_read_tokens: cr,
                cache_creation_tokens: cc,
                service_tier: usage.and_then(|u| u.get("service_tier")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                timestamp,
            });
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-db test_parse_bytes_deduplicates_fallback_path -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "fix: apply token dedup to handle_assistant_value fallback path

Same message.id:requestId dedup logic applied to the Value-based
fallback parser that handles spacing variants in JSONL."
```

---

### Task 4: Add edge-case tests and golden fixture

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — test module
- Create: `crates/db/tests/golden_fixtures/dedup_content_blocks.jsonl`

**Step 1: Write edge-case tests**

```rust
#[test]
fn test_parse_bytes_dedup_different_api_calls_counted_separately() {
    // Two different API responses (different message IDs) should each count
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"thinking"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a2","parentUuid":"a1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"user","uuid":"u2","parentUuid":"a2","message":{"content":[{"type":"tool_result"}]}}
{"type":"assistant","uuid":"a3","parentUuid":"u2","requestId":"req_002","timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_002","model":"claude-opus-4-6","content":[{"type":"text","text":"done"}],"usage":{"input_tokens":150,"output_tokens":60,"cache_read_input_tokens":2000,"cache_creation_input_tokens":300}}}
"#;
    let result = parse_bytes(data);

    // msg_001 (100 input) + msg_002 (150 input) = 250 total
    assert_eq!(result.deep.total_input_tokens, 250);
    assert_eq!(result.deep.total_output_tokens, 110); // 50 + 60
    assert_eq!(result.deep.cache_read_tokens, 3000); // 1000 + 2000
    assert_eq!(result.deep.cache_creation_tokens, 500); // 200 + 300

    // 3 assistant JSONL lines, but only 2 unique API responses
    assert_eq!(result.deep.api_call_count, 2, "should count 2 unique API calls");
}

#[test]
fn test_parse_bytes_dedup_legacy_data_without_ids() {
    // Older JSONL lines without message.id/requestId should still count
    // (graceful fallback — every line counted, no dedup possible)
    let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","timestamp":"2026-01-01T00:00:00Z","message":{"model":"claude-opus-4-6","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"assistant","timestamp":"2026-01-01T00:00:00Z","message":{"model":"claude-opus-4-6","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo"}}],"usage":{"input_tokens":100,"output_tokens":50}}}
"#;
    let result = parse_bytes(data);

    // Without IDs, both lines count (can't dedup — legacy behavior preserved)
    assert_eq!(result.deep.total_input_tokens, 200);
    assert_eq!(result.deep.total_output_tokens, 100);

    // Legacy: each line treated as a separate API call (no IDs to dedup with)
    assert_eq!(result.deep.api_call_count, 2, "legacy lines each count as api call");
}

#[test]
fn test_parse_bytes_dedup_preserves_tool_counts() {
    // Dedup should NOT affect tool counting — all tool_use blocks still tracked
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a.rs"}}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a2","parentUuid":"a1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/b.rs"}}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
"#;
    let result = parse_bytes(data);

    // Tokens deduplicated
    assert_eq!(result.deep.total_input_tokens, 100);

    // But both tool_use blocks are counted
    assert_eq!(result.deep.tool_counts.read, 1);
    assert_eq!(result.deep.tool_counts.edit, 1);

    // Only 1 unique API call
    assert_eq!(result.deep.api_call_count, 1);
}

#[test]
fn test_parse_bytes_dedup_mixed_path_typed_and_fallback() {
    // Mix of typed path (exact "type":"assistant") and fallback path
    // (spacing variant "type" : "assistant"). Both share the same
    // message_id:request_id and should dedup together via the shared HashSet.
    let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type":"assistant","uuid":"c1","parentUuid":"u1","requestId":"req_003","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_003","model":"claude-opus-4-6","content":[{"type":"thinking"}],"usage":{"input_tokens":500,"output_tokens":100,"cache_read_input_tokens":3000,"cache_creation_input_tokens":400}}}
{"type" : "assistant","uuid":"c2","parentUuid":"c1","requestId":"req_003","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_003","model":"claude-opus-4-6","content":[{"type":"text","text":"mixed"}],"usage":{"input_tokens":500,"output_tokens":100,"cache_read_input_tokens":3000,"cache_creation_input_tokens":400}}}
"#;
    let result = parse_bytes(data);

    // Tokens counted once despite one line going through typed path
    // and the other through the Value fallback
    assert_eq!(result.deep.total_input_tokens, 500);
    assert_eq!(result.deep.total_output_tokens, 100);
    assert_eq!(result.deep.api_call_count, 1, "mixed paths should share dedup");
}
```

**Step 2: Create golden fixture for dedup scenario**

Create `crates/db/tests/golden_fixtures/dedup_content_blocks.jsonl` — a realistic multi-API-call session where each API response is split into multiple content block lines:

```jsonl
{"type":"user","uuid":"u1","timestamp":"2026-01-28T10:00:00Z","message":{"content":"Read and fix auth.rs"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","requestId":"req_001","timestamp":"2026-01-28T10:01:00Z","message":{"id":"msg_001","model":"claude-sonnet-4-20250514","content":[{"type":"thinking","thinking":"Let me read the file"}],"usage":{"input_tokens":1500,"output_tokens":150,"cache_read_input_tokens":500,"cache_creation_input_tokens":100}}}
{"type":"assistant","uuid":"a2","parentUuid":"a1","requestId":"req_001","timestamp":"2026-01-28T10:01:00Z","message":{"id":"msg_001","model":"claude-sonnet-4-20250514","content":[{"type":"tool_use","id":"tu1","name":"Read","input":{"file_path":"/project/src/auth.rs"}}],"usage":{"input_tokens":1500,"output_tokens":150,"cache_read_input_tokens":500,"cache_creation_input_tokens":100}}}
{"type":"user","uuid":"u2","parentUuid":"a2","timestamp":"2026-01-28T10:02:00Z","message":{"content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn authenticate() { todo!() }"}]}}
{"type":"assistant","uuid":"a3","parentUuid":"u2","requestId":"req_002","timestamp":"2026-01-28T10:03:00Z","message":{"id":"msg_002","model":"claude-sonnet-4-20250514","content":[{"type":"thinking","thinking":"I need to fix the auth function"}],"usage":{"input_tokens":2000,"output_tokens":200,"cache_read_input_tokens":1500,"cache_creation_input_tokens":0}}}
{"type":"assistant","uuid":"a4","parentUuid":"a3","requestId":"req_002","timestamp":"2026-01-28T10:03:00Z","message":{"id":"msg_002","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"I'll fix the authentication function."}],"usage":{"input_tokens":2000,"output_tokens":200,"cache_read_input_tokens":1500,"cache_creation_input_tokens":0}}}
{"type":"assistant","uuid":"a5","parentUuid":"a4","requestId":"req_002","timestamp":"2026-01-28T10:03:00Z","message":{"id":"msg_002","model":"claude-sonnet-4-20250514","content":[{"type":"tool_use","id":"tu2","name":"Edit","input":{"file_path":"/project/src/auth.rs","old_string":"todo!()","new_string":"validate_token(token)"}}],"usage":{"input_tokens":2000,"output_tokens":200,"cache_read_input_tokens":1500,"cache_creation_input_tokens":0}}}
```

**Step 3: Add golden fixture test**

```rust
#[test]
fn test_golden_dedup_content_blocks() {
    let data = include_bytes!("../tests/golden_fixtures/dedup_content_blocks.jsonl");
    let result = parse_bytes(data);

    // 2 unique API responses (req_001 with 2 blocks, req_002 with 3 blocks)
    assert_eq!(result.deep.api_call_count, 2, "should have 2 unique API calls");

    // Tokens: req_001(1500+150+500+100) + req_002(2000+200+1500+0)
    assert_eq!(result.deep.total_input_tokens, 3500);  // 1500 + 2000
    assert_eq!(result.deep.total_output_tokens, 350);   // 150 + 200
    assert_eq!(result.deep.cache_read_tokens, 2000);    // 500 + 1500
    assert_eq!(result.deep.cache_creation_tokens, 100);  // 100 + 0

    // Tools: Read from req_001, Edit from req_002 (both still counted)
    assert_eq!(result.deep.tool_counts.read, 1);
    assert_eq!(result.deep.tool_counts.edit, 1);

    // 5 assistant JSONL lines → 5 turns, but only 2 carry token data
    assert_eq!(result.turns.len(), 5);
    // First block of each API call has tokens, rest zeroed
    assert_eq!(result.turns[0].input_tokens, Some(1500)); // req_001 first
    assert_eq!(result.turns[1].input_tokens, Some(0));     // req_001 dup
    assert_eq!(result.turns[2].input_tokens, Some(2000)); // req_002 first
    assert_eq!(result.turns[3].input_tokens, Some(0));     // req_002 dup
    assert_eq!(result.turns[4].input_tokens, Some(0));     // req_002 dup

    // user_prompt_count: 2 user lines (one is tool_result, still counted)
    assert_eq!(result.deep.user_prompt_count, 2);
}
```

**Step 4: Run all tests**

Run: `cargo test -p claude-view-db -- --nocapture`

Expected: ALL PASS

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs crates/db/tests/golden_fixtures/dedup_content_blocks.jsonl
git commit -m "test: add edge-case tests and golden fixture for token dedup

Covers: separate API calls counted independently, legacy data without
message IDs falls back to counting all lines, tool counts unaffected
by token dedup, mixed typed/fallback paths share dedup state,
and a realistic golden fixture with multi-block API responses."
```

---

### Task 5: Bump parse_version to force re-index

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` — `CURRENT_PARSE_VERSION` constant (line 29)

**Step 1: Bump the parse version constant**

Change `CURRENT_PARSE_VERSION` from `8` to `9`:

```rust
pub const CURRENT_PARSE_VERSION: i32 = 9;
```

This constant controls whether existing sessions get re-parsed. The check at the re-index decision point (`parse_version < CURRENT_PARSE_VERSION`) will flag all existing sessions (stored as version 8) for re-indexing on next startup, applying the dedup fix to historical data.

**Step 2: Run full test suite**

Run: `cargo test -p claude-view-db`

Expected: PASS (existing tests should not depend on the version number, except `test_pass_2_fills_deep_fields` which asserts `parse_version == CURRENT_PARSE_VERSION` and should auto-update)

**Step 3: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "chore: bump parse_version to 9, force re-index with token dedup fix

All existing sessions will be re-parsed on next startup, correcting
historically inflated token counts and api_call_count."
```

---

### Task 6: Run the full test suite and verify against real data

**Step 1: Run all crate tests**

Run: `cargo test -p claude-view-core -p claude-view-db -p claude-view-server`

Expected: ALL PASS

**Step 2: Verify against a real JSONL file**

Pick any recent session JSONL file (e.g., `~/.claude/projects/*/sessions/*/session.jsonl`). Run a quick comparison:

```bash
# Count raw assistant lines vs unique API calls
JSONL=~/.claude/projects/*/sessions/$(ls -t ~/.claude/projects/*/sessions/ | head -1)/session.jsonl
echo "Total assistant lines:"
grep -c '"type":"assistant"' "$JSONL" 2>/dev/null || echo "0"
echo "Unique message.id:requestId pairs:"
grep -o '"id":"msg_[^"]*".*"requestId":"[^"]*"' "$JSONL" 2>/dev/null | sort -u | wc -l
```

If these numbers differ, the dedup is working. The unique pair count should roughly match ccusage's API call count for the same session.

**Step 3: Build and smoke-test the server**

```bash
cargo build -p claude-view-server
```

Confirm it compiles. If a local DB exists, start the server briefly to verify re-indexing triggers:

```bash
RUST_LOG=warn,claude_view_server=info,claude_view_db=info cargo run -p claude-view-server &
sleep 5
# Check logs for "re-indexing" messages indicating version bump triggered re-parse
kill %1
```

**Step 4: Commit any test fixes needed**

---

## Known Limitations

1. **`compute_primary_model` weights by turn count, not API call count.** Duplicate content block turns (zeroed tokens) still vote in model selection. In practice this is harmless since sessions typically use one model, and when mixed, the weighting is already approximate. Fixing this would require either filtering turns by `input_tokens != 0` (fragile) or adding a field to `RawTurn` (too invasive for this change).

2. **`turn_count` in the DB** is `min(user_count, assistant_count)` — `assistant_count` is still incremented per JSONL line (not per API call) because it's used for `seq` numbering in turns. However, `turn_count` is correct because it's clamped to `user_count`, which is unaffected by the content block splitting. The `api_call_count` column (now deduped) is the correct metric for "number of API calls."

## Out of Scope (tracked separately)

1. **Subagent JSONL discovery** — Extending `scan_files()` in `crates/db/src/indexer.rs` to find `{session}/subagents/agent-*.jsonl`. This is a data-coverage enhancement, not a correctness fix. The dedup via `message.id:requestId` already handles subagent tokens correctly IF they're indexed.

2. **`sonnet-4-6` model pricing** — The pricing table in `crates/core/src/pricing.rs` already has `claude-sonnet-4-6`. The model only appears in worktree sessions that need subagent discovery (see #1).

3. **Cost display UI** — No frontend changes needed. The cost calculation in `crates/server/src/routes/stats.rs` sums from the sessions table, which will be correct after re-indexing.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 5 Step 2 referenced nonexistent test `test_deep_index_new_session` | Minor | Changed to `test_pass_2_fills_deep_fields` (line 3388 of indexer_parallel.rs) |
