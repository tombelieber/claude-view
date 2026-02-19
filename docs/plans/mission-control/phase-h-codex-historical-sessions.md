---
status: pending
date: 2026-02-16
phase: H
depends_on: G
---

# Phase H: Codex Historical Chat Sessions Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Parse and index Codex session history so Codex conversations appear as first-class records in Sessions list and Conversation view.

**Architecture:** Implement Codex-specific discovery + parsing adapters, route them through the source-aware foundation from Phase G, and preserve existing Claude behavior by dispatching by `session.source`.

**Tech Stack:** Rust (`serde_json`, `tokio`, `sqlx`), SQLite, React query hooks, existing `/api/sessions/*` endpoints.

---

## Scope

- In scope:
  - Codex historical session discovery from `~/.codex/sessions/YYYY/MM/DD/*.jsonl`
  - Pass1 + Pass2 indexing for Codex sessions
  - `/api/sessions/:id/parsed` + `/api/sessions/:id/messages` for Codex sessions
  - Source-aware UX in sessions list + conversation header
- Out of scope:
  - Live Mission Control Codex stream/status (Phase I)
  - Contributions/fluency/insights extraction

## Codex Schema Baseline (Validated)

Observed top-level event types:
- `session_meta`
- `response_item`
- `event_msg`
- `turn_context`
- `compacted`

Key payload variants to support:
- `response_item.payload.type = message|function_call|function_call_output|reasoning|custom_tool_call|custom_tool_call_output|web_search_call`
- `event_msg.payload.type = user_message|agent_message|token_count|task_started|task_complete|turn_aborted`

---

### Task 1: Implement Codex Pass1 Discovery

**Files:**
- Create: `crates/core/src/codex/discovery.rs`
- Modify: `crates/core/src/provider/codex.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/src/codex/discovery.rs`

**Step 1: Write failing discovery tests**

Create fixture tests for:
- Recursive date tree scan
- Session ID extraction from first `session_meta` line
- CWD extraction from `session_meta.payload.cwd`
- Graceful handling when `session_meta` missing

```rust
#[test]
fn discovers_codex_jsonl_files_in_date_tree() { ... }

#[test]
fn extracts_source_session_id_from_session_meta() { ... }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-core codex::discovery::tests -- --nocapture`
Expected: FAIL because module/functions are missing.

**Step 3: Implement discovery module**

Add:

```rust
pub struct CodexDiscoveredSession {
    pub source_session_id: String,
    pub file_path: PathBuf,
    pub cwd: Option<String>,
    pub started_at: Option<i64>,
}

pub fn discover_codex_sessions(root: &Path) -> Result<Vec<CodexDiscoveredSession>, DiscoveryError> { ... }
```

Rules:
- Scan all `*.jsonl` files under root recursively.
- Prefer `session_meta.payload.id` as source session ID.
- Fallback to filename suffix UUID if metadata is missing.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-core codex::discovery::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/codex/discovery.rs crates/core/src/provider/codex.rs crates/core/src/lib.rs
git commit -m "feat(core): add codex historical session discovery"
```

---

### Task 2: Implement Codex Historical Parser to Unified Message Model

**Files:**
- Create: `crates/core/src/codex/parser.rs`
- Modify: `crates/core/src/types.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/src/codex/parser.rs`

**Step 1: Write failing parser tests**

Add parser tests for message mapping:
- `response_item/message` user -> `Role::User`
- `response_item/message` assistant output -> `Role::Assistant`
- `response_item/function_call` -> `Role::ToolUse`
- `response_item/function_call_output` -> `Role::ToolResult`
- token aggregation from final `event_msg/token_count`

```rust
#[tokio::test]
async fn maps_response_item_message_roles() { ... }

#[tokio::test]
async fn maps_function_calls_to_tool_messages() { ... }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-core codex::parser::tests -- --nocapture`
Expected: FAIL because parser does not exist.

**Step 3: Implement parser**

Core API:

```rust
pub async fn parse_codex_session(file_path: &Path) -> Result<ParsedSession, ParseError> { ... }
pub async fn parse_codex_session_paginated(file_path: &Path, limit: usize, offset: usize) -> Result<PaginatedMessages, ParseError> { ... }
```

Mapping policy:
- `response_item/message role=user` -> `Role::User`
- `response_item/message role=assistant` with `output_text` blocks -> `Role::Assistant`
- `response_item/message role=developer` -> `Role::System` (metadata-preserving)
- `function_call` / `custom_tool_call` -> `Role::ToolUse`
- `function_call_output` / `custom_tool_call_output` -> `Role::ToolResult`
- `event_msg/user_message` only used as fallback if no user message in same turn

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-core codex::parser::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/codex/parser.rs crates/core/src/types.rs crates/core/src/lib.rs
git commit -m "feat(core): parse codex sessions into unified message model"
```

---

### Task 3: Add Codex Deep-Metadata Extractor for DB Indexing

**Files:**
- Create: `crates/db/src/codex_indexer.rs`
- Modify: `crates/db/src/indexer_parallel.rs`
- Modify: `crates/db/src/queries/sessions.rs`
- Test: `crates/db/src/codex_indexer.rs`

**Step 1: Write failing deep-metadata tests**

Target fields:
- `user_prompt_count`
- `api_call_count`
- `tool_call_count`
- `primary_model`
- token totals from final `event_msg/token_count`

```rust
#[test]
fn extracts_codex_token_totals_from_final_token_count_event() { ... }

#[test]
fn computes_tool_call_count_from_function_call_items() { ... }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-db codex_indexer::tests -- --nocapture`
Expected: FAIL because extractor module is missing.

**Step 3: Implement extractor + integration hook**

- Add Codex parser path in pass2 deep index pipeline:
  - if `session.source == codex`, call Codex metadata extractor
  - else keep existing Claude parser path
- Do not attempt contributions/LOC heuristics for Codex in this phase.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-db codex_indexer::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/codex_indexer.rs crates/db/src/indexer_parallel.rs crates/db/src/queries/sessions.rs
git commit -m "feat(db): add codex deep metadata extraction in pass2 indexer"
```

---

### Task 4: Wire Pass1/Pass2 Indexing for Codex Provider

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs`
- Modify: `crates/server/src/main.rs`
- Modify: `crates/core/src/provider/mod.rs`
- Test: `crates/db/src/indexer_parallel.rs`

**Step 1: Write failing integration tests**

Integration assertions:
- Codex discovered session inserted with `source='codex'`
- Canonical ID format `codex:<source_session_id>`
- Reindex is idempotent

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-db codex_pass1_pass2_integration -- --nocapture`
Expected: FAIL due to Claude-only indexing path.

**Step 3: Implement provider-driven index loops**

- Replace direct `read_all_session_indexes(claude_dir)` only path with source-aware provider iteration.
- For Codex, pass1 is filesystem discovery (no `sessions-index.json`).
- Persist `source`, `source_session_id`, canonical `id` consistently.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-db codex_pass1_pass2_integration -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs crates/server/src/main.rs crates/core/src/provider/mod.rs

git commit -m "refactor(indexer): run pass1/pass2 through source providers including codex"
```

---

### Task 5: Dispatch Session Parse Endpoints by Source

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`
- Modify: `crates/server/src/routes/turns.rs`
- Modify: `crates/core/src/parser.rs`
- Test: `crates/server/src/routes/sessions.rs`

**Step 1: Write failing API route tests**

Add tests proving:
- `/api/sessions/codex:<id>/parsed` uses Codex parser
- `/api/sessions/codex:<id>/messages` pagination works
- Claude route behavior unchanged

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server codex_session_parsed_route -- --nocapture`
Expected: FAIL because route always calls Claude parser.

**Step 3: Implement source dispatch**

- Query session source with file path.
- If source is codex, call `parse_codex_session*` APIs.
- Keep endpoint shape unchanged for frontend compatibility.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server codex_session_parsed_route -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/routes/sessions.rs crates/server/src/routes/turns.rs crates/core/src/parser.rs
git commit -m "feat(server): dispatch session parse endpoints by source"
```

---

### Task 6: Add Source-Aware Historical UI

**Files:**
- Modify: `src/components/HistoryView.tsx`
- Modify: `src/components/SessionCard.tsx`
- Modify: `src/components/ConversationView.tsx`
- Modify: `src/hooks/use-projects.ts`
- Test: `src/components/HistoryView.test.tsx`

**Step 1: Write failing UI tests**

Add tests for:
- Source badge rendering for Codex sessions
- Search/filter unchanged with mixed Claude/Codex sessions
- Conversation header shows source label

**Step 2: Run test to verify it fails**

Run: `npm test -- HistoryView --runInBand`
Expected: FAIL because source metadata is not rendered.

**Step 3: Implement minimal UI changes**

- Add compact source badge (`Claude` / `Codex`) on cards/table rows.
- Preserve existing sorting/filter defaults.
- Add optional source filter in toolbar only if low risk to ship.

**Step 4: Run test to verify it passes**

Run: `npm test -- HistoryView SessionCard ConversationView --runInBand`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/components/HistoryView.tsx src/components/SessionCard.tsx src/components/ConversationView.tsx src/hooks/use-projects.ts src/components/HistoryView.test.tsx

git commit -m "feat(ui): surface session source metadata for mixed claude/codex history"
```

---

## Exit Criteria

- Codex sessions are discoverable and indexed into `sessions` table with `source='codex'`.
- Codex sessions open correctly in Conversation view via existing endpoints.
- Historical UI can display mixed-source sessions without regressions.

## Verification Checklist

Run:
- `cargo test -p vibe-recall-core codex::discovery::tests codex::parser::tests`
- `cargo test -p vibe-recall-db codex_indexer::tests codex_pass1_pass2_integration`
- `cargo test -p vibe-recall-server codex_session_parsed_route`
- `npm test -- HistoryView SessionCard ConversationView --runInBand`

Expected:
- End-to-end historical read path for Codex passes.
- Claude historical path remains green.

## Risks and Mitigations

- Risk: Codex schema drift (new payload variants).
  - Mitigation: parser uses tolerant matching + preserves unknown event counts in diagnostics.
- Risk: token totals are cumulative snapshots, not deltas.
  - Mitigation: use final snapshot totals for session-level metrics in this phase.

