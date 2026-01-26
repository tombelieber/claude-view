# Claude View v2 - Design Specification

> Local personal intelligence system for Claude Code chat history

## 1. Problem Statement

Claude Code generates conversation history in `~/.claude/projects/` as JSONL files. Current claude-view provides basic browsing, but lacks:

1. **Fast search** across large corpus (10GB+)
2. **Skill intelligence** - track which skills/commands are used, when, how often
3. **In-session search** - find specific messages within large sessions
4. **Manual tagging** - organize sessions for later retrieval
5. **Lightweight runtime** - current Node.js backend has overhead

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

## 3. Tech Stack

```
React SPA ──HTTP──► Rust Backend (single binary)
                        │
                        ├──► Tantivy (embedded full-text search)
                        │
                        └──► SQLite (tags, skills, indexer state)
```

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Backend | **Rust** | ~30MB RAM, fast cold start, single binary |
| Search | **Tantivy** | Embedded, no external service, ~15MB |
| Metadata | **SQLite** | Tags persist across reindex, skill stats |
| Distribution | **brew / cargo** | Single command install |
| Frontend | **Keep React** | 80% UI exists, swap API layer |

## 4. Architecture

```
~/.claude/projects/**/*.jsonl
            │
            ▼
   ┌─────────────────┐
   │  Rust Indexer   │  (background, incremental)
   └────────┬────────┘
            │
     ┌──────┴──────┐
     ▼             ▼
┌─────────┐  ┌──────────┐
│ Tantivy │  │  SQLite  │
│ (search)│  │ (meta)   │
└────┬────┘  └────┬─────┘
     └──────┬─────┘
            ▼
   ┌─────────────────┐
   │  Rust REST API  │  (Axum)
   └────────┬────────┘
            ▼
   ┌─────────────────┐
   │   React SPA     │
   └─────────────────┘
```

### Index Unit: Turn-Based

Each searchable document = one conversation turn (human + assistant + tool calls). Natural boundary, good precision, skills align with turns.

```
Turn {
  id, session_id, project, turn_number, timestamp,
  human_text, assistant_text, tool_calls, skills, full_text
}
```

### Skill Extraction

Two patterns in JSONL:

```xml
<!-- User slash command -->
<command-name>/superpowers:brainstorm</command-name>
```

```json
// Assistant Skill tool use
{ "type": "tool_use", "name": "Skill", "input": { "skill": "..." } }
```

### Data Storage

| Store | Location | Contains |
|-------|----------|----------|
| Tantivy | `~/.claude-view/index/` | All turns (incremental) |
| SQLite | `~/.claude-view/claude-view.db` | tags, skills, indexer state |

## 5. Search Design

**Global Search:** Find sessions by keyword → returns grouped results by session_id

**In-Session Search:** Find turns within session → returns turn numbers, jump to match

Both use same Tantivy index with different query filters.

**Skill Autocomplete:** Type `/` → query SQLite for skills sorted by usage (30d)

## 6. API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /api/search?q=...&project=...&skills=...` | Global search |
| `GET /api/search?q=...&session_id=...` | In-session search |
| `GET /api/sessions/:id` | Get session (lazy load) |
| `GET /api/projects` | List projects |
| `GET /api/skills/suggest?q=...` | Skill autocomplete |
| `GET /api/skills/:name/usages` | Skill usage list |
| `GET /api/stats` | Dashboard stats |
| `POST /api/sessions/:id/tags` | Manage tags |

## 7. Frontend Changes

**Stays:** Layout, routing, ConversationView, SessionCard, StatsDashboard, CommandPalette structure.

**Changes:**

| File | Change |
|------|--------|
| `use-projects.ts`, `use-session.ts` | Fetch from Rust API |
| `search.ts` | Remove client-side, call API |
| `CommandPalette.tsx` | Wire to `/api/skills/suggest` |
| `SearchResults.tsx` | Wire to `/api/search` |

**New:** `TagEditor.tsx`, `SkillsCatalog.tsx` (optional)

## 8. Distribution

```bash
brew install claude-view   # macOS
cargo install claude-view  # Rust users
claude-view                # → builds index, opens browser
```

Single ~15MB binary, no dependencies, data in `~/.claude-view/`.

## 9. Success Criteria

- [ ] Index builds for 10GB corpus
- [ ] Search latency < 100ms
- [ ] Global search returns grouped results
- [ ] In-session search jumps to turn
- [ ] Skill autocomplete shows usage counts
- [ ] Dashboard shows top skills
- [ ] Tags persist after restart
- [ ] `brew install && claude-view` works

## 10. Post-MVP Roadmap

| Phase | Features |
|-------|----------|
| v2.1 | LLM batch tagging |
| v2.2 | "Resume where I left off" |
| v2.3 | Vector search toggle |
| v3.0 | Tauri desktop app |
| v3.1 | Multi-AI support |

---

*2026-01-27*
