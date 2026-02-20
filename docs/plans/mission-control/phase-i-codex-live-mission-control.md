---
status: pending
date: 2026-02-16
phase: I
depends_on: G
---

# Phase I: Codex Mission Control Live Monitoring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add robust real-time Codex session monitoring to Mission Control without regressing Claude live behavior.

**Architecture:** Extend live pipeline to be source-aware end-to-end: source-tagged file events, Codex-tail parsing, source-aware process matching, unified live session state model, and frontend rendering that supports mixed-source live grids.

**Tech Stack:** Rust live manager (`notify`, `tokio`, `axum SSE`), `sysinfo`, React hooks/components in `src/components/live/*`.

---

## Scope

- In scope:
  - Codex file watching + startup scan
  - Codex live line parsing + status derivation
  - Mixed-source SSE payloads and frontend handling
- Out of scope:
  - Interactive control / PTY ownership
  - Contributions/fluency/insights

## Source-Specific Live Assumptions

- Claude source of truth: top-level JSONL `type=user|assistant|system|progress|summary`.
- Codex source of truth: `response_item` + `event_msg` stream with `task_started`/`task_complete`/`token_count` signals.
- Both sources share same Mission Control contract: `LiveSession` snapshots + `SessionEvent` broadcasts.

---

### Task 1: Implement Codex Incremental Live Parser

**Files:**
- Create: `crates/core/src/codex/live_parser.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/src/codex/live_parser.rs`

**Step 1: Write failing parser tests**

Add tests for incremental parse behavior:
- Reads only appended bytes
- Extracts user/assistant previews
- Detects task lifecycle events (`task_started`, `task_complete`)
- Extracts latest token snapshot

```rust
#[test]
fn parse_tail_codex_reads_only_new_lines() { ... }

#[test]
fn parse_tail_codex_detects_task_state_transitions() { ... }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core codex::live_parser::tests -- --nocapture`
Expected: FAIL because module is missing.

**Step 3: Implement parser + line model**

Define:

```rust
pub enum CodexLiveLineType {
    User,
    Assistant,
    ToolCall,
    ToolResult,
    TaskStarted,
    TaskComplete,
    TokenCount,
    Other,
}

pub struct CodexLiveLine {
    pub line_type: CodexLiveLineType,
    pub timestamp: Option<String>,
    pub content_preview: String,
    pub model: Option<String>,
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
}
```

Use tolerant extraction against `response_item` and `event_msg` payloads.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-core codex::live_parser::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/codex/live_parser.rs crates/core/src/lib.rs
git commit -m "feat(core): add codex incremental live tail parser"
```

---

### Task 2: Make File Watcher/Initial Scan Multi-Root and Source-Tagged

**Files:**
- Modify: `crates/server/src/live/watcher.rs`
- Modify: `crates/server/src/live/manager.rs`
- Test: `crates/server/src/live/watcher.rs`

**Step 1: Write failing watcher tests**

Add tests for:
- Initial scan across both Claude and Codex roots
- File events include source tag
- Codex date-tree recursion works

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server live::watcher::tests -- --nocapture`
Expected: FAIL because watcher only monitors `~/.claude/projects`.

**Step 3: Implement source-tagged file events**

Update model:

```rust
pub enum FileEvent {
    Modified { source: SessionSource, path: PathBuf },
    Removed { source: SessionSource, path: PathBuf },
}
```

- Register notify watchers for Claude + Codex roots.
- Update initial scan to return `(source, path)` tuples.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server live::watcher::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/live/watcher.rs crates/server/src/live/manager.rs
git commit -m "refactor(live): support source-tagged watcher events for claude and codex"
```

---

### Task 3: Generalize Process Detection to Multiple Sources

**Files:**
- Modify: `crates/server/src/live/process.rs`
- Modify: `crates/server/src/live/manager.rs`
- Test: `crates/server/src/live/process.rs`

**Step 1: Write failing process tests**

Add tests for process classification:
- Claude process classification unchanged
- Codex process detection by process name/cmd args
- `find_process_for_project` supports source-aware lookup

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server live::process::tests -- --nocapture`
Expected: FAIL because process struct and detection are Claude-only.

**Step 3: Implement source-aware process model**

- Rename `ClaudeProcess` to generic `LiveAgentProcess`.
- Add `source: SessionSource` field.
- Detection heuristics:
  - Claude: existing rules
  - Codex: process name/args contains `codex` / `codex-cli`

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server live::process::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/live/process.rs crates/server/src/live/manager.rs
git commit -m "feat(live): add source-aware process detection for codex and claude"
```

---

### Task 4: Extend Live Session State Model with Source + Codex Status Mapping

**Files:**
- Modify: `crates/server/src/live/state.rs`
- Modify: `crates/server/src/live/manager.rs`
- Modify: `crates/server/src/routes/live.rs`
- Test: `crates/server/src/live/state.rs`

**Step 1: Write failing state derivation tests**

Add tests covering Codex-specific transitions:
- `task_started` -> `Working`
- `task_complete` with recent activity -> `Paused`
- stale no process -> `Done`

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server live::state::tests::codex_* -- --nocapture`
Expected: FAIL (no codex mapping logic).

**Step 3: Implement state model updates**

- Add `source` and `source_session_id` to `LiveSession`.
- Route `process_jsonl_update` by source parser:
  - Claude path unchanged
  - Codex path uses `codex::live_parser`
- Add Codex status derivation helper, then map to existing `SessionStatus`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server live::state::tests::codex_* -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/live/state.rs crates/server/src/live/manager.rs crates/server/src/routes/live.rs

git commit -m "feat(live): add codex source fields and status derivation"
```

---

### Task 5: Update SSE + Frontend Live Hook for Mixed Sources

**Files:**
- Modify: `src/components/live/use-live-sessions.ts`
- Modify: `src/components/live/types.ts`
- Modify: `src/components/live/SessionCard.tsx`
- Modify: `src/pages/MissionControlPage.tsx`
- Test: `src/components/live/use-live-sessions.test.ts`

**Step 1: Write failing frontend tests**

Add tests ensuring:
- live session payload with `source='codex'` hydrates correctly
- session cards display source badge
- filters/sorting still stable across mixed sources

**Step 2: Run test to verify it fails**

Run: `npm test -- use-live-sessions SessionCard --runInBand`
Expected: FAIL due to unknown source fields in typings/UI.

**Step 3: Implement frontend support**

- Extend `LiveSession` TS type with source metadata.
- Add source pill on live cards (small, non-intrusive).
- Keep existing needs-you/autonomous summary semantics unchanged.

**Step 4: Run test to verify it passes**

Run: `npm test -- use-live-sessions SessionCard MissionControlPage --runInBand`
Expected: PASS.

**Step 5: Commit**

```bash
git add src/components/live/use-live-sessions.ts src/components/live/types.ts src/components/live/SessionCard.tsx src/pages/MissionControlPage.tsx src/components/live/use-live-sessions.test.ts

git commit -m "feat(ui-live): support mixed-source mission control sessions"
```

---

### Task 6: Live Route Contract and Kill Semantics Validation

**Files:**
- Modify: `crates/server/src/routes/live.rs`
- Modify: `crates/server/src/live/manager.rs`
- Test: `crates/server/src/routes/live.rs`

**Step 1: Write failing route tests**

Add tests for:
- `/api/live/sessions` includes codex sessions
- `/api/live/sessions/:id/messages` parses Codex sessions
- `/api/live/sessions/:id/kill` behavior for Codex process PID

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server routes::live::tests::codex_* -- --nocapture`
Expected: FAIL on parser dispatch and/or payload shape.

**Step 3: Implement route-level dispatch and guardrails**

- Message route dispatches by `session.source` to parser family.
- Keep kill endpoint generic if PID exists; no source branching in syscall path.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server routes::live::tests::codex_* -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/routes/live.rs crates/server/src/live/manager.rs

git commit -m "test(live): validate codex route contracts and kill semantics"
```

---

## Exit Criteria

- Mission Control can show Claude and Codex live sessions simultaneously.
- Codex sessions emit `session_discovered`, `session_updated`, and `summary` correctly.
- Live session messages endpoint works for Codex sessions.

## Verification Checklist

Run:
- `cargo test -p claude-view-core codex::live_parser::tests`
- `cargo test -p claude-view-server live::watcher::tests live::process::tests live::state::tests::codex_* routes::live::tests::codex_*`
- `npm test -- use-live-sessions SessionCard MissionControlPage --runInBand`

Expected:
- Mixed-source live path green.
- No regressions in Claude-only live tests.

## Risks and Mitigations

- Risk: Codex event ordering differences cause status flapping.
  - Mitigation: add hysteresis rules (`seconds_since_modified` + process signal) before `Done` transitions.
- Risk: recursive Codex watching increases filesystem event volume.
  - Mitigation: extension filtering (`.jsonl`) + per-source debounce in watcher callback.

