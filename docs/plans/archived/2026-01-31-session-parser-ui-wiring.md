---
status: done
date: 2026-01-31
---

# Session Parser + UI Wiring — Full 7-Type Conversation View

**Goal:** Upgrade the session parser (`crates/core/src/parser.rs`) to emit all 7 JSONL line types with proper roles, metadata, and parentUuid, then wire the data to the existing frontend components that are currently dead code.

**Problem:** The deep indexer (Pass 2) already handles all 7 line types for metrics/analytics. But the session parser (Pass 1) that serves `/api/session` to the conversation UI only emits `User | Assistant` messages. All system event cards, progress event cards, threading, and Track 4 components (queue/snapshot/summary) are built and tested but never receive data.

**Scope:** Backend parser rewrite + frontend wiring. No new components needed.

---

## Audit Findings (from real JSONL data)

Audited session `c360e1ac` (2,616 lines, all 7 types present):

| Line Type | Count | % | Current Parser | After |
|-----------|-------|---|----------------|-------|
| `progress` | 1,853 | 70.9% | Dropped (`Other`) | `Role::Progress` with metadata |
| `assistant` | 409 | 15.6% | `Role::Assistant` (all) | Split: `Assistant` (text), `ToolUse` (tool-only) |
| `user` | 284 | 10.9% | `Role::User` (all) | Split: `User` (string), `ToolResult` (array w/tool_result) |
| `system` | 44 | 1.7% | Dropped | `Role::System` with subtype metadata |
| `file-history-snapshot` | 22 | 0.8% | Dropped | `Role::System` (metadata.type = "file-history-snapshot") |
| `queue-operation` | 2 | 0.08% | Dropped | `Role::System` (metadata.type = "queue-operation") |
| `summary` | 2 | 0.08% | Dropped | `Role::Summary` |

### Key finding: user messages are mostly tool_results

In the audited session, 254 of 273 non-meta user messages (93%) have array content with `tool_result` blocks — they're tool outputs being sent back to Claude, NOT real user prompts. The current parser shows all of them as "You" messages, which is misleading.

### Subtypes observed in real data

**System subtypes:** `stop_hook_summary` (15), `api_error` (14), `turn_duration` (12), `local_command` (2), `compact_boundary` (1)

**Progress subtypes:** `bash_progress` (1,152), `hook_progress` (647), `agent_progress` (53), `waiting_for_task` (1)

**Not observed:** `mcp_progress` (may exist in other sessions, component should still exist)

---

## Design

### 1. Data Model Changes (Rust)

#### Extend `Role` enum (`crates/core/src/types.rs`)

```rust
pub enum Role {
    User,        // Real user prompt (string content)
    Assistant,   // Claude response with text
    ToolUse,     // Assistant message with only tool_use blocks (no text)
    ToolResult,  // User message with tool_result array content
    System,      // System events + queue-ops + file-snapshots
    Progress,    // Progress events (agent, bash, hook, mcp, waiting)
    Summary,     // Auto-generated session summaries
}
```

#### Extend `Message` struct (`crates/core/src/types.ts`)

```rust
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub thinking: Option<String>,
    // NEW:
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
```

`metadata` carries subtype-specific fields as raw JSON. The frontend already knows how to destructure these in `renderSystemSubtype()` and `renderProgressSubtype()`.

#### Classification rules

| JSONL `type` | Content check | UI Role |
|---|---|---|
| `"user"` + `isMeta=true` | — | Skipped (unchanged) |
| `"user"` + content is `string` | — | `Role::User` |
| `"user"` + content is `array` with `tool_result` blocks | — | `Role::ToolResult` |
| `"assistant"` + has text blocks | — | `Role::Assistant` |
| `"assistant"` + only `tool_use` blocks (no text) | — | `Role::ToolUse` |
| `"assistant"` + only thinking blocks | — | Merged into next assistant (existing behavior) |
| `"system"` | any subtype | `Role::System`, metadata = `{subtype, durationMs, error, ...}` |
| `"progress"` | any `data.type` | `Role::Progress`, metadata = `{type: data.type, ...data}` |
| `"queue-operation"` | — | `Role::System`, metadata = `{type: "queue-operation", operation, content}` |
| `"file-history-snapshot"` | — | `Role::System`, metadata = `{type: "file-history-snapshot", snapshot, isSnapshotUpdate}` |
| `"summary"` | — | `Role::Summary`, metadata = `{summary, leafUuid}` |

### 2. Parser Rewrite (`crates/core/src/parser.rs`)

Replace the current `JsonlEntry` enum approach with `serde_json::Value` parsing (same strategy the deep indexer already uses successfully):

1. Parse each line as `serde_json::Value`
2. Read `type` field as string
3. `match` on type, classify into Role, extract metadata
4. Emit `Message` with `uuid`, `parent_uuid`, `metadata` populated

The existing behavior is preserved:
- `isMeta` user messages still skipped
- Thinking-only assistant messages still merged into next
- Empty messages still skipped
- Command tag cleaning still applied to user string content
- Tool call aggregation still works for assistant messages

New behavior:
- `system`, `progress`, `queue-operation`, `file-history-snapshot`, `summary` lines produce `Message` objects instead of being dropped
- User messages with `tool_result` array content get `Role::ToolResult` instead of `Role::User`
- Assistant messages with only `tool_use` blocks get `Role::ToolUse` instead of `Role::Assistant`
- `uuid` and `parentUuid` extracted from every line that has them

### 3. Frontend — Compact/Full Toggle

#### State in `ConversationView.tsx`

```tsx
const [viewMode, setViewMode] = useState<'compact' | 'full'>('compact')
```

#### Segmented control in header

Placed next to HTML/PDF export buttons. Uses `aria-pressed` for accessibility, Lucide `Eye`/`Code` icons with `aria-hidden="true"`, `cursor-pointer` on inactive state, `transition-colors duration-200` for micro-interaction.

```tsx
<div className="flex items-center gap-1 bg-gray-100 dark:bg-gray-800 rounded-md p-0.5">
  <button
    onClick={() => setViewMode('compact')}
    aria-pressed={viewMode === 'compact'}
    className={cn(
      'px-3 py-1.5 text-xs font-medium rounded transition-colors duration-200',
      viewMode === 'compact'
        ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
        : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
    )}
  >
    <Eye className="w-3.5 h-3.5 inline mr-1.5" aria-hidden="true" />
    Smart
  </button>
  <button
    onClick={() => setViewMode('full')}
    aria-pressed={viewMode === 'full'}
    className={cn(
      'px-3 py-1.5 text-xs font-medium rounded transition-colors duration-200',
      viewMode === 'full'
        ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
        : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
    )}
  >
    <Code className="w-3.5 h-3.5 inline mr-1.5" aria-hidden="true" />
    Full
  </button>
</div>
```

Hidden count badge when in compact mode:

```tsx
{viewMode === 'compact' && hiddenCount > 0 && (
  <span className="text-xs text-gray-400 dark:text-gray-500 ml-2">
    {hiddenCount} hidden
  </span>
)}
```

#### Message filtering

```tsx
function filterMessages(messages: Message[], mode: 'compact' | 'full'): Message[] {
  if (mode === 'full') return messages

  return messages.filter(msg => {
    if (msg.role === 'user') return true          // real prompts
    if (msg.role === 'assistant') return true      // text responses
    if (msg.role === 'tool_use') return false      // collapsed into assistant
    if (msg.role === 'tool_result') return false   // part of tool flow
    if (msg.role === 'system') return false        // operational noise
    if (msg.role === 'progress') return false      // background work
    if (msg.role === 'summary') return false       // metadata
    return false
  })
}
```

#### Wiring MessageTyped props

```tsx
<MessageTyped
  message={message}
  messageIndex={index}
  messageType={message.role}
  metadata={message.metadata}
  parentUuid={message.parentUuid}
  indent={computeIndent(message, messageMap)}
  isChildMessage={!!message.parentUuid}
/>
```

### 4. Track 4 Component Wiring (`MessageTyped.tsx`)

Import and render the three Track 4 components that currently have zero usage:

```tsx
import { MessageQueueEventCard } from './MessageQueueEventCard'
import { FileSnapshotCard } from './FileSnapshotCard'
import { SessionSummaryCard } from './SessionSummaryCard'
```

Render in the content section:

- `Role::System` with `metadata.type === "queue-operation"` → `<MessageQueueEventCard>`
- `Role::System` with `metadata.type === "file-history-snapshot"` → `<FileSnapshotCard>`
- `Role::Summary` → `<SessionSummaryCard>`

### 5. Complete Rendering Dispatch

| Role | Color | Component |
|------|-------|-----------|
| `User` | Blue (#93c5fd) | Existing user message UI + markdown |
| `Assistant` | Orange (#fdba74) | Existing assistant message UI + markdown |
| `ToolUse` | Purple (#d8b4fe) | Assistant shell + ToolCallCard inside |
| `ToolResult` | Green (#86efac) | Tool output content display |
| `System` | Amber (#fcbf49) | Subtype dispatch → TurnDurationCard, ApiErrorCard, CompactBoundaryCard, HookSummaryCard, LocalCommandEventCard, MessageQueueEventCard, FileSnapshotCard |
| `Progress` | Indigo (#a5b4fc) | Subtype dispatch → AgentProgressCard, BashProgressCard, HookProgressCard, McpProgressCard, TaskQueueCard |
| `Summary` | Rose (#fb923c) | SessionSummaryCard |

---

## Files Changed

| File | Change | LOC estimate |
|------|--------|-------------|
| `crates/core/src/types.rs` | Extend `Role` (7 variants), add `uuid`/`parent_uuid`/`metadata` to `Message` | ~30 |
| `crates/core/src/parser.rs` | Rewrite `parse_session()` — classify all 7 types, extract metadata | ~200 |
| `src/types/generated/Role.ts` | Auto-regenerated | auto |
| `src/types/generated/Message.ts` | Auto-regenerated | auto |
| `src/components/ConversationView.tsx` | viewMode state, toggle UI, filterMessages, prop passing | ~60 |
| `src/components/MessageTyped.tsx` | Import Track 4 cards, wire queue/snapshot/summary | ~30 |

**Total: ~320 lines changed across 4 hand-edited files. Zero new files.**

---

## What Already Works (no changes needed)

- All 15 specialized card components (tested, 230+ test cases)
- Threading indentation + dashed connectors in MessageTyped
- ARIA attributes for accessibility
- XmlCard → ToolCallCard + StructuredDataCard integration
- `renderSystemSubtype()` dispatch for 5 system subtypes
- `renderProgressSubtype()` dispatch for 5 progress subtypes
- TYPE_CONFIG color system for all 7 roles

---

## Testing Strategy

**Backend:**
- Add golden fixture with all 7 types to `crates/core/tests/fixtures/`
- Test role classification (user string → User, user array → ToolResult, etc.)
- Test metadata extraction for each subtype
- Test parentUuid passthrough
- Existing parser tests must still pass (backward compat)

**Frontend:**
- Existing 230+ component tests remain valid
- Add integration test: compact mode filters correctly
- Add integration test: full mode shows all message types
- Manual: verify toggle switches view, count badge updates

---

## UI/UX Rules Applied

- Segmented control (not checkbox) for view mode — communicates mutual exclusion
- `aria-pressed` on toggle buttons (semantic HTML, not div with role)
- `aria-hidden="true"` on decorative Lucide icons
- `cursor-pointer` on all interactive elements
- `transition-colors duration-200` (150-300ms micro-interaction)
- 4.5:1+ text contrast in both light and dark modes
- Touch targets >= 44px via padding
- No emojis as icons — SVG only (Lucide)
- `prefers-reduced-motion` support via Tailwind `motion-reduce:`
