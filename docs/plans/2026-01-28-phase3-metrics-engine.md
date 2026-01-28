---
status: pending
date: 2026-01-28
---

# Phase 3: Metrics Engine — Implementation Plan

> Pure facts, no judgment. Collect atomic units, compute derived metrics, let users interpret.

**Estimated steps:** 48 (Part A: 28 backend, Part B: 20 frontend)

---

## Design Principles

1. **Atomic units over abstractions** — Measure smallest provable units, derive everything else
2. **No judgment** — Show numbers, not labels (no "Smooth"/"Turbulent")
3. **Deterministic extraction** — If we can't be 100% certain, don't count it
4. **Ultra-conservative git correlation** — Only provable evidence (Tier 1-2)
5. **Compute on read** — Store atomic units, derive metrics in API layer

---

## Part A: Backend

### A0. JSONL Data Structure Reference

All extraction relies on the actual Claude Code JSONL format. This section documents the exact JSON paths.

#### A0.1 Message Types

```
Root-level field: .type
Valid values: "user" | "assistant" | "system" | "progress" | "file-history-snapshot"
```

#### A0.2 User Message Structure

```json
{
  "type": "user",
  "uuid": "624dfd06-...",
  "timestamp": "2026-01-27T17:09:00.470Z",
  "message": { "role": "user", "content": "..." }
}
```

#### A0.3 Assistant Message Structure (with tool_use)

```json
{
  "type": "assistant",
  "uuid": "4704ef81-...",
  "timestamp": "2026-01-27T17:09:30.844Z",
  "message": {
    "model": "claude-opus-4-5-20251101",
    "content": [
      { "type": "text", "text": "..." },
      {
        "type": "tool_use",
        "name": "Read",
        "input": { "file_path": "/path/to/file.rs" }
      }
    ]
  }
}
```

#### A0.4 Skill Invocation Structure

```json
{
  "type": "tool_use",
  "name": "Skill",
  "input": { "skill": "commit" }
}
```

Valid commit skill names: `"commit"`, `"commit-commands:commit"`, `"commit-commands:commit-push-pr"`

#### A0.5 Timestamp Format

- JSONL: ISO8601 string at root `.timestamp` field
- All internal storage: Unix seconds (INTEGER)
- Parsing: `DateTime::parse_from_rfc3339()` → `.timestamp()` for Unix seconds

---

### A1. Atomic Unit Collection

#### A1.1 User Prompt Count

| Field | `user_prompt_count` |
|-------|---------------------|
| Type | `INTEGER NOT NULL DEFAULT 0` |
| JSON Path | Count JSONL lines where `.type == "user"` |
| Extraction | SIMD scan for `"type":"user"` in `pass_2_deep_index` |

**Acceptance tests:**

| # | Input | Expected | Notes |
|---|-------|----------|-------|
| 1 | 5 user messages, 10 assistant | `5` | Basic counting |
| 2 | 0 user messages | `0` | Empty case |
| 3 | User message content has unicode/emoji | `5` | Content encoding doesn't affect type detection |
| 4 | Malformed JSON line in file | Skip line, continue | Graceful degradation |
| 5 | Empty JSONL file | `0` | File exists but no messages |

---

#### A1.2 Files Read

| Field | `files_read` (JSON array), `files_read_count` (INTEGER) |
|-------|--------------------------------------------------------|
| JSON Path | `.message.content[].input.file_path` WHERE `.message.content[].type == "tool_use"` AND `.message.content[].name == "Read"` |
| Storage | `files_read`: Deduplicated array of unique paths |
| Storage | `files_read_count`: Length of deduplicated array |
| Path Handling | Store paths exactly as-is (no normalization). Dedup on exact string match. |

**Acceptance tests:**

| # | Input | `files_read` | `files_read_count` | Notes |
|---|-------|--------------|--------------------|-------|
| 1 | Read `/a/foo.rs`, `/a/bar.rs` | `["/a/foo.rs", "/a/bar.rs"]` | `2` | Basic |
| 2 | Read `/a/foo.rs` twice | `["/a/foo.rs"]` | `1` | Dedup |
| 3 | No Read tool calls | `[]` | `0` | Empty |
| 4 | Read tool with missing `file_path` | Skip this call | — | Defensive |
| 5 | Path with spaces `/a/my file.rs` | `["/a/my file.rs"]` | `1` | No URL encoding |

---

#### A1.3 Files Edited

| Field | `files_edited` (JSON array), `files_edited_count` (INTEGER) |
|-------|-------------------------------------------------------------|
| JSON Path | `.message.content[].input.file_path` WHERE `.message.content[].type == "tool_use"` AND `.message.content[].name IN ("Edit", "Write")` |
| Storage | `files_edited`: **All occurrences** (not deduplicated) — needed for re-edit calculation |
| Storage | `files_edited_count`: Count of **unique** paths |
| Path Handling | Store paths exactly as-is (no normalization). |

**Acceptance tests:**

| # | Input | `files_edited` | `files_edited_count` | Notes |
|---|-------|----------------|----------------------|-------|
| 1 | Edit `foo.rs`, Write `bar.rs` | `["foo.rs", "bar.rs"]` | `2` | Basic |
| 2 | Edit `foo.rs` 3 times | `["foo.rs", "foo.rs", "foo.rs"]` | `1` | All occurrences stored, count is unique |
| 3 | Edit tool with missing `file_path` | Skip this call | — | Defensive |
| 4 | Write tool with missing `file_path` | Skip this call | — | Defensive |

---

#### A1.4 Re-edited Files Count

| Field | `reedited_files_count` |
|-------|------------------------|
| Type | `INTEGER NOT NULL DEFAULT 0` |
| Source | Count of files appearing 2+ times in `files_edited` array |
| Derivation | `files_edited.group_by(path).filter(count >= 2).len()` |

**Acceptance tests:**

| # | Input (`files_edited`) | `reedited_files_count` | Notes |
|---|------------------------|------------------------|-------|
| 1 | `["foo.rs", "foo.rs", "foo.rs", "bar.rs"]` | `1` | foo.rs edited 3×, bar.rs 1× |
| 2 | `["a.rs", "b.rs", "c.rs"]` | `0` | All unique |
| 3 | `[]` | `0` | Empty |
| 4 | `["x.rs", "x.rs", "y.rs", "y.rs"]` | `2` | Both files re-edited |

---

#### A1.5 Session Duration

| Field | `duration_seconds` |
|-------|-------------------|
| Type | `INTEGER NOT NULL DEFAULT 0` |
| Source | `last_message.timestamp - first_message.timestamp` (in Unix seconds) |
| Timestamp Field | Root-level `.timestamp` field on each JSONL line |
| Ordering | Sort all messages by `.timestamp` ascending; first and last define bounds |

**Acceptance tests:**

| # | Input | `duration_seconds` | Notes |
|---|-------|--------------------|-------|
| 1 | First at `2026-01-27T10:00:00Z`, last at `2026-01-27T10:15:30Z` | `930` | 15m 30s |
| 2 | Single message | `0` | Same timestamp |
| 3 | Empty file (no parseable messages) | `0` | Defensive |
| 4 | Messages with unparseable timestamps | Skip those messages | Use remaining messages |

---

#### A1.6 API Call Count

| Field | `api_call_count` |
|-------|------------------|
| Type | `INTEGER NOT NULL DEFAULT 0` |
| JSON Path | Count JSONL lines where `.type == "assistant"` |
| Note | Each assistant JSONL entry represents one API call (streaming chunks are collapsed) |

**Acceptance tests:**

| # | Input | `api_call_count` | Notes |
|---|-------|------------------|-------|
| 1 | 5 user, 8 assistant messages | `8` | Count assistant only |
| 2 | 0 assistant messages | `0` | User-only session (rare) |

---

#### A1.7 Tool Call Count

| Field | `tool_call_count` |
|-------|-------------------|
| Type | `INTEGER NOT NULL DEFAULT 0` |
| JSON Path | Count all `.message.content[]` blocks where `.type == "tool_use"` across all assistant messages |
| Note | One assistant message can contain multiple tool_use blocks |

**Acceptance tests:**

| # | Input | `tool_call_count` | Notes |
|---|-------|-------------------|-------|
| 1 | 3 assistant messages, each with 2 tool calls | `6` | Sum across messages |
| 2 | Assistant message with no tool_use | `0` | Text-only response |
| 3 | Assistant message with 5 parallel tool calls | `5` | All counted |

---

### A2. Derived Metrics (Computed on Read)

All derived metrics are computed in the API layer, not stored. Precision: 2 decimal places for display, full precision for calculations.

#### A2.1 Tokens Per Prompt

```rust
fn tokens_per_prompt(total_input: u64, total_output: u64, user_prompt_count: u32) -> Option<f64> {
    if user_prompt_count == 0 { return None; }
    Some((total_input + total_output) as f64 / user_prompt_count as f64)
}
```

| Input | Output | Notes |
|-------|--------|-------|
| `(1000, 500, 5)` | `Some(300.0)` | Normal |
| `(1000, 500, 0)` | `None` | Division by zero |

#### A2.2 Re-edit Rate

```rust
fn reedit_rate(reedited_files_count: u32, files_edited_count: u32) -> Option<f64> {
    if files_edited_count == 0 { return None; }
    Some(reedited_files_count as f64 / files_edited_count as f64)
}
```

| Input | Output | Notes |
|-------|--------|-------|
| `(2, 10)` | `Some(0.2)` | 20% re-edit rate |
| `(0, 5)` | `Some(0.0)` | No re-edits |
| `(1, 0)` | `None` | No files edited |

#### A2.3 Tool Density

```rust
fn tool_density(tool_call_count: u32, api_call_count: u32) -> Option<f64> {
    if api_call_count == 0 { return None; }
    Some(tool_call_count as f64 / api_call_count as f64)
}
```

| Input | Output | Notes |
|-------|--------|-------|
| `(15, 5)` | `Some(3.0)` | 3 tools per API call on average |
| `(0, 5)` | `Some(0.0)` | No tools used |
| `(5, 0)` | `None` | No API calls |

#### A2.4 Edit Velocity

```rust
fn edit_velocity(files_edited_count: u32, duration_seconds: u32) -> Option<f64> {
    if duration_seconds == 0 { return None; }
    Some(files_edited_count as f64 / (duration_seconds as f64 / 60.0))
}
```

| Input | Output | Notes |
|-------|--------|-------|
| `(10, 600)` | `Some(1.0)` | 1 edit per minute |
| `(5, 0)` | `None` | Instant session |

#### A2.5 Read-to-Edit Ratio

```rust
fn read_to_edit_ratio(files_read_count: u32, files_edited_count: u32) -> Option<f64> {
    if files_edited_count == 0 { return None; }
    Some(files_read_count as f64 / files_edited_count as f64)
}
```

| Input | Output | Notes |
|-------|--------|-------|
| `(20, 5)` | `Some(4.0)` | Read 4 files per edit |
| `(0, 5)` | `Some(0.0)` | Edit without reading (Write only) |
| `(10, 0)` | `None` | Read-only session |

---

### A3. Git Correlation (Ultra-Conservative)

#### A3.1 Tier 1: Commit Skill Invoked

| Aspect | Specification |
|--------|---------------|
| Detection | `.message.content[].name == "Skill"` AND `.message.content[].input.skill` IN `["commit", "commit-commands:commit", "commit-commands:commit-push-pr"]` |
| Timestamp | Root `.timestamp` field of the JSONL line containing the Skill tool_use |
| Window | `[skill_invocation_ts - 60s, skill_invocation_ts + 300s]` (1 min before to 5 min after) |
| Repo Match | `commit.repo_path == session.project_path` (exact string match) |
| Evidence JSON | `{"rule": "commit_skill", "skill_ts": 1706400000, "commit_ts": 1706400120, "skill_name": "commit"}` |

**Acceptance tests:**

| # | Scenario | Link Created? | Notes |
|---|----------|---------------|-------|
| 1 | `/commit` at 10:00, commit at 10:02, same repo | Yes (Tier 1) | Within window |
| 2 | `/commit` at 10:00, commit at 10:10, same repo | No | Outside +5min window |
| 3 | `/commit` at 10:00, commit at 9:58, same repo | Yes (Tier 1) | Within -1min window |
| 4 | `/commit` at 10:00, commit at 10:02, different repo | No | Repo mismatch |
| 5 | `/commit-commands:commit-push-pr`, commit at 10:01 | Yes (Tier 1) | Alternate skill name |

#### A3.2 Tier 2: Commit During Session

| Aspect | Specification |
|--------|---------------|
| Time Match | `commit.timestamp >= session.first_message_at` AND `commit.timestamp <= session.last_message_at` |
| Repo Match | `commit.repo_path == session.project_path` (exact string match) |
| Evidence JSON | `{"rule": "during_session", "commit_ts": 1706400120, "session_start": 1706399000, "session_end": 1706401000}` |

**Acceptance tests:**

| # | Scenario | Link Created? | Notes |
|---|----------|---------------|-------|
| 1 | Session 10:00-11:00, commit at 10:30, same repo | Yes (Tier 2) | Within session |
| 2 | Session 10:00-11:00, commit at 11:05, same repo | No | After session |
| 3 | Session 10:00-11:00, commit at 9:55, same repo | No | Before session |
| 4 | Session 10:00-11:00, commit at 10:30, different repo | No | Repo mismatch |

#### A3.3 Tier Priority

If a commit matches both Tier 1 and Tier 2 rules, record as **Tier 1** (higher confidence).

#### A3.4 Git Scan Edge Cases

| Scenario | Behavior |
|----------|----------|
| Session in non-git directory | No commits linked, no error |
| Git repo is bare/corrupt | Log warning, skip repo, continue with other repos |
| Commit message contains quotes/newlines | JSON-escape in evidence field |
| Git command fails (timeout, permissions) | Log error, return empty commits for that repo |
| Concurrent git sync requests | Return 409 Conflict if sync already in progress |

#### A3.5 Not Implemented (Removed)

| Feature | Reason |
|---------|--------|
| File overlap matching | Too fuzzy — false positives on common files |
| Branch matching | Can't verify retroactively (branch may be deleted) |
| 5-min post-session window | Unprovable causation |
| 2-hour window | Too loose — coincidental correlation |

---

### A4. Trends

#### A4.1 Time Periods

```rust
fn current_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let monday = now - Duration::days(now.weekday().num_days_from_monday() as i64);
    let start = monday.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let end = now.timestamp();
    (start, end)
}

fn previous_week_bounds() -> (i64, i64) {
    let now = Utc::now();
    let this_monday = now - Duration::days(now.weekday().num_days_from_monday() as i64);
    let prev_monday = this_monday - Duration::days(7);
    let prev_sunday = this_monday - Duration::seconds(1);
    let start = prev_monday.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let end = prev_sunday.timestamp();
    (start, end)
}
```

#### A4.2 Session Week Bucketing

Sessions are assigned to a week based on `first_message_at` timestamp.

| Scenario | Week Assignment |
|----------|-----------------|
| Session starts Sun 23:00 UTC, ends Mon 01:00 UTC | Assigned to **Sunday's week** (uses start time) |
| Session entirely within one week | Assigned to that week |

#### A4.3 Trend Metric Shape

```typescript
interface TrendMetric {
  current: number;
  previous: number;
  delta: number;           // current - previous
  deltaPercent: number | null;  // NULL if previous = 0; else ((current - previous) / previous) * 100
}
```

**Precision:** `deltaPercent` rounded to 1 decimal place (e.g., `15.3`, not `15.333333`).

**Acceptance tests:**

| current | previous | delta | deltaPercent |
|---------|----------|-------|--------------|
| 120 | 100 | 20 | 20.0 |
| 100 | 120 | -20 | -16.7 |
| 50 | 0 | 50 | null |
| 0 | 50 | -50 | -100.0 |
| 0 | 0 | 0 | null |

#### A4.4 Metrics to Trend

| Metric | Aggregation |
|--------|-------------|
| Session count | `COUNT(*)` where `first_message_at` in period |
| Total tokens | `SUM(total_input_tokens + total_output_tokens)` |
| Avg tokens per prompt | `SUM(tokens) / SUM(user_prompt_count)` (weighted average) |
| Total files edited | `SUM(files_edited_count)` |
| Avg re-edit rate | `SUM(reedited_files_count) / SUM(files_edited_count)` (weighted average) |
| Commit link count | `COUNT(*)` from `session_commits` where session in period |

---

### A5. Export

#### A5.1 Formats

| Format | MIME | Use Case | Filename |
|--------|------|----------|----------|
| JSON | `application/json` | Programmatic | `sessions-export-{timestamp}.json` |
| CSV | `text/csv; charset=utf-8` | Spreadsheet | `sessions-export-{timestamp}.csv` |

#### A5.2 Session Export Schema (JSON)

```typescript
interface SessionExport {
  id: string;
  project_path: string;
  first_message_at: number;      // Unix seconds
  last_message_at: number;       // Unix seconds
  duration_seconds: number;
  user_prompt_count: number;
  api_call_count: number;
  tool_call_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
  files_read_count: number;
  files_edited_count: number;
  reedited_files_count: number;
  files_read: string[];
  files_edited: string[];
  tokens_per_prompt: number | null;
  reedit_rate: number | null;
  tool_density: number | null;
  edit_velocity: number | null;
  commits: Array<{
    hash: string;
    message: string;
    timestamp: number;
    tier: 1 | 2;
  }>;
}
```

#### A5.3 CSV Format Specification

**Column order (fixed):**

```
id,project_path,first_message_at,last_message_at,duration_seconds,user_prompt_count,api_call_count,tool_call_count,total_input_tokens,total_output_tokens,files_read_count,files_edited_count,reedited_files_count,files_read,files_edited,tokens_per_prompt,reedit_rate,tool_density,edit_velocity,commit_count,commit_hashes
```

**Serialization rules:**

| Field Type | CSV Serialization | Example |
|------------|-------------------|---------|
| `string` | Double-quote if contains comma/newline/quote; escape quotes as `""` | `"foo, bar"` |
| `number` | Plain number | `1234` |
| `null` | Empty string | `` |
| `string[]` | Pipe-delimited, double-quoted | `"foo.rs|bar.rs|baz.rs"` |
| `commits[]` | Hashes only, pipe-delimited | `"abc1234|def5678"` |

**Example row:**

```csv
"sess-123","/Users/me/project",1706400000,1706401000,1000,5,8,15,5000,2000,10,5,2,"a.rs|b.rs","a.rs|a.rs|b.rs",1400.00,0.40,1.88,0.30,2,"abc1234|def5678"
```

**Edge cases:**

| Scenario | Behavior |
|----------|----------|
| File path contains pipe `|` | Replace with `\|` (escaped) |
| File path contains comma `,` | OK, enclosed in quotes |
| File path contains newline | Replace with space |
| Commit message contains newline | Not included in CSV (only hash) |

---

### A6. Data Freshness

#### A6.1 Storage

```sql
CREATE TABLE IF NOT EXISTS index_metadata (
    id                      INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_indexed_at         INTEGER,           -- Unix seconds when indexing COMPLETED successfully
    last_index_duration_ms  INTEGER,           -- Wall-clock time of last successful index
    sessions_indexed        INTEGER NOT NULL DEFAULT 0,
    projects_indexed        INTEGER NOT NULL DEFAULT 0,
    last_git_sync_at        INTEGER,           -- Unix seconds when git sync COMPLETED
    commits_found           INTEGER NOT NULL DEFAULT 0,
    links_created           INTEGER NOT NULL DEFAULT 0,
    updated_at              INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
```

#### A6.2 Update Logic

```rust
// Called at END of pass_2_deep_index(), only on SUCCESS
fn update_index_metadata(db: &Database, start_time: Instant, sessions: u32, projects: u32) {
    let duration_ms = start_time.elapsed().as_millis() as i64;
    let now = Utc::now().timestamp();

    db.execute(
        "UPDATE index_metadata SET
            last_indexed_at = ?,
            last_index_duration_ms = ?,
            sessions_indexed = ?,
            projects_indexed = ?,
            updated_at = ?
         WHERE id = 1",
        (now, duration_ms, sessions, projects, now)
    );
}

// Called at END of git sync, only on SUCCESS
fn update_git_sync_metadata(db: &Database, commits_found: u32, links_created: u32) {
    let now = Utc::now().timestamp();

    db.execute(
        "UPDATE index_metadata SET
            last_git_sync_at = ?,
            commits_found = ?,
            links_created = ?,
            updated_at = ?
         WHERE id = 1",
        (now, commits_found, links_created, now)
    );
}
```

**Failure behavior:**

| Scenario | Metadata Update |
|----------|-----------------|
| Indexing succeeds | Update `last_indexed_at` |
| Indexing fails partway | **Do not update** — preserve last successful timestamp |
| Git sync succeeds | Update `last_git_sync_at` |
| Git sync fails | **Do not update** — preserve last successful timestamp |

---

### A7. Database Schema (Migration 8)

```sql
-- Migration 8: Metrics Engine + Git Correlation

-- Sessions: atomic units (with defensive CHECK constraints)
ALTER TABLE sessions ADD COLUMN user_prompt_count INTEGER NOT NULL DEFAULT 0 CHECK (user_prompt_count >= 0);
ALTER TABLE sessions ADD COLUMN api_call_count INTEGER NOT NULL DEFAULT 0 CHECK (api_call_count >= 0);
ALTER TABLE sessions ADD COLUMN tool_call_count INTEGER NOT NULL DEFAULT 0 CHECK (tool_call_count >= 0);
ALTER TABLE sessions ADD COLUMN files_read TEXT NOT NULL DEFAULT '[]';
ALTER TABLE sessions ADD COLUMN files_edited TEXT NOT NULL DEFAULT '[]';
ALTER TABLE sessions ADD COLUMN files_read_count INTEGER NOT NULL DEFAULT 0 CHECK (files_read_count >= 0);
ALTER TABLE sessions ADD COLUMN files_edited_count INTEGER NOT NULL DEFAULT 0 CHECK (files_edited_count >= 0);
ALTER TABLE sessions ADD COLUMN reedited_files_count INTEGER NOT NULL DEFAULT 0 CHECK (reedited_files_count >= 0);
ALTER TABLE sessions ADD COLUMN duration_seconds INTEGER NOT NULL DEFAULT 0 CHECK (duration_seconds >= 0);
ALTER TABLE sessions ADD COLUMN commit_count INTEGER NOT NULL DEFAULT 0 CHECK (commit_count >= 0);

-- Commits table
CREATE TABLE IF NOT EXISTS commits (
    hash            TEXT PRIMARY KEY,
    repo_path       TEXT NOT NULL,
    message         TEXT NOT NULL,
    author          TEXT,
    timestamp       INTEGER NOT NULL,
    branch          TEXT,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Session-commit links
CREATE TABLE IF NOT EXISTS session_commits (
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    commit_hash     TEXT NOT NULL REFERENCES commits(hash) ON DELETE CASCADE,
    tier            INTEGER NOT NULL CHECK (tier IN (1, 2)),
    evidence        TEXT NOT NULL DEFAULT '{}',
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (session_id, commit_hash)
);

-- Index metadata (singleton row)
CREATE TABLE IF NOT EXISTS index_metadata (
    id                      INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_indexed_at         INTEGER,           -- Unix seconds when indexing completed successfully
    last_index_duration_ms  INTEGER,           -- Wall-clock ms of last successful index
    sessions_indexed        INTEGER NOT NULL DEFAULT 0,
    projects_indexed        INTEGER NOT NULL DEFAULT 0,
    last_git_sync_at        INTEGER,           -- Unix seconds when git sync completed
    commits_found           INTEGER NOT NULL DEFAULT 0,
    links_created           INTEGER NOT NULL DEFAULT 0,
    updated_at              INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

INSERT OR IGNORE INTO index_metadata (id) VALUES (1);

-- Indexes
CREATE INDEX idx_commits_repo_ts ON commits(repo_path, timestamp DESC);
CREATE INDEX idx_commits_timestamp ON commits(timestamp DESC);
CREATE INDEX idx_session_commits_session ON session_commits(session_id);  -- FK index for CASCADE
CREATE INDEX idx_session_commits_commit ON session_commits(commit_hash);  -- FK index for CASCADE
-- NOTE: No tier index — only 2 values (low cardinality), planner will ignore it
CREATE INDEX idx_sessions_commit_count ON sessions(commit_count) WHERE commit_count > 0;
CREATE INDEX idx_sessions_reedit ON sessions(reedited_files_count) WHERE reedited_files_count > 0;
CREATE INDEX idx_sessions_duration ON sessions(duration_seconds);
```

---

### A8. API Endpoints

#### A8.1 Extended Endpoints

| Endpoint | Changes |
|----------|---------|
| `GET /api/sessions` | Add `filter`, `sort` params; add atomic unit fields |
| `GET /api/sessions/:id` | Add atomic units, derived metrics, files_touched, commits |
| `GET /api/stats/dashboard` | Add currentWeek metrics, trends |

#### A8.2 New Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/trends?period=week` | Week-over-week trend metrics |
| `GET /api/export/sessions?format=json` | Export all sessions |
| `GET /api/export/sessions/:id?format=json` | Export single session |
| `GET /api/status` | Data freshness info |
| `POST /api/sync/git` | Trigger git scan |

#### A8.3 Filter Parameter Specification

**Format:** `?filter={filter_name}`

| Filter Name | SQL Condition | Description |
|-------------|---------------|-------------|
| `all` (default) | No filter | All sessions |
| `has_commits` | `commit_count > 0` | Sessions with 1+ linked commits |
| `high_reedit` | `reedited_files_count * 1.0 / NULLIF(files_edited_count, 0) > 0.2` | Re-edit rate > 20% |
| `long_session` | `duration_seconds > 1800` | Sessions > 30 minutes |

**Multiple filters:** Not supported in v1. Use single filter value.

**Invalid filter:** Return 400 Bad Request with `{"error": "invalid_filter", "valid_filters": ["all", "has_commits", "high_reedit", "long_session"]}`

#### A8.4 Sort Parameter Specification

**Format:** `?sort={sort_name}`

| Sort Name | SQL Order | Description |
|-----------|-----------|-------------|
| `recent` (default) | `first_message_at DESC` | Most recent first |
| `tokens` | `(total_input_tokens + total_output_tokens) DESC` | Most tokens first |
| `prompts` | `user_prompt_count DESC` | Most prompts first |
| `files_edited` | `files_edited_count DESC` | Most files edited first |
| `duration` | `duration_seconds DESC` | Longest sessions first |

**Invalid sort:** Return 400 Bad Request with `{"error": "invalid_sort", "valid_sorts": ["recent", "tokens", "prompts", "files_edited", "duration"]}`

#### A8.5 POST /api/sync/git Behavior

| Scenario | Response |
|----------|----------|
| No sync in progress | 202 Accepted, `{"status": "started"}` |
| Sync already in progress | 409 Conflict, `{"status": "in_progress", "started_at": 1706400000}` |
| Sync completes | (async) Updates `index_metadata` |
| Sync fails | (async) Logs error, does not update metadata |

**Concurrency:** Use a mutex/lock to prevent concurrent git scans. Only one scan runs at a time.

---

### A9. Backend Implementation Steps

| # | Step | Description | Acceptance |
|---|------|-------------|------------|
| 1 | Migration 8 | Add all new columns, tables, indexes | `cargo test -p db` passes |
| 2 | Types: atomic units | Add fields to `SessionInfo`, `SessionDetail` | Types compile, ts-rs generates |
| 3 | Extraction: user_prompt_count | SIMD scan for `"type":"user"` | Unit tests pass (see A1.1) |
| 4 | Extraction: api_call_count | Count `"type":"assistant"` lines | Unit tests pass (see A1.6) |
| 5 | Extraction: tool_call_count | Count `"type":"tool_use"` in content arrays | Unit tests pass (see A1.7) |
| 6 | Extraction: files_read | Parse `.message.content[].input.file_path` for Read | Unit tests pass (see A1.2) |
| 7 | Extraction: files_edited | Parse `.message.content[].input.file_path` for Edit/Write | Unit tests pass (see A1.3) |
| 8 | Extraction: reedited_files_count | Count duplicates in files_edited | Unit tests pass (see A1.4) |
| 9 | Extraction: duration_seconds | Parse `.timestamp`, compute delta | Unit tests pass (see A1.5) |
| 10 | Extraction: skill invocations | Detect `/commit` skill calls for Tier 1 | Unit tests pass (see A3.1) |
| 11 | Integrate into pass_2 | Update `parse_bytes()` and pipeline | Integration tests pass |
| 12 | Queries: session with metrics | Update `get_session()`, `get_sessions()` | Tests pass |
| 13 | Derived metrics | Implement compute functions (A2.1-A2.5) | Unit tests pass |
| 14 | Git: scan_repo_commits() | Spawn `git log`, parse output | Tests with real repo |
| 15 | Git: correlate Tier 1 | Match /commit skill → commit within window | Unit tests pass (see A3.1) |
| 16 | Git: correlate Tier 2 | Match during-session commits | Unit tests pass (see A3.2) |
| 17 | Queries: commit CRUD | Insert commits, session_commits | Integration tests pass |
| 18 | Queries: trends | Week-over-week aggregations (A4.4) | Tests pass |
| 19 | Queries: index_metadata | Update on index/git sync complete | Tests pass |
| 20 | Route: GET /api/sessions (filter/sort) | Implement query params (A8.3-A8.4) | Tests pass |
| 21 | Route: GET /api/sessions/:id (extended) | Add all new fields | Tests pass |
| 22 | Route: GET /api/stats/dashboard (extended) | Add metrics + trends | Tests pass |
| 23 | Route: GET /api/trends | New endpoint | Tests pass |
| 24 | Route: GET /api/export/sessions | JSON + CSV export (A5.2-A5.3) | Tests pass |
| 25 | Route: GET /api/status | Data freshness | Tests pass |
| 26 | Route: POST /api/sync/git | Trigger git scan with mutex (A8.5) | Tests pass |
| 27 | Golden tests | Full pipeline with fixtures | All pass |
| 28 | Edge case tests | Robustness tests (A10) | All pass |

---

### A10. Robustness & Edge Case Tests

This section documents edge cases that must be handled gracefully. All tests should pass without panics or data corruption.

#### A10.1 JSONL Parsing Edge Cases

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | Malformed JSON line (truncated) | Skip line, continue parsing, log warning |
| 2 | Empty JSONL file | Return empty session with all counts = 0 |
| 3 | JSONL with only `system` messages | `user_prompt_count = 0`, `api_call_count = 0` |
| 4 | Missing `.timestamp` field on some lines | Skip those lines for duration calc, use remaining |
| 5 | Timestamp in unexpected format (not ISO8601) | Skip that line, log warning |
| 6 | Very large file (>100MB) | Stream parse, don't load entire file into memory |
| 7 | File with BOM (byte order mark) | Handle UTF-8 BOM transparently |
| 8 | Line with valid JSON but unknown `type` | Skip line, don't count in any metric |

#### A10.2 Tool Extraction Edge Cases

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | `tool_use` with missing `input` field | Skip this tool call |
| 2 | `tool_use` with missing `input.file_path` | Skip this tool call |
| 3 | `tool_use` with `file_path: null` | Skip this tool call |
| 4 | `tool_use` with `file_path: ""` (empty string) | Skip this tool call |
| 5 | File path with special characters (`\n`, `\t`, unicode) | Store as-is |
| 6 | Multiple `tool_use` blocks in one `content` array | Count all of them |
| 7 | `content` is string instead of array | Skip (legacy format) |

#### A10.3 Git Correlation Edge Cases

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | Session in non-git directory | `commit_count = 0`, no error |
| 2 | Git repo is bare | Log warning, skip this repo |
| 3 | Git repo is corrupt (`.git` exists but broken) | Log error, skip this repo |
| 4 | Git command times out (>10s) | Cancel, log error, skip this repo |
| 5 | Git command permission denied | Log error, skip this repo |
| 6 | Commit message contains quotes, newlines, unicode | Properly escape in JSON evidence |
| 7 | Commit hash collision (extremely rare) | Use full hash, not short hash |
| 8 | Session spans 2+ git repos (submodules) | Only correlate with primary repo (project_path) |
| 9 | Clock skew (commit timestamp in future) | Still apply rules, timestamps are what they are |
| 10 | Same commit matches both Tier 1 and Tier 2 | Record as Tier 1 only |

#### A10.4 Concurrent Access Edge Cases

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | Two simultaneous POST /api/sync/git | Second returns 409, first continues |
| 2 | GET /api/status during indexing | Return stale data, indicate "indexing in progress" |
| 3 | Export during indexing | Use snapshot isolation, return consistent data |
| 4 | Database locked (SQLite busy) | Retry with exponential backoff, max 5 retries |

#### A10.5 Data Integrity Edge Cases

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| 1 | Session deleted while loading commits | Cascade delete session_commits |
| 2 | Commit deleted while loading session | session_commits row deleted, session shows 0 commits |
| 3 | Re-index same session | Update atomic units, preserve commit links if unchanged |
| 4 | Very long session (10k+ messages) | No timeout, process incrementally |
| 5 | Session with 0 duration (all same timestamp) | `duration_seconds = 0`, derived metrics return NULL |

---

## Part B: Frontend

### B1. Design System

| Element | Value |
|---------|-------|
| Style | Data-Dense Dashboard |
| Primary | `#1E40AF` (blue-800) |
| Secondary | `#3B82F6` (blue-500) |
| Accent | `#F59E0B` (amber-500) |
| Background | `#F8FAFC` (slate-50) |
| Text | `#1E3A8A` (blue-900) |
| Typography | Fira Code (numbers) + Fira Sans (labels) |
| Icons | Lucide React (no emojis) |

---

### B2. Dashboard UI

#### B2.1 Metric Card Component

```typescript
interface MetricCardProps {
  label: string;
  value: string;
  trend?: {
    delta: number;
    deltaPercent: number | null;
  };
}
```

- Value: Fira Code, `text-2xl font-semibold text-blue-900`
- Trend: Lucide `TrendingUp`/`TrendingDown` icon + percent
- No color coding — icon direction is sufficient

#### B2.2 Metrics Grid

- Desktop: 3 columns
- Tablet: 2 columns
- Mobile: 1 column

#### B2.3 Cards to Display

1. Sessions This Week
2. Tokens This Week
3. Files Edited This Week
4. Avg Tokens Per Prompt
5. Re-edit Rate
6. Commits Linked

---

### B3. Session Detail UI

#### B3.1 Metrics Bar

5 metrics in horizontal row: Prompts, Tokens, Files (R/E), Re-edit %, Commits

#### B3.2 Files Touched Panel

List files with read/edit counts. Highlight re-edited files.

#### B3.3 Linked Commits Panel

Show hash, message (truncated), tier badge. Click to copy hash.

---

### B4. Session List UI

#### B4.1 SessionCard Metrics Row

`5 prompts · 12.4k tokens · 3 files edited · 2 commits`

#### B4.2 Filter Dropdown

- All sessions
- Has commits
- High re-edit (>20%)
- Long sessions (>30min)

#### B4.3 Sort Dropdown

- Most recent
- Most tokens
- Most prompts
- Most files edited

---

### B5. Data Freshness Footer

`Last synced: 5 min ago · 491 sessions`

---

### B6. States

| State | Implementation |
|-------|----------------|
| Loading | Skeleton with `animate-pulse`, `aria-busy="true"` |
| Empty | Descriptive text, no data illustration |
| Error | `role="alert"` with retry button |

---

### B7. Accessibility

- No emojis as icons
- `cursor-pointer` on clickables
- Hover transitions 150-300ms
- Text contrast 4.5:1 minimum
- Focus visible rings
- `prefers-reduced-motion` respected
- Screen reader labels on metrics

---

### B8. SessionCard Time Format

Display both start and end times with duration:

```
┌─────────────────────────────────────────────────────────────────────┐
│ claude-view                                                         │
│ Today 2:30 PM  →  3:15 PM                              45 min total │
│ ─────────────────────────────────────────────────────────────────── │
│ "Fix accessibility issues in dashboard cards"                       │
│                                                                     │
│ 8 prompts · 12.4k tokens · 5 files · 1 re-edit                     │
│ 1 commit  |  /commit  /brainstorm                                   │
└─────────────────────────────────────────────────────────────────────┘
```

**Time display rules:**
- Today: `Today 2:30 PM → 3:15 PM`
- Yesterday: `Yesterday 4:00 PM → 4:18 PM`
- Older: `Jan 26 9:30 AM → 12:48 PM`

---

### B9. UI Mockups

#### B9.1 Dashboard

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│  VIBE RECALL                                    [Cmd+K]               [Settings]    │
├────────────────────┬────────────────────────────────────────────────────────────────┤
│                    │                                                                │
│  ┌──────────────┐  │  Dashboard                                                     │
│  │  History     │  │  ───────────────────────────────────────────────────────────── │
│  └──────────────┘  │                                                                │
│                    │  THIS WEEK                                                     │
│  PROJECTS          │  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐      │
│  ────────────────  │  │ Sessions       │ │ Tokens         │ │ Files Edited   │      │
│                    │  │      24        │ │    289k        │ │      87        │      │
│  > claude-view  89 │  │  [^] +20%      │ │  [^] +15%      │ │  [v] -8%       │      │
│  > api-server   45 │  └────────────────┘ └────────────────┘ └────────────────┘      │
│  > frontend     32 │                                                                │
│  > shared-lib   18 │  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐      │
│                    │  │ Tokens/Prompt  │ │ Re-edit Rate   │ │ Commits Linked │      │
│                    │  │    1,842       │ │     12%        │ │      18        │      │
│                    │  │  [^] +5%       │ │  [v] -3%       │ │  [^] +25%      │      │
│                    │  └────────────────┘ └────────────────┘ └────────────────┘      │
│                    │                                                                │
│                    │  ┌─────────────────────────────────────────────────────────┐   │
│                    │  │ ACTIVITY                                                 │   │
│                    │  │ M   T   W   T   F   S   S   M   T   W   T   F   S   S   │   │
│                    │  │ #   ##  #   ### ##      #   #   ##  ### ##  #            │   │
│                    │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
│                    │  ┌────────────────────────────┐ ┌──────────────────────────┐   │
│                    │  │ RECENT COMMITS             │ │ TOP SKILLS               │   │
│                    │  │ abc1234 "fix: a11y"        │ │ /brainstorm       32     │   │
│                    │  │ Tier 1 · 2h ago            │ │ /commit           28     │   │
│                    │  │ def5678 "feat: export"     │ │ /review-pr        15     │   │
│                    │  │ Tier 2 · 5h ago            │ │ /debug             9     │   │
│                    │  └────────────────────────────┘ └──────────────────────────┘   │
│                    │                                                                │
├────────────────────┴────────────────────────────────────────────────────────────────┤
│  Last synced: 5 min ago · 491 sessions                                              │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

#### B9.2 Session List (History)

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│  VIBE RECALL                                    [Cmd+K]               [Settings]    │
├────────────────────┬────────────────────────────────────────────────────────────────┤
│                    │                                                                │
│  ┌──────────────┐  │  Sessions                                                      │
│  │  History     │  │  ───────────────────────────────────────────────────────────── │
│  └──────────────┘  │                                                                │
│                    │  Filter: [All ▼]  Sort: [Recent ▼]              Search...      │
│  PROJECTS          │                                                                │
│  ────────────────  │  ┌──────────────────────────────────────────────────────────┐  │
│                    │  │ claude-view                                              │  │
│  > claude-view  89 │  │ Today 2:30 PM  →  3:15 PM                    45 min      │  │
│  > api-server   45 │  │ ─────────────────────────────────────────────────────────│  │
│  > frontend     32 │  │ "Fix accessibility issues in dashboard cards"            │  │
│  > shared-lib   18 │  │                                                          │  │
│                    │  │ 8 prompts · 12.4k tokens · 5 files · 1 re-edit           │  │
│                    │  │ 1 commit  |  /commit  /brainstorm                        │  │
│                    │  └──────────────────────────────────────────────────────────┘  │
│                    │                                                                │
│                    │  ┌──────────────────────────────────────────────────────────┐  │
│                    │  │ api-server                                               │  │
│                    │  │ Today 10:15 AM  →  12:22 PM                   2.1 hr     │  │
│                    │  │ ─────────────────────────────────────────────────────────│  │
│                    │  │ "Implement user authentication endpoints"                │  │
│                    │  │                                                          │  │
│                    │  │ 23 prompts · 89.2k tokens · 12 files · 4 re-edits        │  │
│                    │  │ 2 commits  |  /debug  /commit                            │  │
│                    │  └──────────────────────────────────────────────────────────┘  │
│                    │                                                                │
│                    │  ┌──────────────────────────────────────────────────────────┐  │
│                    │  │ shared-lib                                               │  │
│                    │  │ Jan 26 9:30 AM  →  12:48 PM                   3.2 hr     │  │
│                    │  │ ─────────────────────────────────────────────────────────│  │
│                    │  │ "Debug flaky test in CI pipeline"                        │  │
│                    │  │                                                          │  │
│                    │  │ 31 prompts · 145k tokens · 8 files · 6 re-edits          │  │
│                    │  │ No commits  |  /debug  /debug  /debug                    │  │
│                    │  └──────────────────────────────────────────────────────────┘  │
│                    │                                                                │
├────────────────────┴────────────────────────────────────────────────────────────────┤
│  Last synced: 5 min ago · 491 sessions                                              │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

#### B9.3 Session Detail

Conversation is the main focus; metrics panel on the right.

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│  VIBE RECALL                                    [Cmd+K]               [Settings]    │
├────────────────────┬────────────────────────────────────────────────────────────────┤
│                    │                                                                │
│  ┌──────────────┐  │  < Back                                    [HTML] [PDF]        │
│  │  History     │  │                                                                │
│  └──────────────┘  │  ┌─────────────────────────────────────────────────────────┐   │
│                    │  │ claude-view                                              │   │
│  PROJECTS          │  │ Today 2:30 PM  →  3:15 PM                      45 min   │   │
│  ────────────────  │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
│  v claude-view  89 │  ┌─────────────────────────────────┬───────────────────────┐   │
│    · Session 1     │  │                                 │                       │   │
│    · Session 2     │  │  CONVERSATION                   │  METRICS              │   │
│  > api-server   45 │  │                                 │  ─────────────────────│   │
│  > frontend     32 │  │  ┌─ You ─────────────────────┐  │                       │   │
│                    │  │  │ Fix the accessibility     │  │  Prompts          8   │   │
│                    │  │  │ issues in the dashboard   │  │  Tokens      12,438   │   │
│                    │  │  │ cards. The contrast is    │  │    (1.6k/prompt)      │   │
│                    │  │  │ too low.                  │  │  Files Read      12   │   │
│                    │  │  └───────────────────────────┘  │  Files Edited     5   │   │
│                    │  │                                 │  Re-edits         1   │   │
│                    │  │  ┌─ Claude ──────────────────┐  │    (20%)              │   │
│                    │  │  │ I'll fix the contrast     │  │                       │   │
│                    │  │  │ issues...                 │  │  ─────────────────────│   │
│                    │  │  │                           │  │  SKILLS               │   │
│                    │  │  │ [Read] Card.tsx           │  │  /commit              │   │
│                    │  │  │ [Edit] Card.tsx           │  │  /brainstorm          │   │
│                    │  │  └───────────────────────────┘  │                       │   │
│                    │  │                                 │  ─────────────────────│   │
│                    │  │  ┌─ You ─────────────────────┐  │  COMMITS (1)          │   │
│                    │  │  │ Looks good, commit it     │  │  abc1234              │   │
│                    │  │  └───────────────────────────┘  │  "fix: button a11y"   │   │
│                    │  │                                 │  Tier 1 · 3:18 PM     │   │
│                    │  │  ┌─ Claude ──────────────────┐  │                       │   │
│                    │  │  │ [Skill] /commit           │  │  ─────────────────────│   │
│                    │  │  └───────────────────────────┘  │  FILES TOUCHED        │   │
│                    │  │                                 │  Card.tsx    R2 E2 [!]│   │
│                    │  │                                 │  Button.tsx  R1 E1    │   │
│                    │  │                                 │  + 8 more...          │   │
│                    │  └─────────────────────────────────┴───────────────────────┘   │
│                    │                                                                │
├────────────────────┴────────────────────────────────────────────────────────────────┤
│  Last synced: 5 min ago · 491 sessions                                              │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

#### B9.4 Settings

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│  VIBE RECALL                                    [Cmd+K]               [Settings]    │
├────────────────────┬────────────────────────────────────────────────────────────────┤
│                    │                                                                │
│  ┌──────────────┐  │  Settings                                                      │
│  │  History     │  │  ───────────────────────────────────────────────────────────── │
│  └──────────────┘  │                                                                │
│                    │  ┌─────────────────────────────────────────────────────────┐   │
│  PROJECTS          │  │ DATA STATUS                                              │   │
│  ────────────────  │  │  Last indexed      5 minutes ago                         │   │
│                    │  │  Index duration    1.2s                                  │   │
│  > claude-view  89 │  │  Sessions          491                                   │   │
│  > api-server   45 │  │  Projects          12                                    │   │
│  > frontend     32 │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
│                    │  ┌─────────────────────────────────────────────────────────┐   │
│                    │  │ GIT SYNC                                                 │   │
│                    │  │  Scans git history and correlates commits with sessions. │   │
│                    │  │                                                          │   │
│                    │  │  Last sync         2 hours ago                           │   │
│                    │  │  Commits found     847                                   │   │
│                    │  │  Links created     156                                   │   │
│                    │  │                              [ Sync Git History ]        │   │
│                    │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
│                    │  ┌─────────────────────────────────────────────────────────┐   │
│                    │  │ EXPORT DATA                                              │   │
│                    │  │  Export all session data with metrics and commits.       │   │
│                    │  │                                                          │   │
│                    │  │  Format:  ( ) JSON    (•) CSV                            │   │
│                    │  │  Scope:   (•) All sessions  ( ) Current project only     │   │
│                    │  │                              [ Download Export ]         │   │
│                    │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
│                    │  ┌─────────────────────────────────────────────────────────┐   │
│                    │  │ ABOUT                                                    │   │
│                    │  │  Vibe Recall v0.2.0                                      │   │
│                    │  │  Cmd+K  Command palette   Cmd+Shift+E  Export HTML       │   │
│                    │  │  Cmd+/  Focus search      Cmd+Shift+P  Export PDF        │   │
│                    │  └─────────────────────────────────────────────────────────┘   │
│                    │                                                                │
├────────────────────┴────────────────────────────────────────────────────────────────┤
│  Last synced: 5 min ago · 491 sessions                                              │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

---

### B10. Feature Preservation Checklist

Phase 3 is **purely additive**. No existing features are removed.

#### Unchanged Components (18)

| Component | Status |
|-----------|--------|
| App.tsx (layout shell) | ✅ Keep |
| Header.tsx (breadcrumbs, search, settings) | ✅ Keep |
| Sidebar.tsx (project tree) | ✅ Keep |
| StatusBar.tsx | ✅ Keep |
| CommandPalette.tsx | ✅ Keep |
| SearchResults.tsx | ✅ Keep |
| ProjectView.tsx | ✅ Keep |
| DateGroupedList.tsx | ✅ Keep |
| Message.tsx | ✅ Keep |
| ToolBadge.tsx | ✅ Keep |
| CodeBlock.tsx | ✅ Keep |
| ThinkingBlock.tsx | ✅ Keep |
| XmlCard.tsx | ✅ Keep |
| ActivityCalendar.tsx | ✅ Keep |
| ActivitySparkline.tsx | ✅ Keep |
| MiniHeatmap.tsx | ✅ Keep |
| HealthIndicator.tsx | ✅ Keep |
| lib/export-html.ts (HTML/PDF export) | ✅ Keep |

#### Enhanced Components (4)

| Component | Existing Features | Phase 3 Adds |
|-----------|-------------------|--------------|
| StatsDashboard.tsx | Sessions/projects count, Top Skills, Top Projects, Activity Heatmap, Tool Usage | 6 metric cards with trends, Recent Commits |
| SessionCard.tsx | Project badge, timestamp, preview, last message, files touched, tool counts, message/turns, skills | Atomic metrics row, commit badge, time range format |
| HistoryView.tsx | ActivitySparkline, search, time filter, project filter, date groups | Filter: Has commits, High re-edit, Long sessions; Sort dropdown |
| ConversationView.tsx | Back button, project name, **HTML/PDF export buttons**, Virtuoso messages, metadata footer | Metrics sidebar panel |

#### New Components (4)

| Component | Description |
|-----------|-------------|
| Settings.tsx | Data status, Git sync, Export (JSON/CSV), About |
| MetricCard.tsx | Value + trend display |
| CommitsPanel.tsx | Linked commits with tier badges |
| FilesTouchedPanel.tsx | Read/edit counts |

---

### B11. Frontend Implementation Steps

| # | Step | Description | Acceptance |
|---|------|-------------|------------|
| 29 | Types: update TypeScript | Add all new fields from ts-rs | Types compile |
| 30 | Hook: useDashboardStats (extended) | Fetch new metrics + trends | Hook works |
| 31 | Hook: useTrends | Fetch GET /api/trends | Hook works |
| 32 | Hook: useExport | Download export files | Hook works |
| 33 | Hook: useStatus | Fetch data freshness | Hook works |
| 34 | Hook: useGitSync | POST /api/sync/git with loading state | Hook works |
| 35 | Component: MetricCard | Display value + trend | Renders correctly |
| 36 | Component: Dashboard metrics grid | 6 metric cards | Responsive layout |
| 37 | Component: Recent Commits section | Last 5 linked commits | Empty state handled |
| 38 | Component: Session metrics bar | 5 metrics horizontal | Renders correctly |
| 39 | Component: Files touched panel | List with counts | Empty state handled |
| 40 | Component: Commits panel | List with tier badges | Empty state handled |
| 41 | Component: SessionCard metrics row | Inline metrics + time range | Renders correctly |
| 42 | Component: Filter dropdown | URL param persistence | Filter works |
| 43 | Component: Sort dropdown | URL param persistence | Sort works |
| 44 | Component: Settings page | Data status, Git sync, Export | Route works |
| 45 | Component: Data freshness footer | Last synced display | Updates on index |
| 46 | Accessibility audit | All WCAG requirements | Passes audit |
| 47 | Loading states | Skeletons everywhere | No blank screens |
| 48 | E2E tests | Full user flows | Playwright passes |

---

## Testing Strategy

| Type | Coverage |
|------|----------|
| Unit tests | Extraction functions, derived metrics, git correlation |
| Integration tests | DB queries, API endpoints |
| Golden tests | Fixture sessions with known values |
| E2E tests | Dashboard load, session detail, export download |
| Accessibility tests | axe-core audit |

---

## Performance Targets

| Operation | Target |
|-----------|--------|
| Session list with metrics | < 50ms |
| Session detail with files/commits | < 100ms |
| Dashboard stats + trends | < 100ms |
| Git scan (1000 commits) | < 2s |
| Export 500 sessions JSON | < 500ms |
| Export 500 sessions CSV | < 500ms |

---

## Not In Scope (Deferred)

| Feature | Reason | When |
|---------|--------|------|
| Health labels (Smooth/Turbulent) | No judgment, metrics only | If users request |
| LLM analysis | Build data foundation first | Phase 5+ |
| Onboarding/tutorials | Focus on core functionality | Phase 5+ |
| PDF export | Requires layout decisions | Enterprise tier |
| File overlap git matching | Too fuzzy | Never |
| 5-min/2-hour commit windows | Unprovable | Never |

---

## Dependencies

- Phase 2B (turns, tokens) — **DONE**
- Phase 2C (dashboard API) — **DONE**
- Phase 2A-2 (invocations for /commit detection) — **DONE**

All dependencies satisfied. Phase 3 can start immediately.
