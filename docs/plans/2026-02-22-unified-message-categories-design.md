# Unified Message Categories Design

**Date**: 2026-02-22
**Branch**: fix/token-deduplication
**Status**: Approved

## Problem

Three related bugs in message categorization:

1. **Log tab shows 201 hooks but badge filter shows 0 hooks** — Hook events in the Log tab come from WebSocket `hook_event` objects. Badge filter counts from JSONL-parsed messages. The 144 `hook_progress` entries in JSONL are parsed as `role: progress` with no `category` field, so they're invisible to badge filters.

2. **Progress/system messages have no badge filters** — `progress` (agent_progress, bash_progress, hook_progress), `system` (turn_duration, stop_hook_summary), `file-history-snapshot`, and `queue-operation` messages are "structural" types with no category. They appear in verbose mode but can't be filtered.

3. **ConversationView lacks badge filters** — The history detail page's verbose mode shows all message types but has no `ActionFilterChips` bar for filtering by category.

### Root Cause

Categorization was designed only for tool calls (`categorizeTool()` in frontend). The JSONL has rich structural metadata (`data.type`, `subtype`) that should drive categorization at the data layer. Two separate Rust parsers (REST API `parser.rs` and WebSocket `terminal.rs`) each need category assignment.

## Design

### Expanded ActionCategory

```
'skill' | 'mcp' | 'builtin' | 'agent' | 'hook' | 'error' | 'system' | 'snapshot' | 'queue'
```

New categories:
- `system` — turn_duration, stop_hook_summary, api_error
- `snapshot` — file-history-snapshot entries
- `queue` — queue-operation entries

### Category Mapping

| JSONL type | Subtype/metadata | Category |
|---|---|---|
| `tool_use` (name: Bash, Read, Write, etc.) | — | `builtin` |
| `tool_use` (name: mcp__*) | — | `mcp` |
| `tool_use` (name: Task) | — | `agent` |
| `tool_use` (name: Skill) | — | `skill` |
| `tool_result` | — | Inherits from preceding tool_use |
| `progress` | `data.type: hook_progress` | `hook` |
| `progress` | `data.type: agent_progress` | `agent` |
| `progress` | `data.type: bash_progress` | `builtin` |
| `progress` | `data.type: mcp_progress` | `mcp` |
| `progress` | `data.type: waiting_for_task` | `agent` |
| `system` | any subtype | `system` |
| `file-history-snapshot` | — | `snapshot` |
| `queue-operation` | — | `queue` |
| `user` | — | None |
| `assistant` | — | None |
| `summary` | — | None |

### Architecture: Single Source of Truth in Rust

**Shared categorization function** in `crates/core/`:

```rust
pub fn categorize_message(msg_type: &str, metadata: &serde_json::Value) -> Option<String>
```

Both parsers call this:
1. **REST API parser** (`crates/core/src/parser.rs`) — sets `Message.category` for paginated/historical data
2. **WebSocket parser** (`crates/server/src/routes/terminal.rs` `format_line_for_mode()`) — includes `category` in the structured JSON sent to frontend

**Why Rust, not frontend**: Follows the Elasticsearch/Kibana pattern — categorize at ingestion time, filter at query time. Prevents duplication across clients. Deterministic based on JSONL data.

### Rust Changes

1. **`crates/core/src/types.rs`**: Add `category: Option<String>` to `Message` struct
2. **`crates/core/src/category.rs`** (new): Shared `categorize_message()` + `categorize_tool()` functions
3. **`crates/core/src/parser.rs`**: Call `categorize_message()` during parsing, set `message.category`
4. **`crates/server/src/routes/terminal.rs`**: Call `categorize_message()` in `format_line_for_mode()`, include `category` in WebSocket JSON

### Frontend Changes

1. **Delete `src/lib/categorize-tool.ts`** — replaced by Rust-provided categories
2. **`src/components/live/action-log/types.ts`**: Expand `ActionCategory` to include `'system' | 'snapshot' | 'queue'`
3. **`src/lib/message-to-rich.ts`**: Read `category` from `Message.category` instead of calling `categorizeTool()`
4. **`src/hooks/use-live-session-messages.ts`**: Read `category` from WebSocket message instead of calling `categorizeTool()`
5. **`src/components/live/RichPane.tsx`**: Category counts include new categories; verbose filter logic handles new types
6. **`src/components/ConversationView.tsx`**: Verbose mode renders `ActionFilterChips` bar (same component as live terminal)
7. **`src/components/live/action-log/ActionFilterChips.tsx`**: Render labels/icons for new categories
8. **`src/types/generated/`**: Regenerate TS types from Rust struct changes

### ConversationView Parity

When `verboseMode` is active in ConversationView:
- `ActionFilterChips` bar appears at top (same component as live terminal RichPane)
- Category counts computed from paginated `RichMessage[]` array
- Clicking badge filters message list to that category
- Same UX in both live and history views

## What This Fixes

| Before | After |
|---|---|
| Log tab: 201 hooks, badge: 0 hooks | Badge correctly shows hook count from JSONL data |
| progress/system invisible to badges | All non-chat messages have categories + badge filters |
| ConversationView verbose has no badges | Same badge filter bar as live terminal |
| Two categorization systems (Rust + TS) | Single source of truth in Rust parser |
| WebSocket live doesn't categorize progress | Both REST + WebSocket paths emit categories |

## Validation

- Check session `c3393d11-763d-4636-bf86-0c18158864c2` which has 144 hook_progress, 65 agent_progress, 7 bash_progress entries
- Badge filter should show non-zero counts for hook, agent, builtin categories
- ConversationView verbose mode should have functioning badge filter bar
- Live terminal should show same categories as history view for the same session
