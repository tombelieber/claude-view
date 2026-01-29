# Conversation UI Redesign — Full 7-Type JSONL Parser Support

**Status:** Redesign + Implementation Guide (MessageTyped.tsx component created)
**Date:** 2026-01-29
**Context:** New Full JSONL Parser now extracts 7 semantic message types. Existing Message.tsx/XmlCard.tsx were designed for 5 XML card types + flat user/assistant distinction.

---

## Problem Statement

**Current UI:**
- Treats all messages as flat `user` / `assistant` distinction
- 9 XML card type variants (observation, tool_call, task_notification, etc.)
- No semantic visual hierarchy for message **types**
- System/progress/summary lines have no dedicated UI (hidden in XML or text)
- Dense conversations lose structural clarity

**New Schema:**
- 7 semantic message types: user, assistant, tool_use, tool_result, system, progress, summary
- Each type carries operational semantics (errors, tokens, agent events)
- XML parsing is now secondary to type-based rendering

---

## Design Direction: "Editorial Refined + Type Clarity"

**Aesthetic Principles:**
1. **Type-First Rendering** — Icon + accent color identify message type in 150ms
2. **Semantic Color Hierarchy** — 7 distinct accent colors (blue, orange, purple, green, amber, indigo, rose)
3. **Dense Information Density** — Support 500+ message conversations without collapse
4. **Editorial Typography** — Refined fonts (Fira Sans/Roboto Mono), generous vertical spacing
5. **Visual Accent System** — Left border (4px) + icon badge + type label
6. **Metadata First-Class** — System/progress events rendered with structured metadata cards

---

## Component Architecture

### 1. MessageTyped.tsx (NEW)
Replaces Message.tsx for new parser schema.

**Core Features:**
- 7-type support: user, assistant, tool_use, tool_result, system, progress, summary
- TYPE_CONFIG object: icon, accent color, badge styling for each type
- SystemMetadataCard component: renders structured event data (errors, retries, agent events)
- Backward compatible with existing XmlCard extraction
- Thinking block support (from assistant messages)
- Tool calls summary with inline badges

**Type Configuration:**

| Type | Icon | Border | Badge | Use Case |
|------|------|--------|-------|----------|
| **user** | User | Blue-300 | Blue-100 | User prompts, tool results |
| **assistant** | MessageSquare | Orange-300 | Orange-100 | Model responses, reasoning |
| **tool_use** | Wrench | Purple-300 | Purple-100 | Individual tool invocations |
| **tool_result** | CheckCircle | Green-300 | Green-100 | Tool execution output |
| **system** | AlertCircle | Amber-300 | Amber-100 | Errors, duration, retries |
| **progress** | Zap | Indigo-300 | Indigo-100 | Agent spawns, bash progress |
| **summary** | BookOpen | Rose-300 | Rose-100 | Session auto-summaries |

### 2. Message Layout Hierarchy

```
┌─ Type Accent Border (4px, color-coded)
│
├─ Header Row
│  ├─ Icon Badge [Icon + Type Name]
│  ├─ Timestamp
│  └─ Copy Button
│
├─ Content Area
│  ├─ Thinking Block (if present)
│  ├─ Main Text/Markdown
│  ├─ XML Cards (observation, tool_call, etc.)
│  ├─ Metadata Card (system/progress events)
│  └─ Tool Calls Summary (if present)
│
└─ (no footer — compact design)
```

### 3. Message Type Semantics

#### `user` Messages
- **Content:** User prompts, tool results, branches
- **Icon:** User
- **Border:** Blue-300
- **Special Fields:** isSidechain, permissionMode, agentId
- **Rendering:** Standard markdown + tool result blocks

#### `assistant` Messages
- **Content:** Model responses with reasoning and tool calls
- **Icon:** MessageSquare
- **Border:** Orange-300
- **Special Fields:** model, stopReason, thinking (Claude's reasoning)
- **Rendering:** Markdown with ThinkingBlock + tool calls list

#### `tool_use` Messages
- **Content:** Tool invocation details (file_path, command, pattern, etc.)
- **Icon:** Wrench
- **Border:** Purple-300
- **Metadata:** tool name, input parameters
- **Rendering:** Collapsed structured card showing: Tool Name | Input Summary

#### `tool_result` Messages
- **Content:** Tool execution output or error
- **Icon:** CheckCircle
- **Border:** Green-300 (or red if error)
- **Metadata:** success/failure, output length
- **Rendering:** Terminal-style code block or success summary

#### `system` Messages
- **Content:** Operational events (usually metadata-only)
- **Icon:** AlertCircle
- **Border:** Amber-300
- **Metadata Keys:**
  - `duration_ms` — Turn duration
  - `api_error_count` — Errors in turn
  - `retry_count` — Retries
  - `hook_blocked` — Hook blocks
- **Rendering:** Metadata card with amber tint

#### `progress` Messages
- **Content:** Sub-agent, bash, hook, MCP progress events
- **Icon:** Zap
- **Border:** Indigo-300
- **Metadata Keys:**
  - `agent_spawn_count`
  - `bash_progress_count`
  - `mcp_progress_count`
  - `hook_progress_count`
- **Rendering:** Progress timeline card with indigo tint

#### `summary` Messages
- **Content:** Auto-generated session summaries
- **Icon:** BookOpen
- **Border:** Rose-300
- **Metadata:** generation timestamp
- **Rendering:** Full markdown with callout styling

---

## Visual Design Details

### Color Palette

```css
/* Type accent colors (left border, 4px) */
--type-user:       rgb(147, 197, 253)  /* blue-300 */
--type-assistant:  rgb(253, 186, 116)  /* orange-300 */
--type-tool-use:   rgb(216, 180, 254)  /* purple-300 */
--type-tool-result: rgb(134, 239, 172) /* green-300 */
--type-system:     rgb(252, 191, 73)   /* amber-300 */
--type-progress:   rgb(165, 180, 252)  /* indigo-300 */
--type-summary:    rgb(251, 146, 60)   /* rose-300 */
```

### Typography

```css
/* Message header: type name + timestamp */
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
font-weight: 600;
font-size: 0.875rem; /* 14px */
line-height: 1.25rem;

/* Message content: standard prose */
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
font-size: 0.875rem;
line-height: 1.5;

/* Metadata card: monospace */
font-family: 'Roboto Mono', 'Courier New', monospace;
font-size: 0.75rem; /* 12px */
line-height: 1.25;
```

### Spacing

- **Vertical rhythm:** 1rem (16px) between messages
- **Internal padding:** 1rem (16px) top/bottom, 1rem left/right
- **Icon-to-content gap:** 0.75rem (12px)
- **Content sections:** 0.75rem (12px) gap (thinking, text, metadata, tools)

### Hover States

- **Message card:** bg-gray-50/50 (subtle hover)
- **Copy button:** opacity 0 → 100 (on hover)
- **Links:** text-blue-700 (underlined)

---

## Implementation: MessageTyped.tsx

### Usage

```tsx
import { MessageTyped } from '@/components/MessageTyped'

// Basic usage (infers type from message.role)
<MessageTyped message={message} messageIndex={idx} />

// Explicit type (for system/progress messages)
<MessageTyped
  message={systemEvent}
  messageType="system"
  metadata={{ duration_ms: 245, api_error_count: 1 }}
/>

// Progress event with metadata
<MessageTyped
  message={progressEvent}
  messageType="progress"
  metadata={{
    agent_spawn_count: 3,
    bash_progress_count: 5,
    mcp_progress_count: 0
  }}
/>
```

### Key Components

**1. TYPE_CONFIG**
Maps type → icon, accent color, badge styling
```typescript
const TYPE_CONFIG = {
  user: { accent: 'border-blue-300', icon: User, label: 'You' },
  assistant: { accent: 'border-orange-300', icon: MessageSquare, label: 'Claude' },
  // ... 5 more types
}
```

**2. SystemMetadataCard()**
Renders structured event data in colored cards
```typescript
<SystemMetadataCard metadata={{ duration_ms: 245, api_error_count: 1 }} type="system" />
```

**3. TypeBadge()**
Icon + label chip for type identification
```typescript
<TypeBadge type="system" label="System Event" />
```

---

## Integration Steps

1. **Import MessageTyped in SessionView.tsx**
   - Replace `Message` with `MessageTyped`
   - Pass `messageType` prop when rendering parser-provided types

2. **Update API response to include type**
   - Backend: ParsedSession should expose `message_type` field
   - Frontend: useFetch hook maps JSONL type → messageType prop

3. **Backward compatibility**
   - Keep Message.tsx for legacy sessions
   - Use MessageTyped for new parser schema
   - Feature flag: `useNewParserUI` env var

4. **Metadata extraction**
   - Pass system event metadata from API
   - Example: GET /api/session/:id returns messages with optional `metadata` field

---

## Density & Performance

**Dense conversation support:**
- 500+ messages: Virtual scrolling recommended (TanStack VirtualSlice)
- Message height: ~120px avg (thinking) to ~60px min (metadata-only)
- No re-renders on scroll (React.memo on MessageTyped)

**Performance optimizations:**
```typescript
export const MessageTyped = React.memo(MessageTypedComponent)
```

---

## Future Enhancements

1. **Message threading** — Collapse related system/progress msgs
2. **Search highlighting** — Visual emphasis on search results
3. **Message reactions** — Emoji/sentiment annotations
4. **Edit mode** — In-line editing of user messages (Phase 5)
5. **Export formatting** — PDF/Markdown export with type preservation

---

## References

- Full JSONL Parser Spec: `docs/plans/archived/2026-01-29-jsonl-parser-spec.md`
- Full Parser Implementation: `docs/plans/archived/2026-01-29-full-jsonl-parser.md`
- Current XmlCard: `src/components/XmlCard.tsx` (unchanged — still handles embedded XML)
- New Message Component: `src/components/MessageTyped.tsx` (NEW)
