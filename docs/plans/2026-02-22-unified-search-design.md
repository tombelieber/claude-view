# Unified Search: Tantivy-Powered Full-Text Search Everywhere

**Date:** 2026-02-22
**Status:** Design approved, pending implementation plan
**Test word:** "brainstorming" — 987 JSONL files contain it, history shows 16, cmd+k shows 0.

## Problem

Two search surfaces exist with completely different implementations and results:

| Surface | Engine | Searches | "brainstorming" results |
|---------|--------|----------|------------------------|
| History page search bar | SQLite `LIKE '%q%'` | `preview`, `last_message`, `project_display_name` | 16 |
| Cmd+k (CommandPalette) | Tantivy full-text | `content` field (per-message text only) | 0 |
| Raw JSONL files | — | Everything | **987** |

**Root causes:**
1. Tantivy only indexes user text + assistant first text block. Misses tool_use inputs, tool_result content, system messages containing search terms.
2. Session-level metadata (preview, project_display_name) is never indexed in Tantivy.
3. History page search uses SQLite LIKE which only searches 3 metadata columns, not conversation content.
4. No fuzzy/typo-tolerant matching in either path.

## Design

### Principle

One search behavior everywhere. User types a word, gets results. The engine behind the scenes is invisible.

### Architecture

```
User types query in history search bar OR cmd+k
         │
         ▼
    ┌─────────────┐
    │   Tantivy    │  Full-text fuzzy search on ALL indexed content
    │              │  Returns: session_ids + scores + snippets
    └──────┬──────┘
           │
           ▼
    ┌─────────────┐
    │    SQLite    │  Applies structured filters (branch, model, duration, etc.)
    │              │  Sorts, paginates, returns full session metadata
    └──────┬──────┘
           │
           ▼
    Merged response (Tantivy relevance + SQL metadata)
```

- **Text query only** (no filters): Tantivy returns results directly.
- **Filters only** (no text): SQL only, skip Tantivy entirely.
- **Both**: Tantivy narrows by text → SQL filters + paginates the narrowed set.

### Change 1: Enrich Tantivy Index Content

**What to index per message:**

| Content type | Currently indexed | After |
|---|---|---|
| User message text | Yes | Yes |
| Assistant text blocks | First block only | **All text blocks** |
| Tool_use inputs (file paths, code) | Partially (Value path only) | **All tool_use input text** |
| Tool_result content | No | **Yes — tool outputs contain search-relevant text** |
| System messages (CLAUDE.md, reminders) | No | No (noise, not user-searchable) |
| Progress/Summary lines | No | No (noise) |

**What to index per session (new "summary" document):**

Add one document per session with `role: "summary"` containing:
- `preview` (session summary text)
- `project_display_name`
- `last_message`

This makes session metadata searchable via the same Tantivy query without schema changes — it goes in the existing `content` field.

### Change 2: Fuzzy Matching

When the text query is not a quoted phrase, apply fuzzy matching (Levenshtein distance=1) per term.

- `brainstormin` (typo) → matches "brainstorming"
- `"brainstorming session"` (quoted) → exact phrase match, no fuzzy
- `project:claude-view auth` → qualifier is exact, "auth" is fuzzy

Tantivy supports this natively via `FuzzyTermQuery`.

### Change 3: History Page Search → Tantivy

Replace the SQLite LIKE search in `query_sessions_filtered`:

```
-- Before (SQLite LIKE):
WHERE s.preview LIKE '%brainstorming%'
   OR s.last_message LIKE '%brainstorming%'
   OR s.project_display_name LIKE '%brainstorming%'

-- After (Tantivy-powered):
WHERE s.id IN (session_ids_from_tantivy)
  AND <existing structured filters unchanged>
ORDER BY <existing sort unchanged>
```

The `q` parameter in `SessionFilterParams` becomes a pre-resolved set of session_ids from Tantivy, not a LIKE pattern.

### Change 4: Cmd+k Uses Same Path

CommandPalette already calls Tantivy via `/api/search`. After the index enrichment (Changes 1-2), it automatically benefits — no frontend change needed beyond potentially switching to the unified endpoint.

### Change 5: Unified API Surface

Both surfaces call the same search logic:

| Endpoint | Role |
|----------|------|
| `GET /api/sessions?q=brainstorming&branch=main` | History page: text search + structured filters + pagination |
| `GET /api/search?q=brainstorming` | Cmd+k: text search only, returns top 5 with snippets |

`/api/sessions` internally calls Tantivy when `q` is present, feeds session_ids to SQL.
`/api/search` remains as-is (Tantivy-only) for cmd+k which needs turn-level snippets.

Both benefit from the enriched index and fuzzy matching.

### What Stays the Same

- All structured filters (branch, model, duration, tokens, has_commits, etc.) — unchanged SQL
- Sorting by tokens/duration/prompts — unchanged SQL
- Pagination with total count — unchanged SQL
- Tantivy schema fields — no new fields needed (`summary` docs use existing `content` field)
- Schema version bump triggers auto-rebuild — already handled
- `/api/search` endpoint — still returns turn-level results with snippets for cmd+k

## Validation

After implementation, searching "brainstorming" should:
- **Cmd+k**: Return multiple sessions (was: 0)
- **History page**: Return hundreds of sessions (was: 16)
- **Both surfaces**: Return the same sessions for the same query
- **Fuzzy**: `brainstormin` (missing g) still returns results

## Scope Boundaries

**In scope:**
- Enrich Tantivy indexer to capture more content types
- Add session summary document to index
- Add fuzzy matching
- Replace SQLite LIKE with Tantivy in sessions endpoint
- Schema version bump to trigger re-index

**Out of scope:**
- Changing Tantivy schema fields (no new fields, summary uses existing `content`)
- Changing structured filters (branch, model, duration — all unchanged)
- Changing sorting logic (SQL-based, unchanged)
- Indexing system/progress/summary lines (noise)
