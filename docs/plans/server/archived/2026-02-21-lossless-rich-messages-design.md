# Lossless RichMessage Conversion — Design

**Date:** 2026-02-21
**Status:** Approved
**Branch:** fix/token-deduplication

## Problem

ConversationView has two rendering paths that have diverged:

- **Chat mode** (compact): `filterMessages('compact')` → `MessageTyped` — shows user + assistant only. Works as intended.
- **Verbose mode**: `messagesToRichMessages()` → `HistoryRichPane` → `RichPane` — **lossy conversion**. System events, progress events, and summaries are either skipped or flattened into generic assistant messages.

The `messagesToRichMessages()` function drops or flattens 3 of 7 JSONL message types:

| JSONL role | Current behavior | Data lost |
|---|---|---|
| user | → RichMessage{type:'user'} | None |
| assistant | → thinking + assistant | None |
| tool_use | → decomposed tool_calls | None |
| tool_result | → result | None |
| **system** | → mapped to 'assistant' | **All subtype metadata** (turn_duration, api_error, compact_boundary, hook_summary, local_command, queue-operation, file-history-snapshot) |
| **progress** | → **SKIPPED** | **Everything** (agent_progress, bash_progress, hook_progress, mcp_progress, waiting_for_task) |
| **summary** | → **SKIPPED** | **Everything** (session summary, leaf UUID) |

## Design Decisions

1. **Chat mode = pure ChatGPT**: user + assistant text only. No thinking blocks, no tools, no system events.
2. **Verbose mode = everything**: all 7 JSONL types, beautifully rendered in RichPane.
3. **RichPane stays the verbose renderer**: keep its terminal aesthetic. Don't switch to MessageTyped for verbose.
4. **`messagesToRichMessages()` becomes lossless**: carry system/progress/summary with full metadata.
5. **Reuse existing specialized cards**: TurnDurationCard, AgentProgressCard, etc. already exist in MessageTyped's imports — import them into RichPane's new message card components.

## Changes

### 1. Extend `RichMessage` type (`RichPane.tsx`)

Add `'system' | 'progress' | 'summary'` to the type union. Add `metadata` field:

```typescript
export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking'
      | 'error' | 'hook'
      | 'system' | 'progress' | 'summary'   // NEW
  content: string
  name?: string
  input?: string
  inputData?: unknown
  ts?: number
  category?: ActionCategory
  metadata?: Record<string, any>   // NEW
}
```

### 2. Update `messagesToRichMessages()` (`message-to-rich.ts`)

Replace the skip/flatten behavior for system, progress, summary:

```typescript
case 'system': {
  const content = stripCommandTags(msg.content)
  result.push({
    type: 'system',
    content: content || '',
    ts,
    metadata: msg.metadata ?? undefined,
  })
  break
}

case 'progress': {
  const content = stripCommandTags(msg.content)
  result.push({
    type: 'progress',
    content: content || '',
    ts,
    metadata: msg.metadata ?? undefined,
  })
  break
}

case 'summary': {
  result.push({
    type: 'summary',
    content: msg.content || '',
    ts,
    metadata: msg.metadata ?? undefined,
  })
  break
}
```

### 3. Add MessageCard cases in RichPane (`RichPane.tsx`)

Three new card components that dispatch to existing specialized cards:

**SystemMessageCard** — dispatches on `metadata.type`:
- `turn_duration` → `TurnDurationCard`
- `api_error` → `ApiErrorCard`
- `compact_boundary` → `CompactBoundaryCard`
- `hook_summary` → `HookSummaryCard`
- `local_command` → `LocalCommandEventCard`
- `queue-operation` → `MessageQueueEventCard`
- `file-history-snapshot` → `FileSnapshotCard`
- Fallback: generic metadata key-value display

**ProgressMessageCard** — dispatches on `metadata.type`:
- `agent_progress` → `AgentProgressCard`
- `bash_progress` → `BashProgressCard`
- `hook_progress` → `HookProgressCard`
- `mcp_progress` → `McpProgressCard`
- `waiting_for_task` → `TaskQueueCard`
- Fallback: generic metadata display

**SummaryMessageCard** — renders `SessionSummaryCard`

Add to `MessageCard` switch:
```typescript
case 'system':
  return <SystemMessageCard message={message} />
case 'progress':
  return <ProgressMessageCard message={message} />
case 'summary':
  return <SummaryMessageCard message={message} />
```

### 4. RichPane filter update (`RichPane.tsx`)

The `displayMessages` filter in RichPane:
- **Compact** (verboseMode=false): unchanged — user + assistant + error + AskUserQuestion
- **Verbose** (verboseMode=true, filter='all'): include system, progress, summary
- **Category filter**: system/progress/summary have no category, so always show when verbose regardless of active category filter (they're structural, not tool-related)

### 5. ConversationView chat mode: hide thinking (`ConversationView.tsx`)

Add `showThinking` prop to `MessageTyped`. Default `true`. Pass `showThinking={false}` in compact mode:

```tsx
<MessageTyped
  message={message}
  showThinking={false}   // ← NEW
  ...
/>
```

In `MessageTyped`, gate the thinking block:
```tsx
{showThinking !== false && message.thinking && (
  <ThinkingBlock thinking={message.thinking} />
)}
```

### 6. Cleanup

- **Delete** the `HistoryRichPane` wrapper in ConversationView (no longer needed — ConversationView verbose mode still uses RichPane, but through the existing `richMessages` path which is now lossless)
- Actually, `HistoryRichPane` stays — it's the bridge that reads `verboseMode` from the store and passes it to `RichPane`. The wrapper is fine. What changes is the data feeding it: `richMessages` now carries all types.

## Files Changed

| File | Change |
|---|---|
| `src/components/live/RichPane.tsx` | Extend RichMessage type, add SystemMessageCard/ProgressMessageCard/SummaryMessageCard, update MessageCard switch, update displayMessages filter |
| `src/lib/message-to-rich.ts` | Emit system/progress/summary instead of skipping |
| `src/components/MessageTyped.tsx` | Add `showThinking` prop |
| `src/components/ConversationView.tsx` | Pass `showThinking={false}` in compact mode |

## What Stays Unchanged

- Live monitor (WebSocket → `parseRichMessage()` → RichPane) — unaffected
- `useLiveSessionMessages` hook — unaffected
- `categorizeTool()` — unaffected
- All existing specialized cards (TurnDurationCard, etc.) — reused, not modified
- `filterMessages()` — unchanged
- Threading in compact mode — unchanged
