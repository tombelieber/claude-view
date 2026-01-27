---
status: approved
date: 2026-01-27
---

# vibe-recall v2 - Design Specification

> Local web server for browsing and searching Claude Code chat history

**Architecture:** Localhost web server (Rust backend, React frontend). Runs as single binary, serves SPA, opens in browser.

---

## Finalized Decisions (Do Not Revisit)

| Topic | Decision | Alternatives Rejected |
|-------|----------|----------------------|
| **Project name** | vibe-recall | claude-view (generic) |
| **Distribution** | `npx` + `brew` | cargo install (users need Rust) |
| **Binary hosting** | Cloudflare R2 | GitHub Releases (rate limits) |
| **Runtime** | Localhost web server | Tauri desktop (v3.0 maybe) |
| **Docker** | Skip for MVP | Complicates `~/.claude/` access |
| **Port** | `47892` default + ENV override | 3000 (collision prone) |
| **Platforms (MVP)** | macOS only | Linux/Windows (v2.1+) |
| **Crate structure** | 4 crates (Lapce pattern) | Single crate, 9+ crates |
| **Role models** | Lapce (structure), vibe-kanban (distribution) | — |

---

## 1. Problem Statement

Claude Code generates conversation history in `~/.claude/projects/` as JSONL files. Current implementation provides basic browsing, but lacks:

1. **Fast search** across large corpus (10GB+)
2. **Skill intelligence** - track which skills/commands are used, when, how often
3. **In-session search** - find specific messages within large sessions
4. **Manual tagging** - organize sessions for later retrieval
5. **Lightweight runtime** - current Node.js backend has overhead

---

## 2. MVP Scope

### In Scope

| Feature | Description |
|---------|-------------|
| **Global search** | Find sessions by keyword, filter by project/date/skills |
| **In-session search** | Find specific turns within a loaded session |
| **Skill autocomplete** | Type `/` to see skills sorted by usage frequency |
| **Skill stats** | Dashboard showing top skills, usage trends |
| **Manual tags** | Add/remove tags on sessions, filter by tags |
| **Jump-to-context** | Search result links directly to matching turn |

### Out of Scope (Post-MVP)

- Semantic/vector search
- LLM-powered auto-tagging
- MCP integration
- Desktop packaging (Tauri)
- Multi-AI support (Codex, Gemini CLI)

---

## 3. Tech Stack

```
React SPA ──HTTP──► Rust Backend (single binary)
                        │
                        ├──► Tantivy (embedded full-text search)
                        │
                        └──► SQLite (tags, skills, indexer state)
```

### Dependencies (Latest Stable as of Jan 2026)

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.49", features = ["full"] }

# Web framework
axum = { version = "0.8.6", features = ["macros"] }
tower-http = { version = "0.6.6", features = ["cors", "fs", "trace"] }

# Database
sqlx = { version = "0.8.6", features = ["runtime-tokio", "sqlite"] }

# Search
tantivy = "0.25"

# File watcher
notify = "8.2"

# Serialization
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"

# Error handling
thiserror = "2.0.12"
anyhow = "1.0.98"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

---

## 4. Architecture

### 4.1 Crate Structure (4 crates, Lapce pattern)

```
vibe-recall/
├── crates/
│   ├── core/           # Shared types, JSONL parser, skill extraction
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs
│   │       ├── parser.rs
│   │       └── skills.rs
│   │
│   ├── db/             # SQLite via sqlx
│   │   ├── migrations/
│   │   └── src/
│   │       ├── lib.rs
│   │       └── models/
│   │           ├── session.rs
│   │           ├── tag.rs
│   │           └── skill.rs
│   │
│   ├── search/         # Tantivy full-text search
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema.rs
│   │       ├── indexer.rs
│   │       └── query.rs
│   │
│   └── server/         # Axum HTTP server
│       └── src/
│           ├── lib.rs
│           ├── main.rs
│           └── routes/
│               ├── mod.rs
│               ├── projects.rs
│               ├── sessions.rs
│               ├── search.rs
│               ├── tags.rs
│               └── skills.rs
│
├── frontend/           # React SPA
│   ├── src/
│   ├── index.html
│   └── package.json
│
├── npx-cli/            # Thin JS wrapper for distribution
│   ├── package.json
│   ├── bin/cli.js
│   └── lib/
│       ├── platform.js
│       ├── download.js
│       └── cache.js
│
├── Cargo.toml          # Workspace manifest
└── pnpm-workspace.yaml
```

### 4.2 Crate Dependencies

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  server  │────►│  search  │────►│   core   │
│  (Axum)  │     │ (Tantivy)│     │ (types)  │
└────┬─────┘     └──────────┘     └────▲─────┘
     │                                  │
     │           ┌──────────┐          │
     └──────────►│    db    │──────────┘
                 │ (SQLite) │
                 └──────────┘
```

### 4.3 Data Flow

**Indexing:**
```
~/.claude/projects/**/*.jsonl
         │
         ▼
┌─────────────────┐
│  File Watcher   │  (notify crate, incremental)
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│  JSONL Parser   │────►│    Tantivy      │
│  (core)         │     │    (search)     │
└────────┬────────┘     │                 │
         │              │  Index:         │
         │              │  - full_text    │
         │              │  - session_id   │
         │              │  - skills[]     │
         │              │  - timestamp    │
         ▼              └─────────────────┘
┌─────────────────┐
│     SQLite      │
│     (db)        │
│                 │
│  Store:         │
│  - session meta │
│  - skill stats  │
│  - user tags    │
│  - index state  │
└─────────────────┘
```

**Querying:**
```
Browser → GET /api/search?q=...
              │
              ▼
         ┌─────────┐
         │  Axum   │
         └────┬────┘
              │
    ┌─────────┴─────────┐
    ▼                   ▼
┌─────────┐       ┌─────────┐
│ Tantivy │       │ SQLite  │
│ (match) │       │ (enrich)│
└────┬────┘       └────┬────┘
     └────────┬────────┘
              ▼
        Merge & Rank
              │
              ▼
        JSON Response
```

---

## 5. API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/projects` | GET | List all projects |
| `/api/projects/:id/sessions` | GET | Sessions in project |
| `/api/sessions/:id` | GET | Full session (lazy load) |
| `/api/search` | GET | Global search |
| `/api/search/session/:id` | GET | In-session search |
| `/api/sessions/:id/tags` | POST | Add tag |
| `/api/sessions/:id/tags/:tag` | DELETE | Remove tag |
| `/api/skills` | GET | Skill catalog |
| `/api/skills/:name/sessions` | GET | Sessions using skill |
| `/api/stats` | GET | Dashboard stats |
| `/api/index/status` | GET | Indexer health |

### Search Query Parameters

```
GET /api/search?q=react+hooks&project=vibe-recall&skills=commit&after=2026-01-01
```

| Param | Type | Description |
|-------|------|-------------|
| `q` | string | Full-text query |
| `project` | string | Filter by project |
| `skills` | string | Comma-separated skill filter |
| `tags` | string | Comma-separated tag filter |
| `after` | date | Sessions after date |
| `before` | date | Sessions before date |
| `limit` | int | Results per page (default 20) |
| `offset` | int | Pagination offset |

---

## 6. Database Schema (SQLite)

```sql
-- Session metadata (denormalized for fast list queries)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT,
    preview TEXT,
    turn_count INTEGER DEFAULT 0,
    file_count INTEGER DEFAULT 0,
    first_message_at INTEGER,
    last_message_at INTEGER,
    file_path TEXT NOT NULL UNIQUE,
    file_hash TEXT,
    indexed_at INTEGER
);

CREATE INDEX idx_sessions_project ON sessions(project_id);
CREATE INDEX idx_sessions_last_message ON sessions(last_message_at DESC);

-- User tags
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at INTEGER DEFAULT (unixepoch()),
    UNIQUE(session_id, name)
);

CREATE INDEX idx_tags_name ON tags(name);

-- Skill usage stats
CREATE TABLE skills (
    name TEXT PRIMARY KEY,
    usage_count INTEGER DEFAULT 0,
    last_used_at INTEGER,
    first_used_at INTEGER
);

-- Skill usage per session
CREATE TABLE session_skills (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    skill_name TEXT NOT NULL,
    usage_count INTEGER DEFAULT 1,
    PRIMARY KEY (session_id, skill_name)
);

-- Indexer state
CREATE TABLE indexer_state (
    key TEXT PRIMARY KEY,
    value TEXT
);
```

---

## 7. Search Schema (Tantivy)

```rust
pub fn build_schema() -> Schema {
    let mut builder = Schema::builder();

    // Identifiers
    builder.add_text_field("turn_id", STRING | STORED);
    builder.add_text_field("session_id", STRING | STORED | FAST);
    builder.add_text_field("project", STRING | STORED | FAST);

    // Searchable content
    builder.add_text_field("human_text", TEXT | STORED);
    builder.add_text_field("assistant_text", TEXT | STORED);
    builder.add_text_field("tool_calls", TEXT);
    builder.add_text_field("full_text", TEXT);

    // Facets and filters
    builder.add_facet_field("skills", INDEXED | STORED);
    builder.add_date_field("timestamp", INDEXED | STORED | FAST);
    builder.add_u64_field("turn_number", INDEXED | STORED | FAST);

    builder.build()
}
```

---

## 8. Distribution (npx + R2)

### npx-cli structure

```
npx-cli/
├── package.json
├── bin/cli.js
└── lib/
    ├── platform.js   # Detect OS/arch
    ├── download.js   # Fetch from R2
    └── cache.js      # Manage cached binaries
```

### Platform mapping

| Platform | Arch | Binary |
|----------|------|--------|
| darwin | arm64 | `vibe-recall-macos-arm64.zip` |
| darwin | x64 | `vibe-recall-macos-x64.zip` |
| linux | x64 | `vibe-recall-linux-x64.zip` (v2.1) |
| win32 | x64 | `vibe-recall-windows-x64.zip` (v2.2) |

### Cache location

```
~/.cache/vibe-recall/{version}/{platform}/vibe-recall
```

### R2 bucket structure

```
vibe-recall-releases/
├── 0.1.0/
│   ├── vibe-recall-macos-arm64.zip
│   └── vibe-recall-macos-x64.zip
├── 0.2.0/
│   └── ...
└── latest.json  # {"version": "0.1.0"}
```

---

## 9. Development Sequence

### Phase 1: Foundation — COMPLETE (~85%)
| Task | Crate | Status |
|------|-------|--------|
| Scaffold workspace | root | Done — Cargo.toml, 4 crate stubs |
| Define types | core | Done — types.rs, SessionInfo, ProjectInfo |
| JSONL parser | core | Done — parser.rs, battle-tested with fixtures |
| Project discovery | core | Done — discovery.rs, DFS path resolution |
| SQLite setup | db | Done — 4 migrations, WAL mode, in-memory test support |
| Indexer pipeline | db | Done — scan → diff → index with progress callbacks |
| Basic Axum server | server | Done — /api/projects, /api/health, /api/session |
| Frontend served | server | Done — static file serving + Vite dev proxy |

**Milestone: Rust backend replaces Express** ✓

**Remaining:** Fix compilation error in discovery.rs (missing 4 new fields on SessionInfo)

### Phase 2: Parallel Indexing + Invocable Registry — IN PROGRESS

> **Design doc:** [`2026-01-27-phase2-parallel-indexing-and-registry.md`](2026-01-27-phase2-parallel-indexing-and-registry.md)

**Chunk A: Instant Startup + Registry + Invocations** (17 steps)

| Task | Crate | Deliverable |
|------|-------|-------------|
| Pass 1: read `sessions-index.json` | core, db | Instant startup (<10ms), summary/branch/sidechain fields |
| Pass 2: parallel mmap JSONL parsing | db | Background deep indexing with SIMD line scanning |
| Plugin registry from `installed_plugins.json` | core | Registry struct with qualified + bare name lookup |
| Invocation extraction + classification | core, db | `classify_tool_use()` against registry, zero false positives |
| `invocables` + `invocations` tables | db | Schema migration, CRUD operations |
| Server-first startup | server | Server ready before indexing, background spawn |
| SSE progress endpoint | server | `GET /api/indexing/progress` |
| New API endpoints | server | `GET /api/invocables`, `GET /api/stats/overview` |
| TUI progress display | server | Terminal startup UX with indicatif |

**Milestone: Instant startup, skill/tool usage visibility**

**Deferred to Chunk B** (token & model tracking):

| Item | Design source |
|------|---------------|
| `models` table — normalize model IDs | PRD §7-8 |
| `turns` table — per-turn tokens, duration, model | PRD §8.1 |
| Session token aggregates — total input/output/cache tokens | PRD §8.1 |
| `primary_model` on sessions | PRD §8.1 |
| `/api/stats/models`, `/api/stats/tokens` | PRD §10 |

**Deferred to Chunk C** (session health):

| Item | Design source |
|------|---------------|
| Circle-back detection | Analytics design §5 |
| Smooth/Turbulent classification | Analytics design §7 |
| Git commit correlation | Analytics design §6 |
| `daily_stats` table | Analytics design §8 |
| `/api/stats/daily`, `/api/sessions/:id/commits` | Analytics design §9 |
| Rule-based insights generation | Analytics design §12 |

**Deferred infrastructure:**

| Item | Design source |
|------|---------------|
| `sources` table + multi-AI support | PRD §7.1, §8.4 |
| `rejected_invocations` table | PRD §8.1 — log to tracing instead |
| `invocable_stats` materialized view | PRD §8.3 — add when join query is slow |

### Phase 3: Metrics & Analytics
| Task | Crate | Deliverable |
|------|-------|-------------|
| Chunk B: token & model tracking | core, db, server | Per-session cost visibility |
| Chunk C: session health + git correlation | core, db, server | Smooth/turbulent, insights |
| Materialized stats | db | `invocable_stats` if needed for perf |
| Stats API | server | `/api/stats/overview`, `/api/stats/invocables`, `/api/stats/timeline` |
| Autocomplete | server, frontend | Fuzzy skill search |
| Dashboard frontend | frontend | Usage overview, top skills, health breakdown |

> **Design docs:** [`2026-01-27-skills-usage-analytics-prd.md`](2026-01-27-skills-usage-analytics-prd.md), [`2026-01-27-vibe-recall-analytics-design.md`](2026-01-27-vibe-recall-analytics-design.md)

**Milestone: Dashboard shows real analytics with validated data**

### Phase 4: Search
| Task | Crate | Deliverable |
|------|-------|-------------|
| Tantivy schema | search | Index builds over structured data from Phase 2+3 |
| Initial indexer | search | Index all sessions with metadata facets |
| Search API | server | `/api/search` with skill/tag/project/date filters |
| File watcher | search | `notify` crate, incremental re-index |
| In-session search | server | `/api/search/session/:id` jumps to turn |
| Tags CRUD | db, server | Add/remove tags on sessions |

> Search is a **query layer** over data collected in Phase 2+3. Tantivy indexes structured records (sessions with invocables, health, tags), not raw text.

**Milestone: Full-text search works with rich filtering**

### Phase 5: Distribution
| Task | Location | Deliverable |
|------|----------|-------------|
| Build binary | CI | vibe-recall-macos-arm64 |
| npx wrapper | npx-cli | Downloads + runs binary |
| R2 upload | CI | Binaries on R2 |
| Test npx | local | `npx vibe-recall` works |

**Milestone: Users can install via npx**

---

## 10. Success Criteria

- [x] Rust backend replaces Express (Phase 1)
- [x] `/api/projects` returns data from SQLite
- [x] Incremental indexing detects new/modified files
- [ ] Registry discovers all installed skills, commands, agents, MCP tools
- [ ] Zero false positives in skill/invocable counting
- [ ] Server starts in <500ms (before indexing completes)
- [ ] Pass 1 completes in <10ms for 10 projects
- [ ] Index builds for 10GB corpus
- [ ] Search latency < 100ms
- [ ] Global search returns grouped results with facets
- [ ] In-session search jumps to turn
- [ ] Skill autocomplete shows usage counts
- [ ] Dashboard shows top skills with usage trends
- [ ] Tags persist after restart
- [ ] `npx vibe-recall` downloads binary and opens browser

---

## 11. Post-MVP Roadmap

| Phase | Features |
|-------|----------|
| v2.1 | Linux support, LLM batch tagging |
| v2.2 | Windows support, "Resume where I left off" |
| v2.3 | Vector search toggle |
| v3.0 | Tauri desktop app |
| v3.1 | Multi-AI support (Codex, Gemini CLI) |

---

*2026-01-27*
