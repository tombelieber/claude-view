---
status: draft
date: 2026-01-27
---

# Skills & Plugin Usage Analytics — PRD

> Robust, registry-validated analytics for Claude Code plugin usage (skills, commands, agents, MCP tools, built-in tools)

**Author:** Anonymous
**Version:** 2.0 (Registry-Validated, All Invocable Types)

---

## 1. Problem Statement

Claude Code users install and use various plugins, but currently lack any reliable visibility into:

- Which skills/commands they actually use (vs. just installed)
- Agent and MCP tool usage patterns
- Built-in tool usage frequency
- Which skills correlate with productive sessions
- Workflow adoption over time

The key challenge is **accurate detection**: Claude Code has many slash commands (`/model`, `/clear`), file paths appear in messages (`/Users/...`), and the `Skill()` tool sometimes receives built-in tool names as input. **Naive pattern matching produces false positives.**

---

## 2. Goals & Non-Goals

### 2.1 Goals

| ID | Goal | Metric |
|----|------|--------|
| G1 | **Zero false positives** in skill/command counting | 0 built-in commands or file paths counted |
| G2 | **Track all 5 invocable types** | Skills, commands, agents, MCP tools, built-in tools |
| G3 | **Registry is sole authority** | Every counted invocation validated against on-disk registry |
| G4 | **Historical stability** | Removing a plugin does not erase usage history |
| G5 | **Incremental sync** | < 200ms typical sync for append-only JSONL files |

### 2.2 Non-Goals (v1)

- Team / organization aggregation
- Real-time streaming updates
- Skill recommendations or suggestions
- Cross-machine or cloud sync
- Tracking built-in slash commands (`/clear`, `/model`) as skills

---

## 3. Core Principles (Non-Negotiable)

1. **Registry is the source of truth.** A skill invocation is counted *only if* it resolves to a real skill/command found on disk.
2. **Regex is only a candidate extractor.** Pattern matching alone is never sufficient.
3. **Session JSONL tool_use records are the extraction source.** Not the `display` field in `history.jsonl`.
4. **Discard unknowns, never guess.** Any tool call that doesn't match a known category is silently dropped.
5. **History is append-only and authoritative.** Incremental parsing must be idempotent and deterministic.

---

## 4. Five Invocable Types

### 4.1 Overview

| Category | Where Defined | How Invoked | JSONL Signal |
|----------|--------------|-------------|--------------|
| **Skill** | `skills/*/SKILL.md` | `Skill()` tool call | `{"name":"Skill","input":{"skill":"superpowers:brainstorming"}}` |
| **Command** | `commands/*.md` | `Skill()` tool call (same) | `{"name":"Skill","input":{"skill":"commit"}}` |
| **Agent** | `agents/*.md` | `Task()` tool call | `{"name":"Task","input":{"subagent_type":"feature-dev:code-reviewer"}}` |
| **MCP Tool** | `plugin.json` mcpServers | `mcp__*` tool call | `{"name":"mcp__plugin_playwright_playwright__browser_navigate"}` |
| **Built-in Tool** | Hardcoded allowlist | Direct tool call | `{"name":"Bash"}`, `{"name":"Read"}`, etc. |

### 4.2 Why Both Skills and Commands

Plugins expose functionality through **two** directory conventions discovered on the real filesystem:

| Directory | Purpose | File Format | Example |
|-----------|---------|-------------|---------|
| `skills/<name>/SKILL.md` | Background behaviors, proactive knowledge | YAML frontmatter + markdown | `superpowers:brainstorming` |
| `commands/<name>.md` | User-invoked slash commands | YAML frontmatter + markdown | `commit-commands:commit` |

Some plugins have both (e.g., `superpowers` has `commands/brainstorm.md` AND `skills/brainstorming/SKILL.md`). Some have only one. The registry must discover both.

---

## 5. Skill & Command Registry (Source of Truth)

### 5.1 Registry Construction Algorithm

At application startup, build an **immutable registry snapshot**:

1. Parse `~/.claude/plugins/installed_plugins.json` (version 2 format)
2. For each plugin entry:
   a. Resolve `installPath`
   b. Verify path exists on disk (skip if missing/broken)
   c. Scan `{installPath}/skills/*/SKILL.md` → skill entries
   d. Scan `{installPath}/commands/*.md` → command entries
   e. Scan `{installPath}/.claude/skills/*/SKILL.md` → alt skill location (e.g., `ui-ux-pro-max`)
   f. Scan `{installPath}/agents/*.md` → agent entries
   g. Read `{installPath}/.claude-plugin/plugin.json` → extract `mcpServers` definitions
3. Parse YAML frontmatter from each file for metadata (name, description)
4. Build lookup maps (see 5.4)

### 5.2 Registry Entry

```rust
enum InvocableKind {
    Skill,       // from skills/ directory
    Command,     // from commands/ directory
    Agent,       // from agents/ directory
    McpTool,     // from plugin.json mcpServers
    BuiltinTool, // hardcoded allowlist
}

enum InvocableStatus {
    Enabled,     // Plugin currently installed and enabled
    Disabled,    // Plugin installed but disabled
    Historical,  // Plugin removed, usage records remain
}

struct InvocableInfo {
    id: String,               // "superpowers:brainstorming" or "builtin:Bash"
    plugin_name: Option<String>,  // None for built-ins
    marketplace: Option<String>,  // "superpowers-marketplace"
    name: String,             // "brainstorming"
    kind: InvocableKind,
    description: String,      // from YAML frontmatter
    install_path: Option<PathBuf>,
    status: InvocableStatus,
}
```

### 5.3 ID Conventions

| Kind | ID Format | Example |
|------|-----------|---------|
| Skill/Command | `plugin:name` | `superpowers:brainstorming` |
| Agent | `plugin:agent-name` | `feature-dev:code-reviewer` |
| MCP Tool | `mcp:plugin:tool` | `mcp:playwright:browser_navigate` |
| Built-in | `builtin:Name` | `builtin:Bash` |

The prefix namespacing prevents collisions and enables trivial filtering (e.g., `WHERE id LIKE 'builtin:%'`).

### 5.4 Lookup Maps

Three maps handle all invocation patterns found in real data:

```rust
// Exact qualified match: "superpowers:brainstorming" → InvocableInfo
HashMap<String, InvocableInfo>

// Bare name match: "brainstorming" → Vec<InvocableInfo>
// (if two plugins define same bare name, prefer the one with matching scope)
HashMap<String, Vec<InvocableInfo>>

// Plugin listing: "superpowers" → [all its skills + commands + agents]
HashMap<String, Vec<InvocableInfo>>
```

### 5.5 Edge Cases (Observed on Real Filesystem)

| Case | Example | Handling |
|------|---------|----------|
| Plugin has no skills/commands/agents dir | `context7`, `github`, `playwright` | Check for MCP servers in plugin.json; skip if nothing found |
| Alt skill location `.claude/skills/` | `ui-ux-pro-max` | Check both `skills/` and `.claude/skills/` |
| Multiple install versions in cache | `commit-commands` has 3 versions | Use only the version referenced in `installed_plugins.json` |
| `temp_git_*` directories in cache | Build artifacts | Ignore — only follow `installPath` from registry |
| Plugin removed but has historical usage | Uninstalled plugin | Mark as `Historical`, keep counting |
| Plugin installed but disabled | User-disabled plugin | Mark as `Disabled`, still validate against |

---

## 6. Invocation Detection (from Session JSONL)

### 6.1 Data Source

Session JSONL files in `~/.claude/projects/<project-slug>/<session-id>.jsonl`. Each line is a JSON object. Tool calls appear as:

```json
{
  "message": {
    "content": [
      { "type": "tool_use", "name": "<tool_name>", "input": { ... } }
    ]
  }
}
```

### 6.2 Detection Rules by Category

| Category | JSONL Match | Extract | Registry Validate |
|----------|-------------|---------|-------------------|
| **Skill/Command** | `name == "Skill"` | `input.skill` → e.g. `"superpowers:brainstorming"` | Must exist in qualified OR bare name lookup |
| **Agent** | `name == "Task"` | `input.subagent_type` → e.g. `"feature-dev:code-reviewer"` | Plugin prefix must match registered plugin; agent name must exist in that plugin's `agents/` dir |
| **MCP Tool** | `name` starts with `"mcp__plugin_"` | Parse `mcp__plugin_{marketplace}_{plugin}__{tool}` | Plugin must exist in registry |
| **Built-in Tool** | `name` is in hardcoded allowlist | `name` directly | Matched against allowlist (no registry needed) |

### 6.3 Built-in Tool Allowlist

```rust
const BUILTIN_TOOLS: &[&str] = &[
    "Bash", "Read", "Write", "Edit", "Glob", "Grep",
    "Task", "TaskCreate", "TaskUpdate", "TaskList", "TaskGet", "TaskOutput", "TaskStop",
    "WebFetch", "WebSearch",
    "AskUserQuestion", "EnterPlanMode", "ExitPlanMode",
    "NotebookEdit",
];
```

Anything not in this list AND not matching the other three categories is **discarded** — never guessed.

### 6.4 Rejection Rules (Defensive)

A `Skill()` call with `input.skill` value is **rejected** (not counted as plugin usage) if:

- The value matches a built-in tool name (e.g. `"bash"`, `"edit"`, `"read"`, `"glob"`) — these are misrouted calls
- The value is not found in the registry (qualified or bare lookup)
- The value is empty or malformed

Rejected entries are stored in a `rejected_invocations` table for diagnostics, never counted.

### 6.5 Agent Detection Details

The `Task()` tool call contains `input.subagent_type` which has two formats:

| Format | Example | Meaning |
|--------|---------|---------|
| `plugin:agent` | `feature-dev:code-reviewer` | Plugin-provided agent |
| `bare_name` | `Explore`, `Plan`, `Bash` | Built-in agent type |

Built-in agent types (`Explore`, `Plan`, `Bash`, `general-purpose`, etc.) are categorized as built-in tools. Plugin agents are validated against the registry.

### 6.6 MCP Tool Name Parsing

MCP tool names follow the pattern: `mcp__plugin_{marketplace}_{plugin}__{tool_name}`

Example: `mcp__plugin_playwright_playwright__browser_navigate`
- marketplace: `playwright`
- plugin: `playwright`
- tool: `browser_navigate`

Parse by splitting on `__` (double underscore). Validate that the plugin exists in the registry.

### 6.7 Parsing Robustness

| Concern | Handling |
|---------|----------|
| Malformed JSON line | Skip line, increment `parse_errors` counter |
| Missing `message` or `content` field | Skip line |
| `tool_use` with no `input` | Skip entry |
| Duplicate entry (same `source_file` + `byte_offset`) | Idempotent — primary key dedup |
| File truncated mid-write | Stop at last valid line, record offset |
| New tool name we don't recognize | Discard — never guess |

---

## 7. Data Model

### 7.1 Source & Model Tracking

Every record is explicitly tagged with which LLM app produced it and which model(s) were used. This is foundational for multi-AI support.

```rust
/// Which LLM coding app produced this data
enum Source {
    ClaudeCode,   // ~/.claude/projects/
    CodexCli,     // ~/.codex/sessions/ (future)
    GeminiCli,    // ~/.gemini/tmp/ (future)
}

/// Model used for a specific assistant turn
struct ModelInfo {
    model_id: String,       // "claude-opus-4-5-20251101", "o3", "gemini-2.5-pro"
    provider: String,       // "anthropic", "openai", "google"
}
```

### 7.2 Core Entities

```rust
struct Invocable {
    id: String,               // "claude:superpowers:brainstorming" or "claude:builtin:Bash"
    source: Source,           // which LLM app this invocable belongs to
    plugin_name: Option<String>,
    marketplace: Option<String>,
    name: String,
    kind: InvocableKind,      // Skill | Command | Agent | McpTool | BuiltinTool
    description: String,
    status: InvocableStatus,  // Enabled | Disabled | Historical
}

struct Invocation {
    invocable_id: String,
    session_id: String,
    project: String,
    source: Source,           // which LLM app this invocation came from
    model_id: Option<String>, // model active when this tool was called
    timestamp: i64,           // epoch ms, from JSONL parent record
    source_file: String,      // JSONL file path
    byte_offset: u64,         // dedup key
}

struct Session {
    id: String,
    source: Source,           // which LLM app
    project: String,
    models_used: Vec<String>, // all models seen in this session (multi-model)
    primary_model: Option<String>, // most-used model in session
    app_version: Option<String>,   // e.g. Claude Code "2.1.19"
    // ... existing fields (turn_count, duration, etc.)
}

struct Turn {
    session_id: String,
    turn_number: u32,
    model_id: Option<String>,      // model for THIS turn (can differ per turn)
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
    duration_ms: Option<u64>,      // turn duration
    stop_reason: Option<String>,   // "end_turn", "stop_sequence"
    has_error: bool,               // API error on this turn
    is_sidechain: bool,            // subagent turn
    thinking_level: Option<String>, // "high", "low", None
}
```

### 7.3 ID Conventions (Updated with Source Prefix)

All IDs are now namespaced by source to prevent collisions across LLM apps:

| Kind | ID Format | Example |
|------|-----------|---------|
| Skill/Command | `source:plugin:name` | `claude:superpowers:brainstorming` |
| Agent | `source:plugin:agent` | `claude:feature-dev:code-reviewer` |
| MCP Tool | `source:mcp:plugin:tool` | `claude:mcp:playwright:browser_navigate` |
| Built-in | `source:builtin:Name` | `claude:builtin:Bash` |
| (Future) Codex Skill | `codex:skill:name` | `codex:skill:code-review` |
| (Future) Gemini Extension | `gemini:ext:name` | `gemini:ext:firebase` |

### 7.4 Relationships

```
Source    (1) ──< Session    (many)
Source    (1) ──< Invocable  (many)
Invocable (1) ──< Invocation (many)
Session   (1) ──< Turn       (many)
Session   (1) ──< Invocation (many)
Project   (1) ──< Session    (many)
Plugin    (1) ──< Invocable  (many)  [except built-ins]
Model     (1) ──< Turn       (many)
```

### 7.5 Token & Cost Metrics (Per Turn)

Available from Claude Code JSONL `message.usage` fields:

| Metric | Source Field | Analytics Use |
|--------|-------------|---------------|
| `input_tokens` | `usage.input_tokens` | Cost per turn |
| `output_tokens` | `usage.output_tokens` | Response verbosity |
| `cache_read_tokens` | `usage.cache_read_input_tokens` | Cache hit efficiency |
| `cache_creation_tokens` | `usage.cache_creation_input_tokens` | Cache warming cost |
| `duration_ms` | system message `durationMs` | Turn latency |
| `model_id` | `message.model` | Model per turn |
| `stop_reason` | `message.stop_reason` | Truncation detection |
| `has_error` | `error` field presence | Error rate |
| `thinking_level` | `thinkingMetadata.level` | Extended thinking usage |
| `is_sidechain` | `isSidechain` | Subagent work tracking |
| `git_branch` | `gitBranch` | Branch-level analytics |
| `app_version` | `version` | Version-over-version comparison |

---

## 8. Database Schema

### 8.1 Tables

```sql
-- ============================================================
-- SOURCE & MODEL TRACKING
-- ============================================================

-- LLM apps discovered on this machine
CREATE TABLE IF NOT EXISTS sources (
    id          TEXT PRIMARY KEY,    -- "claude_code", "codex_cli", "gemini_cli"
    display_name TEXT NOT NULL,      -- "Claude Code", "Codex CLI", "Gemini CLI"
    session_dir TEXT,                -- "~/.claude/projects/", "~/.codex/sessions/"
    detected_at INTEGER NOT NULL     -- epoch ms, when first discovered
);

-- Models observed across all sessions
CREATE TABLE IF NOT EXISTS models (
    id          TEXT PRIMARY KEY,    -- "claude-opus-4-5-20251101"
    provider    TEXT NOT NULL,       -- "anthropic", "openai", "google"
    display_name TEXT,               -- "Claude Opus 4.5" (derived or looked up)
    first_seen  INTEGER,
    last_seen   INTEGER
);

-- ============================================================
-- SESSIONS & TURNS (per-message granularity)
-- ============================================================

-- Session-level metadata (enriched during sync)
CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY,
    source          TEXT NOT NULL DEFAULT 'claude_code' REFERENCES sources(id),
    project         TEXT NOT NULL,
    title           TEXT,
    preview         TEXT,
    turn_count      INTEGER DEFAULT 0,
    file_count      INTEGER DEFAULT 0,
    first_message_at INTEGER,
    last_message_at  INTEGER,
    file_path       TEXT NOT NULL UNIQUE,
    file_hash       TEXT,
    indexed_at      INTEGER,
    -- Model tracking
    primary_model   TEXT REFERENCES models(id),   -- most-used model in session
    models_used     TEXT DEFAULT '[]',             -- JSON array of model IDs
    model_switch_count INTEGER DEFAULT 0,          -- times model changed mid-session
    -- App metadata
    app_version     TEXT,                          -- e.g. "2.1.19"
    git_branch      TEXT,                          -- branch at session start
    -- Token aggregates (sum across all turns)
    total_input_tokens  INTEGER DEFAULT 0,
    total_output_tokens INTEGER DEFAULT 0,
    total_cache_read_tokens INTEGER DEFAULT 0,
    total_cache_creation_tokens INTEGER DEFAULT 0,
    -- Error tracking
    error_count     INTEGER DEFAULT 0,
    -- Extended thinking
    thinking_turns  INTEGER DEFAULT 0              -- turns with thinking enabled
);

-- Per-turn metrics (one row per assistant response)
CREATE TABLE IF NOT EXISTS turns (
    session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_number     INTEGER NOT NULL,
    model_id        TEXT REFERENCES models(id),
    role            TEXT NOT NULL,                  -- "user", "assistant"
    timestamp       INTEGER,                       -- epoch ms
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    cache_read_tokens INTEGER,
    cache_creation_tokens INTEGER,
    duration_ms     INTEGER,                       -- turn duration
    stop_reason     TEXT,                           -- "end_turn", "stop_sequence"
    has_error       INTEGER DEFAULT 0,             -- 1 if API error
    is_sidechain    INTEGER DEFAULT 0,             -- 1 if subagent turn
    thinking_level  TEXT,                           -- "high", "low", NULL
    source_file     TEXT NOT NULL,
    byte_offset     INTEGER NOT NULL,
    PRIMARY KEY (session_id, turn_number)
);

-- ============================================================
-- INVOCABLE REGISTRY & INVOCATIONS
-- ============================================================

-- Plugin registry snapshot (rebuilt on startup)
CREATE TABLE IF NOT EXISTS invocables (
    id          TEXT PRIMARY KEY,    -- "claude:superpowers:brainstorming"
    source      TEXT NOT NULL DEFAULT 'claude_code' REFERENCES sources(id),
    plugin_name TEXT,                -- NULL for built-ins
    marketplace TEXT,
    name        TEXT NOT NULL,       -- "brainstorming"
    kind        TEXT NOT NULL,       -- skill|command|agent|mcp_tool|builtin_tool
    description TEXT DEFAULT '',
    status      TEXT DEFAULT 'enabled',  -- enabled|disabled|historical
    first_seen  INTEGER,            -- epoch ms, from first invocation
    last_seen   INTEGER             -- epoch ms, from latest invocation
);

-- Individual invocation records
CREATE TABLE IF NOT EXISTS invocations (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    invocable_id TEXT NOT NULL REFERENCES invocables(id),
    session_id   TEXT NOT NULL REFERENCES sessions(id),
    source       TEXT NOT NULL DEFAULT 'claude_code' REFERENCES sources(id),
    model_id     TEXT REFERENCES models(id),       -- model active when tool was called
    project      TEXT NOT NULL,
    timestamp    INTEGER NOT NULL,   -- epoch ms
    PRIMARY KEY (source_file, byte_offset)
);

-- Materialized aggregates (rebuilt after each sync)
CREATE TABLE IF NOT EXISTS invocable_stats (
    invocable_id  TEXT PRIMARY KEY REFERENCES invocables(id),
    total_count   INTEGER DEFAULT 0,
    session_count INTEGER DEFAULT 0,  -- distinct sessions
    project_count INTEGER DEFAULT 0,  -- distinct projects
    first_used    INTEGER,
    last_used     INTEGER,
    -- Model breakdown (JSON: {"claude-opus-4-5-20251101": 12, "claude-sonnet-4-20250514": 5})
    by_model      TEXT DEFAULT '{}'
);

-- Rejected invocations for diagnostics
CREATE TABLE IF NOT EXISTS rejected_invocations (
    source_file  TEXT NOT NULL,
    byte_offset  INTEGER NOT NULL,
    source       TEXT NOT NULL DEFAULT 'claude_code',
    raw_value    TEXT NOT NULL,       -- the unresolved skill/tool name
    reason       TEXT NOT NULL,       -- "not_in_registry", "builtin_misroute", "malformed"
    timestamp    INTEGER,
    PRIMARY KEY (source_file, byte_offset)
);

-- Sync state for incremental parsing
CREATE TABLE IF NOT EXISTS invocation_sync_state (
    source_file  TEXT PRIMARY KEY,
    source       TEXT NOT NULL DEFAULT 'claude_code',
    last_offset  INTEGER NOT NULL DEFAULT 0,
    last_synced  INTEGER NOT NULL,
    parse_errors INTEGER DEFAULT 0
);
```

### 8.2 Indexes

```sql
-- Invocable lookups
CREATE INDEX IF NOT EXISTS idx_invocables_kind       ON invocables(kind);
CREATE INDEX IF NOT EXISTS idx_invocables_source     ON invocables(source);
CREATE INDEX IF NOT EXISTS idx_invocables_status     ON invocables(status);

-- Invocation lookups
CREATE INDEX IF NOT EXISTS idx_invocations_invocable ON invocations(invocable_id);
CREATE INDEX IF NOT EXISTS idx_invocations_session   ON invocations(session_id);
CREATE INDEX IF NOT EXISTS idx_invocations_project   ON invocations(project);
CREATE INDEX IF NOT EXISTS idx_invocations_timestamp ON invocations(timestamp);
CREATE INDEX IF NOT EXISTS idx_invocations_source    ON invocations(source);
CREATE INDEX IF NOT EXISTS idx_invocations_model     ON invocations(model_id);

-- Session lookups
CREATE INDEX IF NOT EXISTS idx_sessions_source       ON sessions(source);
CREATE INDEX IF NOT EXISTS idx_sessions_project      ON sessions(project);
CREATE INDEX IF NOT EXISTS idx_sessions_model        ON sessions(primary_model);
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC);

-- Turn lookups
CREATE INDEX IF NOT EXISTS idx_turns_model           ON turns(model_id);
CREATE INDEX IF NOT EXISTS idx_turns_session         ON turns(session_id);
```

### 8.3 Why Materialized Stats

Aggregating over raw invocations on every query is wasteful for dashboards. `invocable_stats` is rebuilt after each sync via `INSERT OR REPLACE` from aggregate queries. This keeps dashboard queries under 5ms regardless of history size.

### 8.4 Multi-AI Scalability

The schema is multi-AI ready from day one:

| Concern | How It's Handled |
|---------|-----------------|
| Different session formats | `sources` table + `source` FK on every record |
| Different model providers | `models` table normalizes across Anthropic/OpenAI/Google |
| Different plugin systems | `invocable.source` scopes registry per LLM app |
| ID collisions across apps | Source prefix in IDs: `claude:builtin:Bash` vs `codex:builtin:shell` |
| Mixed queries | Filter by `source`, or omit filter for cross-app analytics |
| Adding a new LLM app | Insert into `sources`, add a parser — schema unchanged |

---

## 9. Incremental Sync Strategy

### 9.1 Sync Algorithm

```
on_sync():
    registry = build_registry_snapshot()  // always fresh from disk
    source = detect_source()              // "claude_code" for now

    for each session_jsonl in discover_session_files(source):
        state = sync_state.get(session_jsonl.path)
        if state.last_offset >= session_jsonl.size:
            continue  // no new data

        seek(session_jsonl, state.last_offset)
        turn_number = last_known_turn(session_jsonl)

        for each line in session_jsonl:
            record = parse_jsonl_line(line)

            // 1. Extract turn-level metrics (every assistant message)
            if record.is_assistant_message():
                model = upsert_model(record.message.model)
                insert_turn(Turn {
                    session_id, turn_number++,
                    model_id: model.id,
                    input_tokens: record.message.usage.input_tokens,
                    output_tokens: record.message.usage.output_tokens,
                    cache_read_tokens: record.message.usage.cache_read_input_tokens,
                    cache_creation_tokens: record.message.usage.cache_creation_input_tokens,
                    stop_reason: record.message.stop_reason,
                    is_sidechain: record.isSidechain,
                    ...
                })

            // 2. Extract turn duration (system messages with durationMs)
            if record.is_system_message() && record.subtype == "turn_duration":
                update_turn_duration(session_id, turn_number, record.durationMs)

            // 3. Extract invocable usage (tool_use blocks)
            tool_uses = extract_tool_uses(record)
            for tool_use in tool_uses:
                current_model = last_model_for_session(session_id)
                match classify(tool_use, registry):
                    Valid(invocation) => insert_invocation(invocation, model=current_model)
                    Rejected(reason) => insert_rejected(tool_use, reason)
                    Ignored => skip

            // 4. Track errors
            if record.has_error():
                increment_session_error_count(session_id)

        // 5. Update session aggregates
        update_session_aggregates(session_id, source)
        update_sync_state(session_jsonl.path, current_offset)

    rebuild_materialized_stats()
```

### 9.2 Idempotency

> **Primary key:** `(source_file, byte_offset)`

This guarantees:
- No double counting
- Safe re-sync after crashes
- Deterministic results

### 9.3 Registry Refresh

The registry snapshot is rebuilt on every sync (not cached across syncs). This ensures newly installed plugins are picked up immediately and removed plugins are marked as `Historical`.

---

## 10. API Contract

All list endpoints accept `?source=claude_code` to filter by LLM app. Omit for cross-app results.

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `GET /api/sources` | GET | List detected LLM apps on this machine |
| `GET /api/models` | GET | List all models observed, with usage counts |
| `GET /api/invocables` | GET | List registered invocables, filterable by kind/status/source |
| `GET /api/invocables/search` | GET | Fuzzy autocomplete for skill discovery |
| `GET /api/stats/overview` | GET | Summary across all invocable types |
| `GET /api/stats/invocables` | GET | Per-invocable usage stats, sortable |
| `GET /api/stats/models` | GET | Per-model usage stats (turns, tokens, sessions) |
| `GET /api/stats/timeline` | GET | Time series (daily/weekly/monthly buckets) |
| `GET /api/stats/sessions` | GET | Session-level breakdown with model + token info |
| `GET /api/stats/tokens` | GET | Token economics (input/output/cache by period) |
| `GET /api/stats/pairings` | GET | Co-occurrence analysis (which tools used together) |
| `GET /api/stats/diagnostics` | GET | Parse errors, rejected invocations, last sync |
| `POST /api/sync` | POST | Trigger incremental sync |

### 10.1 Overview Response

```json
{
  "last_synced": "2026-01-27T18:00:00Z",
  "sync_duration_ms": 142,
  "sources": [
    { "id": "claude_code", "display_name": "Claude Code", "session_count": 156 }
  ],
  "totals": {
    "invocables_registered": 87,
    "invocations_total": 2341,
    "sessions_analyzed": 156,
    "projects_analyzed": 8,
    "models_observed": 3
  },
  "by_kind": {
    "skill": { "registered": 14, "used": 8, "invocations": 234 },
    "command": { "registered": 12, "used": 6, "invocations": 189 },
    "agent": { "registered": 18, "used": 5, "invocations": 42 },
    "mcp_tool": { "registered": 24, "used": 8, "invocations": 156 },
    "builtin_tool": { "registered": 19, "used": 15, "invocations": 1720 }
  },
  "by_model": [
    { "model_id": "claude-opus-4-5-20251101", "provider": "anthropic", "turns": 892, "sessions": 45 },
    { "model_id": "claude-sonnet-4-20250514", "provider": "anthropic", "turns": 1204, "sessions": 98 }
  ],
  "token_summary": {
    "total_input": 4523000,
    "total_output": 1892000,
    "total_cache_read": 3100000,
    "total_cache_creation": 1450000,
    "cache_hit_ratio": 0.68
  },
  "top_10": [
    { "id": "claude:builtin:Bash", "kind": "builtin_tool", "count": 1058 },
    { "id": "claude:builtin:Read", "kind": "builtin_tool", "count": 578 },
    { "id": "claude:superpowers:brainstorming", "kind": "skill", "count": 23 }
  ],
  "diagnostics": {
    "parse_errors": 0,
    "rejected_invocations": 12,
    "files_synced": 14
  }
}
```

### 10.2 Per-Invocable Stats Response

```json
{
  "invocables": [
    {
      "id": "claude:superpowers:brainstorming",
      "source": "claude_code",
      "plugin_name": "superpowers",
      "name": "brainstorming",
      "kind": "skill",
      "status": "enabled",
      "description": "Explores user intent before implementation",
      "total_count": 23,
      "session_count": 18,
      "project_count": 4,
      "first_used": "2026-01-10T14:00:00Z",
      "last_used": "2026-01-27T17:30:00Z",
      "by_model": {
        "claude-opus-4-5-20251101": 15,
        "claude-sonnet-4-20250514": 8
      }
    }
  ],
  "filters": {
    "kind": "skill",
    "source": "claude_code",
    "sort": "total_count",
    "order": "desc"
  }
}
```

### 10.3 Model Stats Response

```json
{
  "models": [
    {
      "model_id": "claude-opus-4-5-20251101",
      "provider": "anthropic",
      "display_name": "Claude Opus 4.5",
      "total_turns": 892,
      "total_sessions": 45,
      "total_input_tokens": 2100000,
      "total_output_tokens": 890000,
      "avg_tokens_per_turn": { "input": 2354, "output": 998 },
      "cache_hit_ratio": 0.72,
      "error_rate": 0.02,
      "first_seen": "2026-01-05T10:00:00Z",
      "last_seen": "2026-01-27T17:30:00Z"
    }
  ]
}
```

### 10.4 Sync Response

```json
{
  "status": "completed",
  "duration_ms": 142,
  "sources_synced": ["claude_code"],
  "files_scanned": 14,
  "new_invocations": 23,
  "new_turns": 48,
  "new_models_discovered": 0,
  "rejected": 2,
  "parse_errors": 0,
  "registry_size": 87
}
```

---

## 11. Autocomplete (UX-Only Feature)

### 11.1 Purpose

- Help users discover installed skills
- Speed up skill lookup
- **Must never affect counting logic**

### 11.2 Matching Rules

| Input | Matches |
|-------|---------|
| `bra` | `superpowers:brainstorming` |
| `storm` | `superpowers:brainstorming` |
| `super:` | All skills/commands under `superpowers` |
| `commit` | `commit-commands:commit`, `commit-commands:commit-push-pr` |

### 11.3 Ranking

1. Exact prefix on `plugin:skill`
2. Prefix on skill name only
3. Word boundary match
4. Substring match

Tie-breakers: higher usage count, then more recent usage.

### 11.4 Implementation

- Query length `< 3`: prefix scan on `invocables` table
- Query length `>= 3`: trigram matching
- Debounce 80–120ms on frontend
- Target: < 50ms response time

---

## 12. Performance Targets

| Operation | Target |
|-----------|--------|
| Cold import (first sync) | >= 30 MB/s throughput |
| Incremental sync | < 200ms typical |
| Aggregation queries | < 50ms |
| Autocomplete | < 50ms |
| Registry construction | < 100ms |
| Dashboard full load | < 200ms |

---

## 13. Mapping to v2 Development Phases

This PRD spans **v2 Phases 2 and 3** (see `2026-01-27-claude-view-v2-design.md` Section 9):

### v2 Phase 2: Data Pipeline (this PRD, core)

- [ ] Source detection (`sources` table, Claude Code first)
- [ ] Parse `installed_plugins.json` and build registry snapshot
- [ ] Scan `skills/`, `commands/`, `.claude/skills/`, `agents/` directories
- [ ] Extract MCP server definitions from `plugin.json`
- [ ] Session JSONL parser: detect Skill(), Task(), mcp__*, built-in tool calls
- [ ] **Turn extraction**: per-turn model_id, tokens, duration, errors, thinking level
- [ ] **Model tracking**: `models` table, upsert on discovery, per-turn FK
- [ ] Registry validation pipeline (classify → validate → insert or reject)
- [ ] Incremental sync with byte-offset tracking
- [ ] Database schema (sources, models, sessions, turns, invocables, invocations, rejected, sync_state)
- [ ] Tags CRUD
- [ ] Data pipeline API (`/api/sources`, `/api/models`, `/api/invocables`, `/api/sync`, `/api/stats/diagnostics`)

### v2 Phase 3: Metrics & Analytics (this PRD + analytics-design.md)

- [ ] Session health metrics (turns, duration, circle-back rate)
- [ ] Session-level aggregates (total tokens, primary model, model switches, error count)
- [ ] Materialized stats rebuild after sync (`invocable_stats` with `by_model`)
- [ ] Token economics API (`/api/stats/tokens`, cache hit ratio)
- [ ] Model stats API (`/api/stats/models`, per-model turns/tokens/sessions)
- [ ] Git commit correlation
- [ ] Stats API (`/api/stats/overview`, `/api/stats/invocables`, `/api/stats/timeline`)
- [ ] Autocomplete endpoint with fuzzy matching (`/api/invocables/search`)
- [ ] Dashboard: usage overview with kind breakdown + model breakdown
- [ ] Dashboard: per-invocable detail view
- [ ] Dashboard: timeline charts
- [ ] Dashboard: token economics panel

### Post-MVP: Advanced Analytics

- [ ] Co-occurrence / pairing analysis
- [ ] Skill effectiveness correlation with session health
- [ ] Trend detection
- [ ] CLI `claude-view stats` integration
- [ ] Multi-AI support: Codex CLI parser + registry
- [ ] Multi-AI support: Gemini CLI parser + registry

---

## 14. Success Criteria

### 14.1 Correctness

- **0 false positives**: No built-in commands, file paths, or typos counted as skills
- **100% of real skill invocations counted**: Every Skill() call that resolves in registry is captured
- **No built-in commands counted as skills**: `bash`, `edit`, `read`, etc. filtered out
- **Model tracking accurate**: Every assistant turn has correct model_id
- **Token counts match API**: Sum of per-turn tokens matches session aggregates

### 14.2 Stability

- Historical stats unchanged by plugin removal (status → Historical)
- Deterministic re-sync (same input → same output)
- Idempotent incremental parsing
- Adding a new LLM app source requires zero schema changes

### 14.3 Performance

- Dashboard loads < 200ms for 1000+ sessions
- Sync completes < 200ms for typical incremental update
- Registry construction < 100ms for 20+ plugins
- Model stats query < 50ms

---

## 15. Relationship to Existing Design Documents

| Document | Relationship |
|----------|-------------|
| `2026-01-27-claude-view-v2-design.md` | This PRD implements v2 Phase 2 (Data Pipeline) and Phase 3 (Metrics & Analytics) |
| `2026-01-27-claude-view-analytics-design.md` | Phase 3 metrics (session health, git correlation) are defined there; this PRD provides the validated data foundation |

The `skills_used` JSON field in the existing `sessions` table (from v2 design) will be populated by this system's validated invocation data rather than regex extraction.

---

## Appendix A: Real Data Observations

Findings from examining actual `~/.claude/` filesystem and session JSONL files:

1. **15 of 22 plugins have no `skills/` directory** — they provide commands, agents, or MCP servers instead
2. **`Skill()` tool calls sometimes contain built-in tool names** (e.g., `"bash"`, `"edit"`) — must be filtered
3. **Some plugins use `.claude/skills/` instead of top-level `skills/`** (e.g., `ui-ux-pro-max`)
4. **Commands and skills can have overlapping names** within the same plugin (e.g., `superpowers` has both `commands/brainstorm.md` and `skills/brainstorming/SKILL.md`)
5. **Bare skill names appear alongside qualified names** — the registry must support both `"commit"` and `"commit-commands:commit"` lookups
6. **MCP tool names encode the marketplace and plugin** in the format `mcp__plugin_{marketplace}_{plugin}__{tool}`
7. **Agent subagent_type uses `plugin:agent` format** matching the skill qualified name convention

## Appendix B: JSONL Fields Inventory

Complete catalog of data available in Claude Code session JSONL files:

| Category | Fields | Analytics Use |
|----------|--------|---------------|
| **Identity** | `uuid`, `sessionId`, `parentUuid` | Message graph, turn threading |
| **Timestamps** | `timestamp` (ISO 8601) | Time series, duration calculation |
| **Model** | `message.model` | Per-turn model tracking, model comparison |
| **Tokens** | `usage.input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` | Cost, cache efficiency |
| **Cache Tiers** | `cache_creation.ephemeral_5m_input_tokens`, `ephemeral_1h_input_tokens` | Cache tier analysis |
| **Duration** | `durationMs` (system message subtype `turn_duration`) | Turn latency |
| **Errors** | `error`, `isApiErrorMessage` | Error rate, failure patterns |
| **Stop** | `message.stop_reason`, `message.stop_sequence` | Truncation detection |
| **Thinking** | `thinkingMetadata.level`, `.maxThinkingTokens` | Extended thinking usage |
| **Sidechain** | `isSidechain`, `agentId` | Subagent work tracking |
| **Context** | `cwd`, `gitBranch`, `version` | Project, branch, app version |
| **Tool Use** | `content[].type == "tool_use"`, `.name`, `.input` | Invocable detection |
| **Hooks** | `hookCount`, `hookInfos`, `hookErrors` | Plugin overhead |
| **Queue** | `operation` (`enqueue`/`dequeue`) | Queue usage patterns |

## Appendix C: Multi-AI Compatibility

Research findings on Codex CLI and Gemini CLI session formats:

| Feature | Claude Code | Codex CLI | Gemini CLI |
|---------|------------|-----------|------------|
| **Storage** | `~/.claude/projects/` | `~/.codex/sessions/` | `~/.gemini/tmp/` |
| **Format** | JSONL | JSONL | JSON (migrating to JSONL) |
| **Naming** | UUID-based | Date-tree + rollout prefix | Project-hash based |
| **Skills** | `SKILL.md` + plugins | `SKILL.md` + skills catalog | Extensions + `gemini-extension.json` |
| **Config** | `~/.claude/settings.json` | `~/.codex/config.toml` | `~/.gemini/settings.json` |
| **MCP** | Native | Native (can also serve as MCP) | Via extensions |
| **Instruction files** | `CLAUDE.md` | `AGENTS.md` | `GEMINI.md` |

All three store the same fundamental data (messages, tool calls, timestamps, model info) in different schemas. The `SessionParser` trait + `Source` enum design supports adding parsers without schema changes.

## Appendix D: Candidate Regex (Extractor Only)

For any future history.jsonl display-field extraction (e.g., autocomplete suggestions):

```
^/([a-zA-Z0-9_-]+):([a-zA-Z0-9_-]+)(?:\s+.*)?$
```

> This regex is NEVER used for counting. Validation always happens against the registry snapshot.

---

*2026-01-27*
