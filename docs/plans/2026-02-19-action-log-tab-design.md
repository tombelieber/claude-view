---
status: approved
date: 2026-02-19
---

# Action Log Tab — Developer Debug Timeline

## Problem

Mission Control's Terminal tab shows the full conversation thread (messages, tool calls, results, thinking blocks). This is conversation-centric — great for following the chat, but developers building Claude Code skills, MCP servers, agents, and workflows need an **activity-centric** view to debug what the agent actually *did*, with what args, and what came back.

## Solution

Add a **Log** tab (5th tab) to the `SessionDetailPanel` that transforms existing WebSocket data into a compact, filterable action timeline with paired tool_use/tool_result rows, timing, and expandable raw JSON.

## Target Persona

Claude Code skill/workflow/agent/MCP developer who needs to:
- Verify their skill loaded with the correct prompt
- Debug MCP server calls (args in, response out)
- Trace tool call sequences and causal chains
- Identify failures, slow calls, and error patterns

## Architecture: Pure Frontend Transform (Approach A)

Zero backend changes. The WebSocket already streams all `tool_use`, `tool_result`, `message`, and `error` events to the side panel. The Log tab filters and reformats the same `RichMessage[]` array that the Terminal tab uses.

**Why this is optimal perf:**
- Data already in memory (no network round-trip)
- No JSONL re-parsing (messages already deserialized)
- Transform is a `useMemo` filter — O(n) over existing array
- Virtualized rendering (react-window) — only visible rows in DOM
- No data duplication — references same message objects as Terminal tab

## Data Model

```typescript
interface ActionItem {
  id: string                  // message ID (from RichMessage)
  timestamp: Date
  duration?: number           // ms between tool_use → tool_result
  category: 'skill' | 'mcp' | 'builtin' | 'agent' | 'error'
  toolName: string            // "Skill", "mcp__sentry__getIssues", "Edit", "Task"
  label: string               // human-readable summary
  status: 'success' | 'error' | 'pending'
  input: object | string      // tool_use input (args)
  output?: string             // tool_result output
}
```

### Category Mapping

| Pattern | Category | Developer use case |
|---|---|---|
| `Skill` tool call | `skill` | "Did my skill load? Was the prompt right?" |
| `mcp__*` tool calls | `mcp` | "Did my MCP server get called? What args? What response?" |
| `Task` (sub-agent spawn) | `agent` | "Did it dispatch correctly? What prompt?" |
| `Read`, `Write`, `Edit`, `Bash`, `Grep`, `Glob`, etc. | `builtin` | "What tool sequence did the agent follow?" |
| Any tool_result with error | `error` | "Where did it break? What was the error?" |

### Turn Separators

User/assistant messages appear as thin divider lines between action groups:

```
── User: "fix the auth bug in login.ts" ──────────
```

This provides **causality** — which prompt triggered which batch of tool calls — without duplicating the Terminal tab's conversation view.

## Visual Design

### Layout (within 480px side panel)

**1. Filter chips row (sticky top)**

```
[ All ] [ Skill (2) ] [ MCP (12) ] [ Builtin (34) ] [ Agent (3) ] [ Error (1) ]
```

- Pill-shaped, toggleable (click to show/hide category)
- Count badge on each chip
- "Error" chip: red-tinted when errors exist
- Active = filled bg, Inactive = outline

**2. Action timeline (scrollable, virtualized)**

Collapsed row (~40px):

```
🟢 Edit   src/components/App.tsx:42         0.2s  ▸
🟢 Bash   npm test                          3.8s  ▸
🔴 Bash   npm run build                    12.1s  ▸    ← red bg tint
🟡 mcp    sentry__getIssues                 ...   ●    ← pending (spinner)
```

Row anatomy:
- **Status dot**: green=success, red=error, amber=pending or slow (>5s)
- **Category badge**: monospace, color-coded per category
- **Label**: filepath, command, or tool name — truncated with tooltip
- **Duration**: right-aligned, monospace. Amber if >5s, red if >30s
- **Expand chevron**: `▸` collapsed, `▾` expanded

Expanded row (click to expand):

```
🟢 Edit   src/components/App.tsx:42         0.2s  ▾
┌─ Input ────────────────────────────── [Copy] ─┐
│ old_string: "const foo = bar"                  │
│ new_string: "const foo = baz"                  │
└────────────────────────────────────────────────┘
┌─ Output ───────────────────────────── [Copy] ─┐
│ File edited successfully                       │
└────────────────────────────────────────────────┘
```

- Input/Output in monospace, dark bg (`bg-gray-900`)
- Copy button on each block (copies raw JSON)
- Long outputs height-capped (max 200px) with "Show more"

**3. Turn separators**

Thin dashed divider with truncated user prompt text. Clicking scrolls to the corresponding message in the Terminal tab.

**4. Auto-scroll**

- If scrolled to bottom → auto-scroll on new item
- If scrolled up → "↓ New actions" pill at bottom

### Color System

| Category | Badge color | Rationale |
|---|---|---|
| `skill` | Purple (`bg-purple-500/10 text-purple-400`) | Skills are "magic" — purple conveys that |
| `mcp` | Blue (`bg-blue-500/10 text-blue-400`) | External services = blue (convention) |
| `builtin` | Gray (`bg-gray-500/10 text-gray-400`) | Background noise — shouldn't dominate |
| `agent` | Indigo (`bg-indigo-500/10 text-indigo-400`) | Agent spawns are notable but not alarming |
| `error` | Red (`bg-red-500/10 text-red-400`) | Universal error color |

### Duration Thresholds

| Duration | Treatment |
|---|---|
| < 5s | Normal (`text-gray-400`) |
| 5-30s | Amber (`text-amber-400`) |
| > 30s | Red (`text-red-400`) |
| Pending | Spinner + "..." |

## Components

| Component | Purpose |
|---|---|
| `ActionLogTab.tsx` | Main tab component — filter state, transform, layout |
| `ActionTimeline.tsx` | Virtualized list of `ActionItem` rows |
| `ActionRow.tsx` | Single collapsed/expanded action row |
| `ActionFilterChips.tsx` | Sticky filter chip bar with counts |
| `useActionItems.ts` | Hook: `(messages: RichMessage[]) => ActionItem[]` transform |

## Performance

| Concern | Solution |
|---|---|
| Long sessions (1000+ actions) | `react-window` VariableSizeList — only visible rows in DOM |
| Filter toggles | Bitmask in `useMemo` — recomputes only on messages/filter change |
| New messages streaming | Append-only — existing items never re-render |
| Auto-scroll | `scrollToItem(items.length - 1)` only when at bottom |
| Memory | No duplication — ActionItems reference same message data |

## Not In Scope (Future)

- Unified activity feed across ALL sessions (Phase 2 aggregation)
- Backend `/api/sessions/:id/actions` endpoint (for completed session history)
- Search within the log
- MCP server grouping/collapsing
- Export log to file
