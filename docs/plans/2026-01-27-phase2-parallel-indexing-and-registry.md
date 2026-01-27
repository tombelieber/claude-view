---
status: approved
date: 2026-01-27
supersedes: 2026-01-27-startup-ux-parallel-indexing.md
reviewed: 2026-01-28
---

# Phase 2: Parallel Indexing + Invocable Registry

> Combined design: instant startup via `sessions-index.json`, parallel JSONL deep indexing with invocation extraction, and plugin registry for skill/tool usage analytics.

**Supersedes:** `2026-01-27-startup-ux-parallel-indexing.md` (merged into this doc)

---

## Problem

Two problems solved together because they share the same JSONL parsing pipeline:

1. **Startup is unusable.** Current indexer reads 807 MB of JSONL sequentially (~83s) before starting the server. At 30 GB, that's 50 minutes.

2. **No skill/tool visibility.** Users can't see which skills, commands, agents, or MCP tools they actually use, how often, or when.

**Why combined:** Both require parsing JSONL `tool_use` blocks. Pass 2 already reads every line — adding invocation classification costs ~0 extra I/O.

---

## Architecture Overview

```
main() {
  1. Open DB                                    // <1ms
  2. Create registry holder (empty)             // <1ms
  3. Start Axum server                          // <100ms
  4. Print "Ready" + URL                        // immediate
  5. tokio::spawn(background_index)             // non-blocking
}

background_index() {
  // Pass 1 and Registry build are INDEPENDENT — run in parallel
  let (pass1_result, registry) = tokio::join!(
    pass_1_read_indexes(),                      // <10ms — JSON files
    build_registry(),                           // <100ms — scan plugin dirs
  );
  registry_holder.write(Some(registry));        // single write, API routes unblocked

  Pass 2: parallel_deep_index(changed_files, &registry)
        → mmap + parse_bytes (zero-copy, no .to_vec())
        → classify tool_use blocks against registry
        → batch update sessions + insert invocations (single transaction)
}
```

**Key design decisions:**
- Registry builds in background, NOT blocking server startup (server-first principle)
- Pass 1 and registry build run concurrently via `tokio::join!` (no false dependency)
- API routes return 503 if registry not yet ready (`Arc<RwLock<Option<Registry>>>`)
- Pass 2 uses zero-copy mmap (never `.to_vec()`) and batched DB writes

### Three Startup Scenarios

| Scenario | UI behavior | Backend |
|----------|------------|---------|
| **First launch** (empty DB) | Full data at <100ms, extended fields fill in | Pass 1 + Pass 2 |
| **Returning** (no changes) | Instant from DB cache | Pass 1 diffs, finds 0 changes, skip Pass 2 |
| **Returning + changes** | Cached data + new sessions appear | Pass 1 diffs, Pass 2 for changed only |

---

## Part 1: Registry Construction

### Source: `~/.claude/plugins/installed_plugins.json`

```json
{
  "version": 2,
  "plugins": {
    "superpowers@superpowers-marketplace": [
      {
        "scope": "user",
        "installPath": "/Users/.../.claude/plugins/cache/superpowers-marketplace/superpowers/4.0.3",
        "version": "4.0.3",
        "installedAt": "2025-12-03T15:26:33.551Z"
      }
    ]
  }
}
```

### Algorithm

1. Parse `installed_plugins.json` (version 2 format)
2. For each plugin entry:
   a. Read `{installPath}/plugin.json` for metadata (name, description)
   b. Scan `{installPath}/*/SKILL.md` → skill entries (flat layout, NOT nested `skills/` dir)
   c. Scan `{installPath}/commands/*.md` → command entries (if dir exists)
   d. Scan `{installPath}/agents/*.md` → agent entries (if dir exists)
   e. Read `{installPath}/.mcp.json` → MCP server definitions (if exists)
3. Build lookup maps
4. Hardcode built-in tools allowlist

**Verified paths** (2026-01-28 audit against real `~/.claude/plugins/`):
- `plugin.json` lives at **root** of installPath, NOT in `.claude-plugin/` subdirectory
- Skills are **direct subdirectories** of installPath (e.g. `{installPath}/brainstorming/SKILL.md`), not nested under a `skills/` parent
- `.mcp.json` confirmed at root (verified with `context7` plugin)

### Registry Types

```rust
// crates/core/src/registry.rs

pub enum InvocableKind {
    Skill,
    Command,
    Agent,
    McpTool,
    BuiltinTool,
}

pub struct InvocableInfo {
    pub id: String,               // "superpowers:brainstorming" or "builtin:Bash"
    pub plugin_name: Option<String>,
    pub name: String,
    pub kind: InvocableKind,
    pub description: String,
}

pub struct Registry {
    /// "superpowers:brainstorming" → InvocableInfo
    qualified: HashMap<String, InvocableInfo>,
    /// "brainstorming" → Vec<InvocableInfo> (bare name → possibly multiple matches)
    bare: HashMap<String, Vec<InvocableInfo>>,
}
```

### ID Conventions

| Kind | ID Format | Example |
|------|-----------|---------|
| Skill | `plugin:name` | `superpowers:brainstorming` |
| Command | `plugin:name` | `commit-commands:commit` |
| Agent | `plugin:name` | `feature-dev:code-reviewer` |
| MCP Tool | `mcp:plugin:tool` | `mcp:playwright:browser_navigate` |
| Built-in | `builtin:Name` | `builtin:Bash` |

### Built-in Tools Allowlist

```rust
const BUILTIN_TOOLS: &[&str] = &[
    "Bash", "Read", "Write", "Edit", "Glob", "Grep",
    "Task", "TaskCreate", "TaskUpdate", "TaskList", "TaskGet", "TaskOutput", "TaskStop",
    "WebFetch", "WebSearch",
    "AskUserQuestion", "EnterPlanMode", "ExitPlanMode",
    "NotebookEdit", "ToolSearch",
];
```

### Edge Cases

| Case | Handling |
|------|----------|
| Plugin has no skills/commands/agents dir | Check for MCP in .mcp.json; skip if nothing |
| Skill layout varies (flat `*/SKILL.md` vs nested `skills/*/SKILL.md`) | Try flat first (verified layout), fall back to nested |
| `plugin.json` missing | Use plugin key from installed_plugins.json as name |
| Multiple install versions in cache | Only follow `installPath` from installed_plugins.json |
| Bare name ambiguous (two plugins define `commit`) | Pick first match, log warning |
| Plugin removed but has usage history | Keep invocation records; invocable status = historical |
| `Skill()` receives built-in name (e.g. `"bash"`) | Reject — don't count as skill usage |

---

## Part 2: Pass 1 — Session Index Reading

### Data Source: `sessions-index.json` (per project)

**Location:** `~/.claude/projects/<encoded-path>/sessions-index.json`

Each project has a pre-computed index with session metadata. Fields available for free (no JSONL parsing):

| Field | Source | Maps to |
|-------|--------|---------|
| `sessionId` | UUID | `SessionInfo.id` |
| `firstPrompt` | Truncated string | `SessionInfo.preview` |
| `summary` | Claude-generated | `SessionInfo.summary` (NEW) |
| `messageCount` | Integer | `SessionInfo.message_count` |
| `created` | ISO timestamp | `SessionInfo.first_message_at` |
| `modified` | ISO timestamp | `SessionInfo.modified_at` |
| `gitBranch` | String | `SessionInfo.git_branch` (NEW) |
| `isSidechain` | Boolean | `SessionInfo.is_sidechain` (NEW) |
| `projectPath` | Absolute path | `SessionInfo.project_path` |

### Algorithm

```
pass_1_read_indexes(base_dir, db):
  for each project_dir in base_dir:
    index_path = project_dir / "sessions-index.json"
    if !exists(index_path): continue  // graceful fallback

    entries = parse_json(index_path)  // Vec<SessionIndexEntry>
    for entry in entries:
      session = SessionInfo from entry fields
      db.upsert_session(session)      // deep_indexed_at = NULL

  report: "{N} projects, {M} sessions loaded in {T}ms"
```

### Performance

Independent of JSONL data size. 30 GB of sessions makes no difference.

| Dataset | Files to read | Time |
|---------|--------------|------|
| 10 projects | 10 JSON files (~50 KB) | <10ms |
| 50 projects | 50 JSON files (~250 KB) | <20ms |

---

## Part 3: Pass 2 — Deep JSONL Index + Invocation Extraction

Only needed for fields NOT in `sessions-index.json`: tool_counts, skills_used, files_touched, last_message, turn_count, **and invocations**.

### Pipeline

```
changed_files[] ──┬── spawn_blocking ── mmap(file) → parse_bytes(&[u8], &registry)
                  ├── spawn_blocking ── mmap(file) → parse_bytes(&[u8], &registry)
                  └── ... (semaphore-bounded to num_cpus)
                             │
                             ▼ returns (ExtendedMetadata, Vec<RawInvocation>)
                     BEGIN TRANSACTION
                       UPDATE sessions SET tool_counts=?, ... WHERE id=?  (×N)
                       INSERT INTO invocations ...                        (×M)
                     COMMIT
```

### `parse_bytes()` — Extended with Invocation Collection

```rust
struct ParseResult {
    metadata: ExtendedMetadata,      // tool_counts, skills, files, last_message, turn_count
    raw_invocations: Vec<RawToolUse>, // name + input JSON for each tool_use block
}

fn parse_bytes(data: &[u8]) -> ParseResult {
    let mut meta = ExtendedMetadata::default();
    let mut raw_invocations = Vec::new();

    // ALL Finders created ONCE here — never inside per-line functions
    // (memmem::Finder::new pre-computes SIMD lookup tables)
    let user_finder = memmem::Finder::new(b"\"type\":\"user\"");
    let asst_finder = memmem::Finder::new(b"\"type\":\"assistant\"");
    let content_finder = memmem::Finder::new(b"\"content\":\"");
    let text_finder = memmem::Finder::new(b"\"text\":\"");
    let file_path_finder = memmem::Finder::new(b"\"file_path\":\"");
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");
    let skill_finder = memmem::Finder::new(b"\"name\":\"Skill\"");
    let task_finder = memmem::Finder::new(b"\"name\":\"Task\"");
    let mcp_finder = memmem::Finder::new(b"\"name\":\"mcp__plugin_");
    // Tool count finders also hoisted (existing)
    let read_finder = memmem::Finder::new(b"\"Read\"");
    let edit_finder = memmem::Finder::new(b"\"Edit\"");
    let write_finder = memmem::Finder::new(b"\"Write\"");
    let bash_finder = memmem::Finder::new(b"\"Bash\"");

    for line in split_lines_simd(data) {
        if line.is_empty() { continue; }

        // Existing: count user/assistant messages, extract metadata
        // ...existing parse_bytes logic, but pass finders by &reference...

        // NEW: collect raw tool_use blocks for invocation classification
        if skill_finder.find(line).is_some()
            || task_finder.find(line).is_some()
            || mcp_finder.find(line).is_some()
        {
            if let Some(tool_uses) = extract_tool_use_blocks(line) {
                raw_invocations.extend(tool_uses);
            }
        }
    }

    ParseResult { metadata: meta, raw_invocations }
}
```

**Performance rules applied:**
1. **SIMD pre-filter:** Only lines matching `"name":"Skill"` / `"Task"` / `"mcp__plugin_` are JSON-parsed. Most lines skip (~98%).
2. **Finders hoisted:** All `memmem::Finder` instances created once at top, passed by `&reference`. Never recreated per-line.
3. **Zero-copy mmap:** Caller passes `&[u8]` from mmap directly — never `.to_vec()`.

### Caller: zero-copy mmap integration

```rust
// In pass_2_deep_index, each spawn_blocking does:
let mmap = unsafe { Mmap::map(&file)? };
let result = parse_bytes(&mmap);  // zero-copy, mmap drops after return
// NOT: let data = mmap.to_vec(); parse_bytes(&data);  — this defeats mmap
```

### Invocation Classification (after parsing)

```rust
// crates/core/src/invocation.rs

pub enum ClassifyResult {
    Valid { invocable_id: String, kind: InvocableKind },
    Rejected { raw_value: String, reason: String },
    Ignored, // unknown tool, silently discard
}

pub fn classify_tool_use(
    name: &str,
    input: &Option<serde_json::Value>,
    registry: &Registry,
) -> ClassifyResult {
    match name {
        "Skill" => {
            let skill_name = input.as_ref()
                .and_then(|v| v.get("skill"))
                .and_then(|v| v.as_str());
            match skill_name {
                Some(s) if BUILTIN_TOOLS.contains(&s) =>
                    ClassifyResult::Rejected { raw_value: s.into(), reason: "builtin_misroute".into() },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid { invocable_id: info.id.clone(), kind: info.kind },
                    None => ClassifyResult::Rejected { raw_value: s.into(), reason: "not_in_registry".into() },
                },
                None => ClassifyResult::Ignored,
            }
        }
        "Task" => {
            let agent_type = input.as_ref()
                .and_then(|v| v.get("subagent_type"))
                .and_then(|v| v.as_str());
            match agent_type {
                Some(s) if is_builtin_agent(s) =>
                    ClassifyResult::Valid { invocable_id: format!("builtin:{s}"), kind: InvocableKind::BuiltinTool },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid { invocable_id: info.id.clone(), kind: info.kind },
                    None => ClassifyResult::Rejected { raw_value: s.into(), reason: "not_in_registry".into() },
                },
                None => ClassifyResult::Ignored,
            }
        }
        n if n.starts_with("mcp__plugin_") => {
            match parse_mcp_tool_name(n) {
                Some((plugin, tool)) => match registry.lookup_mcp(&plugin, &tool) {
                    Some(info) => ClassifyResult::Valid { invocable_id: info.id.clone(), kind: InvocableKind::McpTool },
                    None => ClassifyResult::Rejected { raw_value: n.into(), reason: "not_in_registry".into() },
                },
                None => ClassifyResult::Ignored,
            }
        }
        n if BUILTIN_TOOLS.contains(&n) =>
            ClassifyResult::Valid { invocable_id: format!("builtin:{n}"), kind: InvocableKind::BuiltinTool },
        _ => ClassifyResult::Ignored,
    }
}
```

### Performance Targets

**Pass 2 (JSONL deep parse + invocation extraction, background):**

| Data size | Time |
|-----------|------|
| 807 MB (now) | <1s |
| 10 GB | <5s |
| 30 GB | <10s |

Invocation classification adds negligible overhead — it's a HashMap lookup per tool_use block, and SIMD pre-filtering means most lines never trigger JSON parsing.

---

## Part 4: Database Schema

### Migration 4 (ALREADY SHIPPED): Session columns

These columns already exist in production (Migration 4 in `migrations.rs`). No action needed.

```sql
-- ALREADY DONE — do not re-add
ALTER TABLE sessions ADD COLUMN summary TEXT;
ALTER TABLE sessions ADD COLUMN git_branch TEXT;
ALTER TABLE sessions ADD COLUMN is_sidechain BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN deep_indexed_at INTEGER;
```

### Migration 5 (NEW): Invocables + Invocations

```sql
-- Plugin registry snapshot (rebuilt on startup)
CREATE TABLE IF NOT EXISTS invocables (
    id          TEXT PRIMARY KEY,    -- "superpowers:brainstorming" or "builtin:Bash"
    plugin_name TEXT,                -- NULL for built-ins
    name        TEXT NOT NULL,       -- "brainstorming"
    kind        TEXT NOT NULL,       -- skill|command|agent|mcp_tool|builtin_tool
    description TEXT DEFAULT '',
    status      TEXT DEFAULT 'enabled'  -- enabled|historical
);

-- Individual invocation records
CREATE TABLE IF NOT EXISTS invocations (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    invocable_id TEXT NOT NULL REFERENCES invocables(id),
    session_id   TEXT NOT NULL,
    project      TEXT NOT NULL,
    timestamp    INTEGER NOT NULL,   -- epoch ms
    PRIMARY KEY (source_file, byte_offset)
);

CREATE INDEX IF NOT EXISTS idx_invocations_invocable ON invocations(invocable_id);
CREATE INDEX IF NOT EXISTS idx_invocations_session   ON invocations(session_id);
CREATE INDEX IF NOT EXISTS idx_invocations_timestamp ON invocations(timestamp);
```

**What's NOT included** (cut from original PRD):
- No `sources` table (only Claude Code for now)
- No `turns` table (per-session aggregates suffice)
- No `rejected_invocations` table (log to tracing instead)
- No `invocable_stats` materialized table (join query is fast enough for MVP)
- No `models` table (defer to Chunk B)

---

## Part 5: API Endpoints

### Existing (unchanged)

| Endpoint | Purpose |
|----------|---------|
| `GET /api/health` | Health check |
| `GET /api/projects` | List projects with sessions (now includes summary, git_branch, is_sidechain) |

### New

| Endpoint | Purpose |
|----------|---------|
| `GET /api/invocables` | List registered invocables with usage counts |
| `GET /api/stats/overview` | Dashboard summary: totals by kind, top 10 |
| `GET /api/indexing/progress` | SSE stream for indexing progress |
| `POST /api/sync` | Trigger incremental re-index |

### `GET /api/invocables` Response

```json
{
  "invocables": [
    {
      "id": "superpowers:brainstorming",
      "pluginName": "superpowers",
      "name": "brainstorming",
      "kind": "skill",
      "description": "Explores user intent before implementation",
      "status": "enabled",
      "totalCount": 23,
      "sessionCount": 18,
      "lastUsed": "2026-01-27T17:30:00Z"
    }
  ]
}
```

### `GET /api/stats/overview` Response

```json
{
  "totalInvocations": 2341,
  "byKind": {
    "skill": { "registered": 14, "used": 8, "invocations": 234 },
    "command": { "registered": 12, "used": 6, "invocations": 189 },
    "agent": { "registered": 18, "used": 5, "invocations": 42 },
    "mcpTool": { "registered": 24, "used": 8, "invocations": 156 },
    "builtinTool": { "registered": 19, "used": 15, "invocations": 1720 }
  },
  "top10": [
    { "id": "builtin:Bash", "kind": "builtinTool", "count": 1058 }
  ]
}
```

### `GET /api/indexing/progress` (SSE)

```
event: ready
data: {"status":"ready","projects":10,"sessions":542}

event: deep-progress
data: {"status":"deep-indexing","indexed":42,"total":542}

event: done
data: {"status":"done","indexed":542,"total":542,"durationMs":800}
```

---

## Part 6: Shared State

### IndexingState

```rust
pub struct IndexingState {
    pub status: AtomicU8,           // 0=idle, 1=reading-indexes, 2=deep-indexing, 3=done, 4=error
    pub total: AtomicUsize,
    pub indexed: AtomicUsize,
    pub projects_found: AtomicUsize,
    pub sessions_found: AtomicUsize,
    pub error: RwLock<Option<String>>,
}
```

### AppState (modified)

```rust
pub struct AppState {
    pub start_time: Instant,
    pub db: Database,
    pub indexing: Arc<IndexingState>,
    pub registry: Arc<RwLock<Option<Registry>>>,  // NEW — None until background build completes
}
```

**Why `RwLock<Option<...>>`:** Registry builds in background (not blocking startup). API routes check `registry.read()`:
- `None` → return 503 "Indexing in progress"
- `Some(r)` → normal response

Written exactly once (after `build_registry()` completes). All subsequent access is read-only — `RwLock` readers never block each other.

---

## Part 7: SessionInfo Changes

```rust
pub struct SessionInfo {
    // existing fields unchanged
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub file_path: String,
    pub modified_at: i64,
    pub size_bytes: u64,
    pub preview: String,
    pub last_message: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
    pub tool_counts: ToolCounts,
    pub message_count: usize,
    pub turn_count: usize,

    // NEW from Pass 1 (sessions-index.json)
    pub summary: Option<String>,
    pub git_branch: Option<String>,
    pub is_sidechain: bool,
    pub deep_indexed: bool,           // true after Pass 2 completes
}
```

---

## Implementation Plan

### New Files

| File | Purpose |
|------|---------|
| `crates/core/src/registry.rs` | Parse `installed_plugins.json`, scan plugin dirs, build lookup maps |
| `crates/core/src/invocation.rs` | `classify_tool_use()`, `BUILTIN_TOOLS` allowlist, `RawToolUse` type |
| `crates/db/src/indexer_parallel.rs` | Two-pass: read indexes + parallel JSONL with invocations |
| `crates/server/src/routes/invocables.rs` | `/api/invocables`, `/api/stats/overview` |
| `crates/server/src/routes/indexing.rs` | SSE endpoint `GET /api/indexing/progress` |

### Modified Files

| File | Changes |
|------|---------|
| `crates/core/src/lib.rs` | Export `registry`, `invocation` modules |
| `crates/core/src/types.rs` | Existing — `summary`, `git_branch`, `is_sidechain`, `deep_indexed` already added |
| `crates/db/src/migrations.rs` | Add migration 5 (session columns) + migration 6 (invocables + invocations) |
| `crates/db/src/queries.rs` | Add `insert_invocable()`, `batch_insert_invocations()`, `list_invocables_with_counts()` |
| `crates/server/src/main.rs` | Build registry at startup, spawn background indexer |
| `crates/server/src/state.rs` | Add `Arc<Registry>` to AppState |
| `crates/server/src/lib.rs` | Register new routes |
| `crates/db/Cargo.toml` | `memmap2`, `memchr` (already added) |
| `crates/server/Cargo.toml` | `tokio-stream` for SSE (already added) |

### Steps (ordered by dependency)

Phase 2A-1 steps (1, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17) are **DONE**. Remaining Phase 2A-2 steps:

| Step | Depends on | Deliverable |
|------|-----------|-------------|
| 2. `registry.rs` — parse installed_plugins.json + scan dirs | — | Registry struct + tests |
| 3. `invocation.rs` — classify_tool_use + BUILTIN_TOOLS | Step 2 | Classification logic + tests |
| 4. Migration 5 — invocables/invocations tables | — | Schema SQL (session columns already in Migration 4) |
| 5. Update queries.rs — invocable + invocation CRUD + batch writes | Steps 3, 4 | DB operations |
| P1. Fix `read_file_fast()` — zero-copy mmap (remove `.to_vec()`) | — | Perf fix |
| P2. Fix `parse_bytes()` — hoist all Finders, pass by &ref | — | Perf fix |
| 9. Extend `parse_bytes()` → `ParseResult` with raw_invocations | Steps P1, P2 | Extended parser |
| 9b. Integrate invocations into `pass_2_deep_index()` | Steps 3, 5, 9 | Classify + batch DB in single txn |
| 10b. Update `run_background_index()` — `tokio::join!` Pass 1 + Registry | Step 2 | Parallel startup |
| 11b. Update AppState — `Arc<RwLock<Option<Registry>>>` | Step 2 | Shared state |
| 12b. Update `main.rs` — registry holder, pass to background | Steps 10b, 11b | Startup flow |
| 14. Routes: `/api/invocables`, `/api/stats/overview` | Step 5 | New API endpoints |

**Parallelizable work (start simultaneously):**
- Steps 2 + 4 + P1 + P2 (all independent)
- Then: Step 3 (needs 2) → Step 5 (needs 3+4) → Steps 9, 9b, 10b, 11b, 12b, 14

### What Stays the Same

- `scan_files()` — still used by Pass 2 to find JSONL files
- `diff_against_db()` — used to detect changes
- Existing routes (`/api/projects`, `/api/health`)
- `indexer.rs` — kept as reference implementation
- `extract_session_metadata()` — kept as golden reference for correctness tests

---

## Acceptance Criteria

### Registry

- **AC-R1:** Registry discovers all skills, commands, agents from installed plugins
- **AC-R2:** Built-in tools allowlist matches all known Claude Code tools
- **AC-R3:** Registry handles missing dirs, missing plugin.json gracefully
- **AC-R4:** Bare name lookup resolves `"commit"` → `"commit-commands:commit"`

### Invocations

- **AC-I1:** `classify_tool_use("Skill", {"skill":"superpowers:brainstorming"})` → Valid
- **AC-I2:** `classify_tool_use("Skill", {"skill":"bash"})` → Rejected (builtin_misroute)
- **AC-I3:** `classify_tool_use("Skill", {"skill":"nonexistent"})` → Rejected (not_in_registry)
- **AC-I4:** `classify_tool_use("Bash", ...)` → Valid (builtin:Bash)
- **AC-I5:** MCP tool name parsed correctly: `mcp__plugin_playwright_playwright__browser_navigate`
- **AC-I6:** Zero false positives — golden test against real session data

### Startup UX

- **AC-S1:** Server starts before indexing (`/api/health` returns 200 immediately)
- **AC-S2:** Pass 1 reads sessions-index.json correctly (all fields parsed)
- **AC-S3:** Pass 1 handles missing/malformed index files gracefully
- **AC-S4:** Pass 2 fills extended metadata + invocations
- **AC-S5:** Subsequent launches skip Pass 2 when no files changed
- **AC-S6:** SSE endpoint streams progress events

### Performance (not CI-blocking)

| Metric | Target |
|--------|--------|
| Server ready time | <500ms |
| Pass 1 (10 projects) | <10ms |
| Pass 2 (807 MB) | <1s |
| Subsequent launch (no changes) | <10ms |
| `/api/projects` response | <100ms |
| `/api/invocables` response | <100ms |
| Registry construction | <100ms |

---

## Performance Fixes (included in this plan)

Identified during 2026-01-28 review. These fix existing code and are prerequisites for the invocation work.

| Fix | File | Issue | Expected impact |
|-----|------|-------|-----------------|
| **P1: Zero-copy mmap** | `indexer_parallel.rs:45` | `mmap.to_vec()` copies entire file to heap, defeating mmap | ~30-40% on large files |
| **P2: Hoist Finders** | `indexer_parallel.rs:153,199,218` | `memmem::Finder::new()` inside per-line functions rebuilds SIMD tables ~100k times | ~10-15% |
| **P3: Batch DB writes** | `indexer_parallel.rs:392` | Individual `UPDATE` per session (491 implicit transactions) | ~20-30% |
| **P4: Non-blocking registry** | `main.rs` (plan fix) | Registry was blocking startup; moved to background `tokio::join!` with Pass 1 | Startup unblocked |

**Combined target:** 1.8s → ~0.7-1.0s for 807MB dataset. Theoretical floor ~520ms (SSD + SIMD + batch DB).

---

## Deferred Items

Items intentionally excluded from this plan. Each is fully designed in its source document — implement when this plan ships.

### Chunk B: Token & Model Tracking

**Source:** `2026-01-27-skills-usage-analytics-prd.md` §7-8

| Item | What it adds | Why deferred |
|------|-------------|--------------|
| `models` table | Normalize model IDs across sessions (`claude-opus-4-5-20251101`, etc.) | No value without per-turn data |
| `turns` table | One row per assistant response: model_id, input/output tokens, cache tokens, duration, stop_reason, thinking_level | Large table; per-session aggregates suffice for MVP |
| Session token aggregates | `total_input_tokens`, `total_output_tokens`, `total_cache_read_tokens` on `sessions` | Requires turns extraction pipeline |
| `primary_model` on sessions | Most-used model per session | Requires model tracking |
| `/api/stats/models` | Per-model usage stats (turns, tokens, sessions) | Requires models + turns tables |
| `/api/stats/tokens` | Token economics, cache hit ratio | Requires token aggregates |

**Prerequisite:** This plan's Pass 2 pipeline. Chunk B extends `parse_bytes()` to also extract `message.model` and `message.usage.*` fields.

### Chunk C: Session Health & Git Correlation

**Source:** `2026-01-27-vibe-recall-analytics-design.md` §5-7

| Item | What it adds | Why deferred |
|------|-------------|--------------|
| Circle-back detection | Identify when same file is edited again after 3+ turns | Requires files_touched per turn (not just per session) |
| Session health classification | Smooth / Neutral / Turbulent based on turn count, duration, circle-back rate | Requires circle-back + git correlation |
| Git commit correlation | Match commits to sessions by file overlap + recency | New `commits` + `session_commits` tables, git scanning |
| `daily_stats` table | Pre-aggregated daily metrics for fast dashboard | Requires health classification |
| `/api/stats/daily` | Daily breakdown for charts | Requires daily_stats |
| `/api/sessions/:id/commits` | Commits linked to a session | Requires git correlation |
| Insights generation | Rule-based patterns ("smoothest sessions start with /brainstorm") | Requires health + invocation data |

**Prerequisite:** This plan (invocation data) + Chunk B (turn-level metrics).

### Infrastructure Deferred

| Item | Source | Why deferred |
|------|--------|--------------|
| `sources` table + multi-AI | PRD §7.1, §8.4 | Only Claude Code exists; add when second AI tool supported |
| `rejected_invocations` table | PRD §8.1 | Log to tracing instead; persist if diagnostics prove needed |
| `invocable_stats` materialized view | PRD §8.3 | Join query is fast enough for MVP; materialize when slow |
| `notify` file watcher | v2 design §4.3 | Phase 4 of v2 roadmap |
| API pagination for `/api/projects` | API schema design doc | Response size optimization is separate concern |

---

*2026-01-27*
