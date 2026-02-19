---
status: done
date: 2026-02-16
---

# Task Time: Per-Turn Duration Metrics

## Problem

Session cards show wall-clock duration (first message → last message), which includes idle time when the user is reading, thinking, or AFK. A "45 min" session might contain only 12 minutes of actual AI work. This makes duration meaningless for comparing session productivity.

Claude Code's TUI shows the real metric: `"✻ Baked for 52m 6s"` — the time from when you hit Enter to when Claude returns control. We call this **task time**.

## Key Discovery: First-Party Data Already Exists

Claude Code writes `turn_duration` system messages to JSONL after every turn:

```json
{
  "type": "system",
  "subtype": "turn_duration",
  "durationMs": 440846,
  "timestamp": "2026-02-16T05:05:57.387Z"
}
```

This is **Claude API processing time** (thinking + generation), excluding tool execution wall clock. Our deep indexer (`crates/db/src/indexer_parallel.rs`) already parses these messages and aggregates them into `turn_duration_avg_ms`, `turn_duration_max_ms`, and `turn_duration_total_ms` on `SessionInfo`. These fields are populated correctly. What's missing is wall-clock task time (including tool execution) and surfacing the data in the UI.

### Two Clocks

| Clock | What it measures | Source | When they diverge |
|-------|-----------------|--------|-------------------|
| **CC `durationMs`** | Claude thinking + generation time | `turn_duration` system messages | Simple turns: matches wall clock |
| **Wall-clock delta** | Full turn including tool execution | `user.timestamp → last_assistant.timestamp` | Complex turns: wall clock is 2-3x larger |

For a turn with many tool calls (bash, file reads, sub-agents), CC's `durationMs` might be 2m53s while wall clock is 5m53s — the difference is tool execution time.

**Decision: Use wall-clock delta as the primary "task time" metric.** This is what the user experiences — "how long did my task take?" includes the time tools ran. CC's `durationMs` is available as a secondary metric ("Claude thinking time") for power users.

**Bonus: CC's `durationMs` is a useful cross-check.** When wall-clock >> CC durationMs, it means most time was spent on tool execution (long bash commands, large file reads). When they're close, Claude was thinking the whole time.

## Terminology

| Term | Context | Example |
|------|---------|---------|
| **Task time** | Metrics, analytics, column headers | "Longest task: 12m", "Total task time: 38m" |
| **Whimsical verb** | Spinner personality layer | "✶ Noodling… 12m", "· Baked · opus-4.6" |
| **Wall clock** | Session-level time range | "Today 2:30 PM → 3:15 PM" |

The spinner shows `"✶ Noodling… 12m"` (personality), the metrics row says `"longest task: 12m"` (data).

## Information Hierarchy

Three durations, each in its natural visual zone:

```
┌──────────────────────────────────────────────────────────────┐
│  my-project  main   Today 2:30 PM → 3:15 PM                 │  WALL CLOCK (when it happened)
│  "Add authentication to the API"                             │
│  · Baked 38m · opus-4.6                                      │  TOTAL TASK TIME (AI work)
│  5 prompts · 23.1K tokens · longest 12m                      │  LONGEST TASK (complexity)
│  +142 / -28                                                  │
└──────────────────────────────────────────────────────────────┘
```

- **Header** = wall clock time range (contextual — WHEN)
- **Spinner row** = total task time (primary — HOW MUCH work)
- **Metrics row** = longest single task (analytical — complexity signal)

No labels needed. The visual language makes the distinction self-evident: time range in the header is clearly a calendar reference, the spinner verb communicates AI work, and "longest 12m" in the metrics row is a stat.

## Surface Mapping

| Surface | Shows task time? | What it shows |
|---------|-----------------|---------------|
| **Historical SessionCard** | Yes | Spinner: total task time. Metrics: longest task. |
| **CompactSessionTable** | Yes | "Task" column replaces "Dur." — shows total task time |
| **SessionDrillDown** | Yes | Primary stat + per-turn breakdown |
| **Live card (needs_you)** | Yes | Frozen: last completed task time ("✻ Baked 12m 34s") |
| **Live card (autonomous)** | No | Spinner already shows current turn elapsed — sufficient |
| **Live ListView** | No | Operational triage — "is it alive?" not "how much work?" |
| **MonitorPane** | No | Terminal output is the focus |
| **RichPane messages** | No | Per-message timestamps are a different concept |

### The NeedsYou Moment

When a session flips Autonomous → NeedsYou, the spinner freezes and shows the completed task time. This is the exact "✻ Baked for 52m 6s" TUI experience:

```
Autonomous card:           NeedsYou card:
┌──────────────────┐       ┌──────────────────┐
│ ✶ Noodling… 12m  │  →→→  │ ✻ Baked 12m 34s  │
│   (ticking live) │       │   (frozen final)  │
└──────────────────┘       └──────────────────┘
```

## Turn Detection Algorithm

A "turn" = one human prompt → Claude finishes responding (possibly through many tool calls).

**Turn start** = JSONL entry where:
- `type: "user"`
- `message.content` is a `String` (not a list containing `tool_result`)
- Content does NOT start with these system prefixes:
  - `<local-command-caveat>`
  - `<local-command-stdout>`
  - `<command-name>/clear`
  - `<command-name>/context`
- Content is NOT a context continuation (`"This session is being continued..."`)
- Content is NOT a task notification (`<task-notification>`)

**Turn end** = last `type: "assistant"` entry before the next turn start (or EOF).

**Task time** = `turn_end.timestamp - turn_start.timestamp` (wall-clock delta).

**Context continuations** are NOT new turns. Claude Code's own `turn_duration` merges them into the parent turn. Our wall-clock computation should do the same — skip continuation messages as turn boundaries.

**Task notifications** (`<task-notification>`) are borderline. They trigger a mini-turn where Claude processes the sub-agent result. CC emits a `turn_duration` for them. Include them as turns but they'll naturally be short (30-60s).

### Implementation Note: Live Parser Gaps

The live parser (`crates/core/src/live_parser.rs`) currently:
- **CAN** distinguish `type: "user"` / `"assistant"` / `"system"` via `LineType` enum
- **CAN** detect `isMeta: true` messages
- **CAN** extract `content_preview` and `timestamp`
- **CANNOT** distinguish `message.content` as String vs Array (flattens both to `content_preview: String`)
- **CANNOT** detect `tool_result` blocks in user content arrays
- **CANNOT** filter system prefix patterns (`<local-command-caveat>`, etc.)

To support turn detection, `LiveLine` needs two new fields:
```rust
pub is_tool_result_continuation: bool,  // content was Array with tool_result blocks
pub has_system_prefix: bool,            // content starts with system prefix pattern
```

These must be computed in `parse_single_line()` (lines 146-272 of `live_parser.rs`) and `extract_content_and_tools()` (lines 275-309).

## Data Model

### Historical Sessions — Add Wall-Clock Task Time Fields

The CC API processing time fields `turn_duration_avg_ms` and `turn_duration_max_ms` already exist on `SessionInfo` and are populated by `crates/db/src/indexer_parallel.rs` (lines 820-823, 1907-1914). Note: `turn_duration_total_ms` is computed and stored in SQLite but is NOT mapped to `SessionInfo` — the `into_session_info()` conversion in `crates/db/src/queries/row_types.rs` drops it. This is fine for our purposes since we compute wall-clock task time independently.

What's missing is **wall-clock task time** — the time including tool execution, which is what the user experiences.

**New fields on `SessionInfo`:**

```rust
// Wall-clock task time (what we display)
#[serde(default, skip_serializing_if = "Option::is_none")]
#[ts(type = "number | null")]
pub total_task_time_seconds: Option<u32>,    // sum of all turn wall-clock durations
#[serde(default, skip_serializing_if = "Option::is_none")]
#[ts(type = "number | null")]
pub longest_task_seconds: Option<u32>,       // single longest turn (wall clock)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub longest_task_preview: Option<String>,    // first 60 chars of the prompt that started it

// CC's API processing time (secondary metric) — already on SessionInfo, already populated
// turn_duration_avg_ms: Option<u64>   — DO NOT ADD (exists)
// turn_duration_max_ms: Option<u64>   — DO NOT ADD (exists)
// turn_duration_total_ms              — in SQLite only, NOT on SessionInfo (intentional)
```

### Live Sessions — 2 New Fields on `LiveSession` + 1 on `SessionAccumulator`

The JSONL watcher tracks turn boundaries in real-time. Fields to add:

**`SessionAccumulator` (crates/server/src/live/manager.rs, lines 37-96):**
```rust
current_turn_started_at: Option<i64>,    // set when human text message arrives
```

**`LiveSession` (crates/server/src/live/state.rs, lines 64-106):**
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub current_turn_started_at: Option<i64>,    // set when human text message arrives
#[serde(skip_serializing_if = "Option::is_none")]
pub last_turn_task_seconds: Option<u32>,     // set when turn completes (autonomous → needs_you)
```

Both fields automatically appear in SSE payloads via Serde `#[derive(Serialize)]` with `#[serde(rename_all = "camelCase")]` — Rust `current_turn_started_at` becomes `currentTurnStartedAt` in JSON/TypeScript.

**Lifecycle:**
1. Watcher sees human text message (filtered via turn detection) → set `acc.current_turn_started_at = timestamp`
2. Session transitions Working → Paused + NeedsYou (in `handle_status_change()`, line 753) → compute `last_turn_task_seconds = last_activity - current_turn_started_at`
3. Session transitions back to Working (new prompt) → reset `current_turn_started_at`, clear `last_turn_task_seconds`

**Frontend uses:**
- Autonomous card spinner: `currentTime - current_turn_started_at` (current task elapsed)
- NeedsYou card: show `last_turn_task_seconds` as frozen final task time

**Current state of NeedsYou spinner:** Currently shows "● Awaiting input" with **no duration at all** (SessionSpinner.tsx lines 114-121). The plan adds a duration display: `"✻ Baked 12m 34s"`.

**Current state of Autonomous spinner:** Currently shows `currentTime - session.startedAt` (session elapsed, line 63 of live/SessionCard.tsx). Must change to `currentTime - current_turn_started_at` (current TASK elapsed).

### New API Endpoint — Per-Turn Breakdown

```
GET /api/sessions/{id}/turns
```

Returns on-demand by re-parsing the JSONL (fast — SIMD pre-filter for `"type":"user"` and `"type":"system"` lines):

```json
[
  {
    "index": 1,
    "startedAt": 1708052316,
    "wallClockSeconds": 441,
    "ccDurationMs": 440846,
    "promptPreview": "with the latest fix with @docs/plans..."
  },
  {
    "index": 2,
    "startedAt": 1708052825,
    "wallClockSeconds": 777,
    "ccDurationMs": 776998,
    "promptPreview": "ok fix it"
  }
]
```

Both wall-clock and CC duration included — lets the UI show tool execution overhead when expanded.

## Implementation Steps

### Step Dependencies

```
Step 1 (live parser) ──→ Step 3 (live watcher) ──→ Step 8 (live frontend)
                    └──→ Step 9b (parser tests)
Step 2 (deep indexer) ─→ Step 4 (turns endpoint) ─→ Step 5, 6, 7 (historical frontend)
                    └──→ Step 9c (indexer tests) └──→ Step 9e (endpoint tests)
Step 2a (migration) ──→ Step 9d (migration test)
```

- **Steps 1 and 2** are independent — can be done in parallel
- **Step 3** depends on Step 1 (needs `is_tool_result_continuation`, `has_system_prefix` on `LiveLine`)
- **Step 4** depends on Step 2 (needs the turn detection algorithm, shares code)
- **Steps 5, 6, 7, 8** are independent frontend changes — can be done in parallel once backend steps are complete
- **Step 9** (tests) should be done alongside or immediately after backend steps — Step 9a is a BLOCKER that must be done WITH Step 1

### Step 1: Extend live parser with turn detection fields (backend)

**Modify:** `crates/core/src/live_parser.rs`

The live parser flattens `message.content` (String vs Array) into `content_preview: String`, losing the distinction needed for turn detection. Add:

1. Add `is_tool_result_continuation: bool` and `has_system_prefix: bool` to `LiveLine` struct (line 21), defaulting to `false`
2. In `extract_content_and_tools()` (lines 275-309), change return type from `(String, Vec<String>)` to `(String, Vec<String>, bool)` and return an additional `bool` for tool_result detection:
   ```rust
   // In the Array branch of the content match:
   Some(serde_json::Value::Array(blocks)) => {
       let mut has_tool_result = false;
       for block in blocks {
           match block.get("type").and_then(|t| t.as_str()) {
               Some("text") => { /* existing logic */ }
               Some("tool_use") => { /* existing logic */ }
               Some("tool_result") => { has_tool_result = true; }
               _ => {}
           }
       }
       // Return has_tool_result alongside existing (preview, tool_names)
   }
   ```
   **Also update the call site** at line 203 of `parse_single_line()`:
   ```rust
   // BEFORE:
   let (content_preview, tool_names) = extract_content_and_tools(content_source, finders);
   // AFTER:
   let (content_preview, tool_names, is_tool_result) = extract_content_and_tools(content_source, finders);
   ```
3. In `parse_single_line()`, compute `has_system_prefix` **AFTER** the `extract_content_and_tools()` call (line ~203), since `content_preview` is not available before that point:
   ```rust
   let has_system_prefix = if line_type == LineType::User {
       let c = content_preview.trim_start();
       c.starts_with("<local-command-caveat>")
           || c.starts_with("<local-command-stdout>")
           || c.starts_with("<command-name>/clear")
           || c.starts_with("<command-name>/context")
           || c.starts_with("This session is being continued")
           || c.starts_with("<task-notification>")
   } else {
       false
   };
   ```
4. **Update ALL `LiveLine` construction sites** — there are 3 in total:
   - **Error fallback path** in `parse_single_line()` (line 167 of `live_parser.rs`): returns early-constructed `LiveLine` on JSON parse failure. Add `is_tool_result_continuation: false, has_system_prefix: false`.
   - **Normal parse path** in `parse_single_line()` (line 257 of `live_parser.rs`): final `LiveLine` construction. Set from computed values.
   - **Test helper** `make_live_line()` in `crates/server/src/live/state.rs` (line 237): used by 27 tests. Add `is_tool_result_continuation: false, has_system_prefix: false`.

### Step 2: Add wall-clock task time computation to deep indexer (backend)

**Modify:** `crates/core/src/types.rs`, `crates/db/src/migrations.rs`, `crates/db/src/queries/row_types.rs`, `crates/db/src/indexer_parallel.rs`

CC's `turn_duration` aggregation (avg/max/total) is ALREADY implemented (lines 820-823, 1907-1918). What's missing is **wall-clock per-turn durations**.

**Sub-step 2a: Schema changes**
1. Add new fields to `SessionInfo` in `crates/core/src/types.rs`: `total_task_time_seconds`, `longest_task_seconds`, `longest_task_preview`

   **CRITICAL: `SessionInfo` does NOT derive `Default`.** Adding 3 new fields breaks **30+ construction sites** across 15+ files. Every `SessionInfo { ... }` struct literal must be updated with `total_task_time_seconds: None, longest_task_seconds: None, longest_task_preview: None`. This is mechanical but must not be skipped or compilation fails everywhere.

   **Affected files (add `total_task_time_seconds: None, longest_task_seconds: None, longest_task_preview: None` to every `SessionInfo { ... }` literal):**
   - `crates/core/src/types.rs` — 3 test sites (~lines 999, 1122-1123)
   - `crates/core/src/discovery.rs` — 2 sites (~lines 529, 1326)
   - `crates/core/src/patterns/mod.rs` — 2 sites (~lines 177, 244)
   - `crates/core/examples/debug_json.rs` — 1 site (~line 4)
   - `crates/db/src/indexer.rs` — 1 site (~line 254)
   - `crates/db/src/queries/row_types.rs` — 1 site (~line 570, already mentioned below)
   - `crates/db/src/trends.rs` — 3 sites (~lines 872, 941, 951)
   - `crates/db/tests/queries_shared.rs` — 1 site (~line 5)
   - `crates/db/tests/queries_sessions_test.rs` — 3 sites (~lines 56, 114, 120)
   - `crates/db/tests/queries_dashboard_test.rs` — 9 sites
   - `crates/server/src/routes/stats.rs` — 7 sites
   - `crates/server/src/routes/projects.rs` — 8 sites
   - `crates/server/src/routes/sessions.rs` — 1 site (~line 559)
   - `crates/server/src/routes/export.rs` — 1 site (~line 245)
   - `crates/server/src/routes/invocables.rs` — 1 site (~line 166)
   - `crates/server/src/routes/insights.rs` — 1 site (~line 304)

   **Frontend test mocks** (add `totalTaskTimeSeconds: null, longestTaskSeconds: null, longestTaskPreview: null` — camelCase because ts-rs generates camelCase):
   - `src/components/CompactSessionTable.test.tsx` — line 8: full `SessionInfo` literal (`mockSession`)
   - `src/components/SessionCard.test.tsx` — line 7: `createMockSession` base object (lists all fields)
   - `src/utils/group-sessions.test.ts` — line 7: `makeSession` base object
   - `src/components/ActivityCalendar.test.tsx` — line 7: `createMockSession` base object

   **Tip (Rust):** Run `cargo build 2>&1 | grep "missing field"` to find any you missed.
   **Tip (TypeScript):** Run `bun run typecheck` after ts-rs regeneration to find any missing mock fields.

2. Add SQLite migration to `crates/db/src/migrations.rs` — append to the `MIGRATIONS` array:
   ```sql
   ALTER TABLE sessions ADD COLUMN total_task_time_seconds INTEGER;
   ALTER TABLE sessions ADD COLUMN longest_task_seconds INTEGER;
   ALTER TABLE sessions ADD COLUMN longest_task_preview TEXT;
   ```
   Without this, all INSERT/UPDATE statements referencing these columns will fail with "no such column".
3. Add corresponding fields to `SessionRow` struct in `crates/db/src/queries/row_types.rs`
4. Update `FromRow` impl for `SessionRow` to `try_get` the new columns
5. Update `into_session_info()` (line ~559 of `row_types.rs`) to map new `SessionRow` fields → `SessionInfo` fields

**Sub-step 2b: Computation**

**Important:** The deep indexer does NOT use `LiveLine` or `live_parser.rs` — it has its own `memmem`-based scanning in `parse_bytes()`. Turn detection must be implemented within the deep indexer's existing scan loop. **Bonus:** `is_system_user_content()` already exists at line 1423 and filters the exact same system prefix patterns — reuse it for turn detection.

6. Add turn tracking fields to `ExtendedMetadata` struct (line 130, after `first_user_prompt` at line 199):
   ```rust
   // Turn tracking for wall-clock task time
   current_turn_start_ts: Option<i64>,        // timestamp of current turn start
   current_turn_prompt: Option<String>,        // first 60 chars of prompt
   longest_task_seconds: Option<u32>,
   longest_task_preview: Option<String>,
   total_task_time_seconds: u32,
   ```
   `ExtendedMetadata` derives `Default` (line 130) so these auto-initialize to `None`/`0`.

7. In the `"user"` branch of the type match (line 774), **after** the existing content extraction (line 783), add turn boundary detection:
   ```rust
   // Turn detection: close previous turn and start new one
   if let Some(content) = extract_first_text_content(line, &content_finder, &text_finder) {
       if !is_system_user_content(&content) {  // reuse existing filter at line 1423
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
           result.deep.current_turn_start_ts = /* parsed timestamp from this line */;
           result.deep.current_turn_prompt = Some(content.chars().take(60).collect());
       }
   }
   ```
   **Also close the final turn at EOF** (after the line-by-line loop ends, before aggregation at ~line 1900) using `result.deep.last_timestamp` as the end timestamp.
8. Aggregate into the new SessionInfo fields alongside existing turn_duration aggregation (lines 1907-1914)
9. Add corresponding columns to the `INSERT`/`UPDATE` SQL statements. **Critical coupling:**
   - The SQL constant `UPDATE_SESSION_DEEP_SQL` (line 31) and the `rusqlite::params!` macro (line 1929) must be updated in lockstep
   - The comment at line 28 says "Must match the sqlx `_tx` function in queries.rs exactly (51 params)" — update to 54 params
   - There is also an sqlx-based fallback path (~line 2050) that also needs the new columns

**Sub-step 2c: Re-index trigger**
10. Bump `parse_version` from 5 → 6 (line 24 of `indexer_parallel.rs`). Add comments for undocumented versions:
    ```rust
    // Version 1-3: documented at lines 20-22
    // Version 4: (add brief description of what v4 added if known, else "undocumented")
    // Version 5: (add brief description of what v5 added if known, else "undocumented")
    // Version 6: Wall-clock task time fields (total_task_time_seconds, longest_task_seconds)
    pub const CURRENT_PARSE_VERSION: i32 = 6;
    ```
   **Performance note:** This causes ALL sessions (~2700+) to be re-indexed on next startup, which may take several minutes. The indexing runs in the background and the server starts immediately (per project rules).

### Step 3: Add turn tracking to live JSONL watcher (backend)

**Modify:** `crates/server/src/live/manager.rs` + `crates/server/src/live/state.rs`

**Depends on:** Step 1 (live parser fields must exist)

1. Add `current_turn_started_at: Option<i64>` to `SessionAccumulator` (struct at lines 37-96) and initialize to `None` in `SessionAccumulator::new()` (lines 71-96)
2. Add `current_turn_started_at: Option<i64>` and `last_turn_task_seconds: Option<u32>` to `LiveSession` (struct at lines 64-106 of `state.rs`)
3. In the JSONL processing loop (`process_jsonl_update()`, lines 503-734), when a user message is detected (line 606), apply the full turn detection filter. **Note:** `parse_timestamp_to_unix()` takes `&str`, not `String`, so use `if let` with `ref` (matching the existing pattern at manager.rs line 619-621):
   ```rust
   if line.line_type == LineType::User
       && !line.is_meta
       && !line.is_tool_result_continuation
       && !line.has_system_prefix
   {
       if let Some(ref ts) = line.timestamp {
           acc.current_turn_started_at = parse_timestamp_to_unix(ts);
       }
   }
   ```
4. In `handle_status_change()` (lines 739-832), when Working → Paused:
   - If `acc.current_turn_started_at` is set, compute `last_turn_task_seconds = last_activity_at - current_turn_started_at`
   - Copy to `LiveSession.last_turn_task_seconds`
5. When Working resumes (new prompt detected), reset `current_turn_started_at`, clear `last_turn_task_seconds`
6. **Update `LiveSession` construction site** in `process_jsonl_update()` (lines 692-713 of `manager.rs`). This struct literal lists every field explicitly — add:
   ```rust
   current_turn_started_at: acc.current_turn_started_at,
   last_turn_task_seconds: None, // populated in handle_status_change
   ```

### Step 4: Add `/api/sessions/{id}/turns` endpoint (backend)

**New file:** `crates/server/src/routes/turns.rs`
**Modify:** `crates/server/src/routes/mod.rs` (add `pub mod turns;` and register route)

1. Create handler following codebase conventions (see `contributions.rs` line 467 for reference):
   ```rust
   pub async fn get_session_turns(
       State(state): State<Arc<AppState>>,   // Arc wrapper required — all routes use Arc<AppState>
       Path(session_id): Path<String>,       // Convention: State first, then Path
   ) -> ApiResult<impl IntoResponse>         // ApiResult for ? operator on DB queries
   ```
2. Locate the JSONL file via the DB-mediated pattern (see `sessions.rs` lines 390-410 for reference):
   ```rust
   let file_path = state.db.get_session_file_path(&session_id).await?
       .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;
   let path = std::path::PathBuf::from(&file_path);
   if !path.exists() {
       return Err(ApiError::SessionNotFound(session_id));
   }
   ```
   **Note:** There is NO `sessions_dir` field on `AppState`. File paths are always resolved via `state.db.get_session_file_path()`.
3. Parse JSONL on demand. Two options:
   - **Fast path (preferred):** Write a new scanning function using `memmem::Finder` to pre-filter for `"type":"user"` and `"type":"system"` lines before JSON deserialization (like `live_parser.rs` lines 52-83). Most lines are assistant/tool messages that don't need parsing.
   - **Simple path:** Use existing `vibe_recall_core::parse_session(&path)` which deserializes every line, then post-filter. Simpler but slower for large sessions.
4. Return per-turn data with both wall-clock and CC duration
5. Create `pub fn router() -> Router<Arc<AppState>>` function and register via `.nest("/api", turns::router())` pattern (see lines 84-108 of `mod.rs`)

### Step 5: Update historical SessionCard (frontend)

**Modify:** `src/components/SessionCard.tsx` + `src/components/spinner/SessionSpinner.tsx`

Current state: The spinner row (lines 240-249 of SessionCard.tsx) renders `<SessionSpinner mode="historical" ... />` which shows model + past-tense verb (e.g., `"· Baked · opus-4.6"`). Duration is displayed in the **header** time range, NOT in the spinner.

Changes:
1. Add `taskTimeSeconds?: number | null` prop to `HistoricalSpinnerProps` in `SessionSpinner.tsx` (lines 28-31)
2. Update historical spinner format: `"· Baked 38m · opus-4.6"` (insert duration between verb and model)
3. In SessionCard metrics row (lines 251-297 — the prompts/tokens/files display section), add `"longest Xm"` stat using `session.longestTaskSeconds` (camelCase from ts-rs)
4. Header: keep wall-clock time range (unchanged)

### Step 6: Update CompactSessionTable (frontend)

**Modify:** `src/components/CompactSessionTable.tsx`

1. Rename column header: `'Dur.'` → `'Task'` (line 236)
2. Change accessor from `'durationSeconds'` to `'totalTaskTimeSeconds'` (line 234) — camelCase because `SessionInfo` uses `#[serde(rename_all = "camelCase")]` via ts-rs
3. Update display logic (line 242) to use `s.totalTaskTimeSeconds`
4. Fallback: if `totalTaskTimeSeconds` is null/undefined (pre-reindex sessions), fall back to `durationSeconds`

### Step 7: Update SessionDrillDown + contribution API (frontend + backend)

**Modify (backend):** `crates/db/src/snapshots.rs`, `crates/server/src/routes/contributions.rs`
**Modify (frontend):** `src/components/contributions/SessionDrillDown.tsx`

Current state: This component uses `useSessionContribution(sessionId)` hook (imported at line 4, used at line 30) which fetches data from `/api/contributions/sessions/{id}` (see `src/hooks/use-contributions.ts` line 77). The "Duration" stat (lines 84-88) shows `data.duration` from the **contribution API**, not from `SessionInfo`.

Changes:
1. **Backend:** Add `total_task_time_seconds` to the `SessionContribution` struct in `crates/db/src/snapshots.rs` (line ~212) and its SQL query (`get_session_contribution`, line ~1399). **Note:** This query uses `query_as` with a **tuple type** `(String, Option<String>, i64, i64, i64, i64, i64, i64, i64)`, NOT a struct with `FromRow`. Adding a field means: (a) change tuple type to 10 elements, (b) add `total_task_time_seconds` to SELECT, (c) update the `.map()` closure binding
2. **Backend:** Add `total_task_time_seconds` to `SessionContributionResponse` in `crates/server/src/routes/contributions.rs` (line ~191)
3. **Frontend:** Primary stat: show task time instead of wall-clock duration
4. **Frontend:** Add secondary line: "Wall clock: 45 min (84% active)"
5. **Frontend:** Add per-turn breakdown section that calls the new `/api/sessions/{id}/turns` endpoint

### Step 8: Update live SessionCard for NeedsYou state (frontend)

**Modify:** `src/components/live/SessionCard.tsx` + `src/components/spinner/SessionSpinner.tsx`

Current state:
- **Autonomous mode**: Spinner shows `verb… duration` where duration = `currentTime - session.startedAt` (session elapsed, computed at line 63 of live/SessionCard.tsx)
- **NeedsYou mode**: Spinner shows "● Awaiting input" with **no duration at all** (SessionSpinner.tsx lines 114-121)

Changes:
1. **NeedsYou**: Replace "● Awaiting input" with `"✻ Baked 12m 34s"` using `session.last_turn_task_seconds`
2. **Autonomous**: Change elapsed computation with fallback to prevent NaN:
   ```tsx
   const turnStart = session.currentTurnStartedAt ?? session.startedAt ?? currentTime
   const elapsedSeconds = currentTime - turnStart
   ```
   `currentTurnStartedAt` is null when the session was just discovered but no user message has been parsed yet. Fall back to `startedAt` (session-level) then `currentTime` (shows 0s). **Note:** When falling back to `startedAt`, the spinner shows total session elapsed time (same as current behavior) — this is acceptable for the brief window before the first user message is parsed.
3. Update the `LiveSession` TypeScript interface in `src/components/live/use-live-sessions.ts` (lines 7-41) to include both new fields:
   ```tsx
   currentTurnStartedAt?: number | null
   lastTurnTaskSeconds?: number | null
   ```

### Step 9: Tests (backend)

**Depends on:** Steps 1-4 (all backend work complete)

The plan adds ~400 lines of new logic. Without tests, turn detection bugs will silently produce wrong durations. All test infrastructure already exists (`tempfile`, `tokio-test`, `axum-test`, `pretty_assertions` in dev-deps).

**Step 9a: Update existing test helpers (BLOCKER — must be done WITH Step 1)**
Already covered in Step 1 item 4. The `make_live_line()` helper and two `LiveLine` construction sites in `live_parser.rs` must include the new fields or 27+ tests won't compile.

**Step 9b: Live parser turn detection unit tests**
**File:** `crates/core/src/live_parser.rs` (test module at line 346+, which already has 10 tests)

1. Test `is_tool_result_continuation = true` for Array content with `tool_result` blocks
2. Test `is_tool_result_continuation = false` for Array content with only `text`/`tool_use` blocks
3. Test `has_system_prefix = true` for each of the 6 patterns:
   - `<local-command-caveat>...`
   - `<local-command-stdout>...`
   - `<command-name>/clear...`
   - `<command-name>/context...`
   - `"This session is being continued..."`
   - `<task-notification>...`
4. Test `has_system_prefix = false` for normal user messages
5. Edge cases: empty content, missing `message.content` field

**Step 9c: Deep indexer turn detection + fixture validation**
**File:** `crates/db/src/indexer_parallel.rs` (test module)

1. Extend existing `test_golden_complete_session()` (line 3459) to assert:
   - `total_task_time_seconds` is `Some(...)` and > 0
   - `longest_task_seconds` is `Some(...)` and > 0
   - `longest_task_preview` is `Some(...)` and non-empty
   - Wall-clock task time >= CC `turn_duration_total_ms / 1000` (sanity check)
2. Add multi-turn fixture test verifying:
   - Correct turn count (system prefix messages NOT counted as turns)
   - Wall-clock delta computed correctly (turn_end.timestamp - turn_start.timestamp)
   - Longest task is the maximum, not the first or last

**Step 9d: Migration validation test**
**File:** `crates/db/src/migrations.rs` (test module — follow pattern of `test_migration8_sessions_new_columns_exist()` at line 604)

1. New test `test_migration_task_time_columns_exist()` verifying:
   - `total_task_time_seconds` column exists after running all migrations
   - `longest_task_seconds` column exists
   - `longest_task_preview` column exists

**Step 9e: `/api/sessions/{id}/turns` endpoint tests**
**File:** `crates/server/src/routes/turns.rs` (new `#[cfg(test)] mod tests`)

Follow pattern from `crates/server/src/routes/sessions.rs` tests (lines 526-1400):

1. Test 200 response with valid session — assert turn count, field presence
2. Test 404 for non-existent session ID
3. Test empty session (zero turns) — should return empty array `[]`
4. Test response shape: each turn has `index`, `startedAt`, `wallClockSeconds`, `ccDurationMs`, `promptPreview`
5. Test prompt preview truncation at 60 chars

## Rollback Plan

This is a pure additive feature (new fields, new endpoint). No existing behavior is modified. Rollback is straightforward:

1. **Git revert** all commits from this feature branch
2. **Downgrade `parse_version`** back to 5 — sessions will NOT be re-indexed (they keep stale v6 data but the new columns are simply ignored since the frontend won't read them)
3. **SQLite columns persist** — `ALTER TABLE ADD COLUMN` is not reversible in SQLite, but orphaned columns with NULL values cause no harm. They'll be ignored by all queries.
4. **No data migration needed** — the new columns are write-only from the indexer. No existing data is modified.

**Partial rollback** (remove frontend only, keep backend): Revert Steps 5-8 commits. The backend populates the fields silently; the frontend just doesn't display them.

## Files to Modify

| File | Change | Step |
|------|--------|------|
| `crates/core/src/types.rs` | Add `total_task_time_seconds`, `longest_task_seconds`, `longest_task_preview` to `SessionInfo` | 2a |
| `crates/core/src/live_parser.rs` | Add `is_tool_result_continuation` and `has_system_prefix` fields to `LiveLine`; detect content type and system prefix patterns | 1 |
| `crates/db/src/migrations.rs` | Append `ALTER TABLE sessions ADD COLUMN` for 3 new columns | 2a |
| `crates/db/src/queries/row_types.rs` | Add 3 fields to `SessionRow`, update `FromRow` impl, update `into_session_info()` mapping | 2a |
| `crates/db/src/indexer_parallel.rs` | Add wall-clock turn boundary detection and task time computation. Bump `parse_version` 5→6. Add new columns to INSERT/UPDATE SQL. | 2b-c |
| `crates/db/src/snapshots.rs` | Add `total_task_time_seconds` to `SessionContribution` struct + SQL query | 7 |
| `crates/server/src/live/state.rs` | Add `current_turn_started_at`, `last_turn_task_seconds` to `LiveSession` | 3 |
| `crates/server/src/live/manager.rs` | Add `current_turn_started_at` to `SessionAccumulator`. Track turn boundaries. Compute task time on Working→Paused. | 3 |
| `crates/server/src/routes/mod.rs` | Register `turns` module and route | 4 |
| `crates/server/src/routes/contributions.rs` | Add `total_task_time_seconds` to `SessionContributionResponse` | 7 |
| `src/components/SessionCard.tsx` | Pass `taskTimeSeconds` to spinner. Add "longest Xm" to metrics row. | 5 |
| `src/components/spinner/SessionSpinner.tsx` | Add `taskTimeSeconds` prop to historical mode. Insert duration between verb and model. Update NeedsYou mode to show frozen task time. | 5, 8 |
| `src/components/CompactSessionTable.tsx` | "Dur." → "Task" column, use `total_task_time_seconds` with `durationSeconds` fallback | 6 |
| `src/components/contributions/SessionDrillDown.tsx` | Primary stat → task time (from contribution API). Per-turn breakdown (from turns API). | 7 |
| `src/components/live/SessionCard.tsx` | Autonomous: compute elapsed from `currentTurnStartedAt` with fallback. NeedsYou: pass `lastTurnTaskSeconds` to spinner. | 8 |
| `src/components/live/use-live-sessions.ts` | Add `currentTurnStartedAt` and `lastTurnTaskSeconds` to `LiveSession` interface | 8 |

**Auto-regenerated (do not edit manually):**

| File | Trigger |
|------|---------|
| `src/types/generated/SessionInfo.ts` | Run `cargo test` or `cargo build` after modifying `crates/core/src/types.rs` — ts-rs auto-generates from `#[ts(export)]` |

## Files to Create

| File | Purpose |
|------|---------|
| `crates/server/src/routes/turns.rs` | Per-turn breakdown endpoint handler |

## What This Plan Does NOT Do

| Deferred | Why |
|----------|-----|
| Display CC `durationMs` vs wall-clock comparison | Power user feature — add in drill-down later |
| Aggregate task time across sessions for leaderboard | Needs the data first — build after metrics are populated |
| Show task time in live ListView | Operational view, not analytics |
| Show task time in MonitorPane | Terminal output is the focus |
| Backfill existing indexed sessions | Re-index needed — handled by parse_version bump |

## Success Criteria

- Historical session cards show total task time in spinner row (e.g., `"· Baked 38m · opus-4.6"`)
- Historical session metrics show longest task duration (e.g., `"longest 12m"`)
- CompactSessionTable "Task" column shows total task time (falls back to wall-clock for pre-reindex sessions)
- SessionDrillDown shows per-turn breakdown with prompt previews
- Live NeedsYou cards show frozen last task time (e.g., `"✻ Baked 12m 34s"` instead of "● Awaiting input")
- Live Autonomous cards show current task elapsed (not session elapsed)
- `total_task_time_seconds`, `longest_task_seconds`, `longest_task_preview` populated for newly indexed sessions
- New `/api/sessions/{id}/turns` endpoint returns per-turn data
- All existing tests pass (`cargo test -p vibe-recall-core`, `cargo test -p vibe-recall-db`, `cargo test -p vibe-recall-server`)
- New turn detection tests cover all 6 system prefix patterns and tool_result detection
- New endpoint tests cover 200, 404, and empty session cases
- Migration test validates 3 new columns exist

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Plan claimed `turn_duration_avg_ms`/`max_ms` are "always None" — they ARE populated by `indexer_parallel.rs` (lines 820-823, 1907-1918) | Blocker | Corrected Data Model section. Removed Step 1 (already done). Reframed scope: only wall-clock task time is new work. |
| 2 | Plan referenced `crates/core/src/discovery.rs` for deep JSONL parsing — wrong file. Deep parsing is in `crates/db/src/indexer_parallel.rs`. | Blocker | Changed all `discovery.rs` references to `indexer_parallel.rs`. Updated Steps 1-2. |
| 3 | Plan said "Spinner row: show `total_task_time_seconds` instead of `durationSeconds`" — but spinner shows model+verb, NOT duration. Duration is in the header. | Blocker | Rewrote Step 5 to clarify: spinner currently shows `"· Baked · opus-4.6"`. Change is to INSERT duration: `"· Baked 38m · opus-4.6"`. Requires new `taskTimeSeconds` prop on `SessionSpinner`. |
| 4 | Plan said NeedsYou spinner shows frozen duration — it actually shows "● Awaiting input" with NO duration. | Blocker | Rewrote Step 8 to clarify current state. The change is more significant: replacing text-only display with duration-inclusive display. |
| 5 | Plan said SessionDrillDown reads from SessionInfo — it reads from `/api/sessions/{id}/contribution` via `useSessionContribution()` hook. | Blocker | Rewrote Step 7 to reference contribution API. Added note that contribution endpoint must also return `total_task_time_seconds`. |
| 6 | Live parser lacks content type detection (String vs Array/tool_result) and system prefix filtering needed for turn detection. | Blocker | Added new Step 1 (extend live parser). Added "Implementation Note: Live Parser Gaps" section to Turn Detection Algorithm. |
| 7 | `src/types/generated/SessionInfo.ts` is auto-generated via ts-rs — plan implied manual editing. | Warning | Moved to "Auto-regenerated" section in Files to Modify. Added note to run `cargo test`/`cargo build` to trigger codegen. |
| 8 | `SessionAccumulator` needs `current_turn_started_at` field — not mentioned in original plan. | Warning | Added to Step 3 and Data Model section. Referenced exact struct location (manager.rs lines 36-68). |
| 9 | Files to Modify table was missing `live_parser.rs`, `spinner/SessionSpinner.tsx`, `routes/mod.rs`, `use-live-sessions.ts`. | Minor | Added all missing files to the table. |
| 10 | Route registration pattern not specified for new endpoint. | Minor | Added `routes/mod.rs` modification and referenced existing pattern (lines 84-108). |

### Round 2: Adversarial Review Fixes

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 11 | Plan claimed `turn_duration_total_ms` exists on `SessionInfo` — it's in SQLite/SessionRow but NOT mapped to SessionInfo via `into_session_info()`. | Blocker | Corrected Data Model: clarified `turn_duration_total_ms` is in SQLite only, intentionally not on `SessionInfo`. |
| 12 | Missing SQLite migration: 3 new columns need `ALTER TABLE` in `migrations.rs`. Without this, all INSERT/UPDATE fail with "no such column". | Blocker | Added Sub-step 2a with explicit migration SQL. Added `crates/db/src/migrations.rs` to Files to Modify. |
| 13 | Missing read-path wiring: `SessionRow` struct, `FromRow` impl, and `into_session_info()` in `row_types.rs` must be updated or new data is written but never read back. | Blocker | Added Sub-step 2a items 3-5. Added `crates/db/src/queries/row_types.rs` to Files to Modify. |
| 14 | Contribution API not updated: Step 7 says endpoint must return `total_task_time_seconds` but never specifies modifying `SessionContribution` in `snapshots.rs` or `SessionContributionResponse` in `contributions.rs`. | Blocker | Rewrote Step 7 to include both backend files. Added both to Files to Modify. |
| 15 | `LiveLine` struct changes break `make_live_line()` test helper in `state.rs`. | Warning | Added item 4 to Step 1 noting all call sites must be updated. |
| 16 | `tool_result` detection in `extract_content_and_tools()` needed explicit code — plan was too vague for verbatim implementation. | Warning | Added explicit Rust code snippet to Step 1 showing Array branch detection logic. |
| 17 | `parse_version` bump lacked details: current value (5), target (6), version comment, performance implication of full re-index. | Warning | Added Sub-step 2c with version details and performance note (~2700 sessions, several minutes). |
| 18 | `has_system_prefix` must be computed AFTER `extract_content_and_tools()` call (line ~203) since `content_preview` is empty before that point. | Warning | Added explicit ordering note to Step 1 item 3. |
| 19 | Step 3 missing complete filter expression for turn detection in `process_jsonl_update()`. | Warning | Added explicit Rust code block with full filter chain: `LineType::User && !is_meta && !is_tool_result_continuation && !has_system_prefix`. |
| 20 | Step 8 autonomous spinner: `currentTime - null` = NaN when `current_turn_started_at` is null (session just discovered, no user message parsed). | Warning | Added explicit fallback: `session.currentTurnStartedAt ?? session.startedAt ?? currentTime`. |
| 21 | Step 5 missing TypeScript field name for longest task. | Minor | Added `session.longestTaskSeconds` (camelCase from ts-rs). |
| 22 | Steps lacked dependency graph and parallelization guidance. | Minor | Added "Step Dependencies" section with ASCII graph showing which steps can run in parallel. |

### Round 3: Full Audit Against Codebase (4 parallel agents)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 23 | Contribution API endpoint path wrong: plan said `/api/sessions/{id}/contribution`, actual is `/api/contributions/sessions/{id}`. | Blocker | Fixed Step 7 to reference correct endpoint path. Added `use-contributions.ts` line 77 as reference. |
| 24 | Handler signature wrong: `State<AppState>` — all routes use `State<Arc<AppState>>`. | Blocker | Rewrote Step 4 handler signature with `Arc` wrapper and reference to `contributions.rs` line 467. |
| 25 | Return type wrong: `impl IntoResponse` — codebase uses `ApiResult<impl IntoResponse>` for `?` operator on DB queries. | Blocker | Fixed Step 4 return type to `ApiResult<impl IntoResponse>`. |
| 26 | JSONL file access pattern unspecified: plan said "re-parse JSONL" but didn't show how to locate the file. No `sessions_dir` on AppState — paths are DB-mediated via `state.db.get_session_file_path()`. | Blocker | Added explicit DB-mediated file resolution code to Step 4 with reference to `sessions.rs` lines 390-410. |
| 27 | Zero tests specified: plan adds ~400 lines of new logic (turn detection, endpoint, live tracking) with no tests. 27 existing tests break from LiveLine struct changes. | Blocker | Added entire Step 9 with 5 sub-steps: test helper updates, parser tests, indexer fixture tests, migration tests, endpoint tests. |
| 28 | Two `LiveLine` construction sites in `live_parser.rs` (error fallback line 167, normal path line 257) also need new fields — plan only mentioned `make_live_line()` test helper. | Warning | Expanded Step 1 item 4 to list all 3 construction sites with line numbers. |
| 29 | `parse_single_line()` line reference off by 8: plan said lines 138-272, actual is lines 146-272. | Warning | Fixed line reference in Implementation Note section. |
| 30 | `SessionAccumulator` struct extends to line 96 (not 68): plan underestimated struct size. Missing `SessionAccumulator::new()` reference for initializing new field. | Warning | Fixed to lines 37-96 in Data Model section. Added `new()` at lines 71-96 to Step 3. |
| 31 | `process_jsonl_update()` starts at line 503, not 496. | Minor | Fixed line reference in Step 3. |
| 32 | `LiveSession` serde `rename_all = "camelCase"` not mentioned — Rust snake_case fields become camelCase in JSON/TypeScript. | Minor | Added note to Data Model section about automatic camelCase rename. |
| 33 | SessionCard spinner row starts at line 240, not 241. Metrics row starts at line 251. | Minor | Fixed line references in Step 5. |
| 34 | Step 4 parameter order: codebase convention is State first, then Path (not Path first). | Minor | Fixed handler signature in Step 4 to match convention. |
| 35 | Step 4 missing `pub fn router()` function requirement for route registration. | Minor | Added item 5 to Step 4 specifying `Router<Arc<AppState>>` function. |
| 36 | Step 4 SIMD claim: existing `parse_session()` does full deserialization, not SIMD pre-filter. Plan should clarify two implementation options. | Minor | Added "Fast path" vs "Simple path" options to Step 4 item 3. |
| 37 | `turn_duration` aggregation line range: plan said 1907-1918, actual is 1907-1914. | Minor | Fixed in Data Model section. |

### Round 4: Adversarial Review

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 38 | Adding 3 fields to `SessionInfo` (no `Default` derive) breaks **30+ construction sites** across 15+ files — plan only mentioned `row_types.rs`. | Blocker | Added full list of all affected files with line numbers to Sub-step 2a. Added `cargo build | grep "missing field"` tip. |
| 39 | Step 3 code block type mismatch: `line.timestamp.and_then(parse_timestamp_to_unix)` won't compile — `parse_timestamp_to_unix` takes `&str`, not `String`. | Blocker | Rewrote to `if let Some(ref ts)` pattern matching existing code at manager.rs line 619-621. |
| 40 | `LiveSession` struct literal at manager.rs lines 692-713 also needs new fields — plan didn't mention this construction site. | Blocker | Added Step 3 item 6 with explicit field values for the construction site. |
| 41 | `parse_version` versions 4 and 5 have no documentation comments. | Warning | Added placeholder comments for versions 4/5 alongside version 6. |
| 42 | Deep indexer insertion point for turn detection not specified — it uses its own `memmem` scanning, NOT `LiveLine`/`live_parser.rs`. | Warning | Rewrote Sub-step 2b to reference `ExtendedMetadata` struct (line 130) and specify insertion at lines 700-820. |
| 43 | `UPDATE_SESSION_DEEP_SQL` (line 31), `params!` macro (line 1929), param count comment (line 28), and sqlx fallback (~line 2050) must ALL be updated in lockstep. | Warning | Added explicit coupling note to Sub-step 2b item 9. |
| 44 | `SessionContribution` query uses tuple-based `query_as`, not struct-based `FromRow`. Adding a field requires changing tuple type to 10 elements. | Warning | Added explicit tuple pattern note to Step 7 item 1. |
| 45 | `extract_content_and_tools()` call site at line 203 must update destructuring from `(preview, tools)` to `(preview, tools, is_tool_result)`. | Warning | Added explicit before/after code to Step 1 item 2. |
| 46 | CompactSessionTable accessor should use camelCase `totalTaskTimeSeconds` (ts-rs generates camelCase), not snake_case `total_task_time_seconds`. | Minor | Fixed all references in Step 6. |
| 47 | `taskTimeSeconds` prop type not specified — should be `number | null` to match SessionInfo. | Minor | Added type annotation to Step 5 item 1. |
| 48 | NaN fallback UX note: when falling back to `startedAt`, spinner shows session-level elapsed (same as current behavior). | Minor | Added UX note to Step 8 item 2. |

### Round 5: Final Polish (100/100 patch)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 49 | Deep indexer insertion point was approximate ("around lines 700-820"). | Warning | Pinpointed exact location: `"user"` branch at line 774, after content extraction at line 783. Added verbatim Rust code showing turn open/close logic. Noted `is_system_user_content()` at line 1423 can be reused. Added EOF turn-close note. |
| 50 | 4 frontend test files construct `SessionInfo` mock objects that will fail TypeScript compilation after ts-rs regeneration. | Warning | Added `CompactSessionTable.test.tsx`, `SessionCard.test.tsx`, `group-sessions.test.ts`, `ActivityCalendar.test.tsx` to the SessionInfo cascade list with camelCase field names. Added `bun run typecheck` tip. |
| 51 | No rollback section — plan didn't explain how to undo if things go wrong. | Warning | Added "Rollback Plan" section: git revert, parse_version downgrade, SQLite column persistence note, partial rollback option. |
