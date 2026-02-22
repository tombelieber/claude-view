# Rich Mode Tool Card Redesign

**Date:** 2026-02-22
**Status:** Approved
**Scope:** RichPane verbose mode — tool_use + tool_result rendering

## Problem

In rich/verbose mode, MCP and Skill tool cards are "too lean":
- MCP tools show a name chip + collapsed `JsonKeyValueChips` or raw JSON — no structure
- Skill shows a purple badge + plain text args — minimal
- tool_use and tool_result render as separate messages, forcing the user to visually pair them
- Built-in tools (Edit, Write, Read, Bash, Grep) have dedicated renderers with structured fields — MCP/Skill should match this quality

## Design

### 1. `usePairedMessages()` Hook

**File:** `src/hooks/use-paired-messages.ts`

A pure `useMemo` hook that transforms `RichMessage[]` into `DisplayItem[]`:

```typescript
type DisplayItem =
  | { kind: 'message'; message: RichMessage }
  | { kind: 'tool_pair'; toolUse: RichMessage; toolResult: RichMessage | null }
```

**Pairing logic:** Walk the array linearly. When a `tool_use` is found, look ahead for the next `tool_result` (skipping `thinking` messages only). If found before another `tool_use`/`user`/`assistant`, pair them. Otherwise emit `tool_use` alone with `toolResult: null`.

**Virtuoso impact:** `displayMessages` shrinks (pairs merge two items into one row). `itemContent` dispatches on `kind`. Keys use `toolUse.ts` or stable index.

### 2. Smart MCP Renderer

**File:** `src/components/live/ToolRenderers.tsx` (new `SmartMcpRenderer`)

Handles any `mcp__*` tool by auto-detecting parameter types:

| Pattern | Component |
|---------|-----------|
| Value starts with `http://` or `https://` | `<Link>` icon + clickable URL |
| Value looks like a file path (`/`, `.ext`) | `<FilePathHeader>` |
| Key is `uid`, `selector`, `xpath`, `css` | Highlighted monospace chip (amber, like Grep pattern) |
| Multi-line string (>1 newline) | `<CompactCodeBlock>` with language detection |
| Boolean | Colored chip (green=true, red=false) |
| Number | `<Chip label={key} value={val}>` |
| Object or array | Inline `<JsonTree>` (collapsed by default) |
| Short string (default) | `<Chip label={key} value={val}>` |

**Registry fallback:** For any tool name starting with `mcp__` without a specific registry entry, use `SmartMcpRenderer`. Existing builtin renderers unchanged.

### 3. PairedToolCard Component

**File:** `src/components/live/PairedToolCard.tsx`

Replaces separate `ToolUseMessage` + `ToolResultMessage` with a unified card:

```
┌─ border-left (tool category color) ────────────────────────────┐
│ 🔧  [tool-name]  server-name       { } rich/json    12:30 PM  │
│                                                                 │
│  ┌── Input ──────────────────────────────────────────────────┐ │
│  │  (ToolRenderer output: structured fields, code, chips)    │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌── Result ──────────────────── ✓ success  0.3s ────────────┐ │
│  │  (Auto-detected: JsonTree / CompactCodeBlock / Markdown)  │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Behavior:**
- **Header:** Tool icon + name chip (colored by category via `toolChipColor`) + server name (muted) + `{ }` rich/json toggle + timestamp
- **Input:** Always visible in verbose mode. Uses `ToolRenderer` from registry (or SmartMcpRenderer for MCP). No more `JsonKeyValueChips` collapse for tool cards.
- **Result:** Status badge (success ✓ / error ✗) + duration (computed from `toolUse.ts` and `toolResult.ts`). Content via smart detection:
  - JSON → `JsonTree` (rich mode) or `CompactCodeBlock` (json mode)
  - Diff-like → `CompactCodeBlock` with `language="diff"`
  - Code-like → `CompactCodeBlock` with detected language
  - Text → `Markdown` with `markdownComponents`
- **Pending state:** If `toolResult` is null, show subtle "pending..." indicator
- **Result collapsible:** Defaults to visible in verbose mode, collapsible via `[ Collapse ]` button

### 4. Enhanced Skill & Task Renderers

**SkillRenderer (enhanced):**
- Purple badge with skill name
- Args displayed via `InlineMarkdown` (not plain text)
- Path-like args get `FilePathHeader` treatment

**TaskRenderer (enhanced):**
- Blue subagent_type chip
- Name as bold label
- Description as subtitle text
- Prompt in `InlineMarkdown` (always visible in verbose, collapsed to 3 lines otherwise)

### 5. What Stays the Same

- `CompactCodeBlock` — untouched
- `MessageTyped` (compact mode) — untouched
- System/Progress card dispatchers — untouched
- `ActionLogTab` — untouched
- Data model (`RichMessage`, `Message`) — untouched
- Streaming/WS pipeline — untouched
- `JsonKeyValueChips` — still available for non-tool contexts

## Files to Create/Modify

| File | Action |
|------|--------|
| `src/hooks/use-paired-messages.ts` | **Create** — pairing hook |
| `src/components/live/PairedToolCard.tsx` | **Create** — unified tool card |
| `src/components/live/ToolRenderers.tsx` | **Modify** — add SmartMcpRenderer, enhance SkillRenderer/TaskRenderer |
| `src/components/live/RichPane.tsx` | **Modify** — use `usePairedMessages()`, replace ToolUseMessage/ToolResultMessage with PairedToolCard dispatch |

## Non-Goals

- No changes to compact mode (ConversationView)
- No changes to data model or streaming
- No per-MCP-server custom renderers (smart generic only)
- No ActionLogTab changes
