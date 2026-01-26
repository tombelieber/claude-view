# Claude View Bugfixes & UX Enhancements - Design Specification

> Fixes for 6 issues found via E2E testing, with UX enhancements following industry best practices.

---

## Issues Summary

| # | Issue | Root Cause | Priority |
|---|-------|------------|----------|
| 1 | SessionCard shows only first line, hard to differentiate | Data not fully populated or displayed | Critical |
| 2 | First session always shows "Active" incorrectly | 5-min heuristic unreliable, no real-time integration | High |
| 3 | Sidebar tool stats always 0 | `toolCounts` not reaching frontend | Critical |
| 4 | Chat loads slowly, no virtualization | All messages rendered at once | High |
| 5 | XML/templated messages render as raw text | No special handling for structured content | Medium |
| 6 | Top skills in usage page always 0 | `skillsUsed` not reaching frontend | Critical |

---

## Design Decisions

### Decision #1: Remove Active Session Feature (Issue #2)

**Rationale:** No robust way to determine truly active sessions without real-time CLI integration.

**Action:**
- Remove `isActive` prop from SessionCard
- Remove green left border, pulsing dot, "Active" label
- Remove `activeCount` from Sidebar project stats
- Keep data model intact for future real-time integration (Option C)

### Decision #2: Chat Virtualization with react-virtuoso (Issue #4)

**Rationale:** Industry standard for chat UIs. Handles variable heights, auto-scroll, load-on-scroll.

**Implementation:**
```tsx
<Virtuoso
  data={messages}
  followOutput="smooth"
  overscan={800}
  increaseViewportBy={{ top: 400, bottom: 400 }}
  alignToBottom
  atBottomThreshold={100}
/>
```

### Decision #3: Progressive Disclosure Cards for XML (Issue #5)

**Rationale:** Used by Claude.ai, ChatGPT, Cursor. Reduces cognitive load while preserving access to details.

**Rendering hierarchy:**
1. Parse known XML tags → Semantic cards (collapsed by default)
2. Unknown XML → Syntax-highlighted code block (collapsed)
3. Plain markdown → Normal rendering (expanded)

---

## Component Designs

### 1. Enhanced SessionCard (Preview Card)

**Purpose:** Show enough context to identify the right session at a glance. Click → enter full chat.

**Layout:**
```
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  "fix the login bug in the auth flow"                                   │
│  → "looks good, let's ship it"                                  2h ago  │
│                                                                         │
│  auth.ts  Login.tsx  api/session.ts  +2                                 │
│                                                                         │
│  ✏️ 12   💻 3   👁️ 8      💬 47 msgs · 12 turns      /commit /brainstorm │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

**Data displayed:**

| Row | Content | Source |
|-----|---------|--------|
| 1 | Started phrase (first user message) | `session.preview` |
| 2 | Ended phrase (last user message) + timestamp | `session.lastMessage` + `session.modifiedAt` |
| 3 | Files touched (max 3, then +N) | `session.filesTouched` |
| 4 | Tool counts + message stats + skills | `session.toolCounts` + `session.messageCount` + `session.turnCount` + `session.skillsUsed` |

**Visual hierarchy:**
- **Started phrase:** `text-sm font-medium text-gray-900` - primary identifier
- **Ended phrase:** `text-sm text-gray-600` with `→` prefix showing conversation flow
- **Timestamp:** `text-xs text-gray-400` - top-right aligned
- **Files:** Small pills `text-xs bg-gray-100 rounded px-1.5 py-0.5`
- **Bottom row:** Icon + number pairs, monospace numbers, evenly spaced

**New fields needed from API:**
- `messageCount: number` - total messages in session
- `turnCount: number` - human→assistant exchange pairs

**Card states:**
```css
/* Default */
.session-card {
  background: white;
  border: 1px solid var(--border-subtle);
}

/* Hover */
.session-card:hover {
  background: var(--bg-secondary);
  border-color: var(--border-default);
  box-shadow: 0 1px 3px rgba(0,0,0,0.04);
}

/* Selected (currently viewing) */
.session-card--selected {
  background: #eff6ff;
  border-color: var(--color-interactive);
  box-shadow: 0 0 0 1px var(--color-interactive);
}
```

---

### 2. XML Progressive Disclosure Cards (in ConversationView)

**Purpose:** Render structured XML content as scannable cards instead of raw text.

**Supported XML types:**

| Tag | Icon | Summary |
|-----|------|---------|
| `<observed_from_primary_session>` | 📖 | `{what_happened}` · `{file_path}` |
| `<observation>` | 🧠 | `{type}` · `{title}` |
| `<tool_call>` | 🔧 | `{tool_name}` · `{parameters summary}` |
| Unknown XML | 📄 | "Structured content" |

**Collapsed state (default):**
```
┌──────────────────────────────────────────────────────┐
│ 📖 Read · session-discovery-ux-design.md   ▶ Expand  │
└──────────────────────────────────────────────────────┘
```

**Expanded state:**
```
┌──────────────────────────────────────────────────────┐
│ 📖 Read · session-discovery-ux-design.md   ▼ Collapse│
├──────────────────────────────────────────────────────┤
│ Path: docs/plans/2026-01-26-session-discovery-ux... │
│ Lines: 654                                           │
│ Outcome: Success                                     │
│                                                      │
│ ┌────────────────────────────────────────────────┐   │
│ │ # Session Discovery UX - Design Spec...        │   │
│ │ > Visual design system and component...        │   │
│ │ ...                                            │   │
│ └────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

**Observation card expanded:**
```
┌──────────────────────────────────────────────────────┐
│ 🧠 Discovery · Session Discovery UX Spec   ▼ Collapse│
├──────────────────────────────────────────────────────┤
│ Key facts:                                           │
│ • Sage green (#7c9885) accent color scheme           │
│ • Command palette with dark modal (#111113)          │
│ • Enhanced session cards with tool stats             │
│ • 5 more facts...                                    │
│                                                      │
│ Files read: session-discovery-ux-design.md           │
└──────────────────────────────────────────────────────┘
```

**Fallback for unknown XML:**
```
┌──────────────────────────────────────────────────────┐
│ 📄 Structured content                      ▶ Expand  │
└──────────────────────────────────────────────────────┘

     ↓ expanded ↓

┌──────────────────────────────────────────────────────┐
│ 📄 Structured content                      ▼ Collapse│
├──────────────────────────────────────────────────────┤
│ ```xml                                               │
│ <unknown_tag>                                        │
│   <child>content</child>                             │
│ </unknown_tag>                                       │
│ ```                                                  │
└──────────────────────────────────────────────────────┘
```

---

### 3. Virtualized ConversationView

**Current (problem):**
```tsx
{session.messages.map((message, index) => (
  <Message key={index} message={message} />
))}
```

**New (solution):**
```tsx
import { Virtuoso } from 'react-virtuoso'

<Virtuoso
  data={session.messages}
  itemContent={(index, message) => (
    <Message key={message.id || index} message={message} />
  )}
  followOutput="smooth"
  overscan={800}
  increaseViewportBy={{ top: 400, bottom: 400 }}
  alignToBottom
  atBottomThreshold={100}
  className="flex-1 overflow-auto"
/>
```

**Benefits:**
- Only renders visible messages + 800px buffer
- Auto-scrolls to bottom on load
- Handles variable height messages (code blocks)
- Maintains scroll position on resize

---

## Data Pipeline Debug Checklist

Issues #1, #3, #6 likely share a root cause: data not flowing from backend to frontend.

**Debug path:**

1. **JSONL Parser** (`src/server/parser.ts`)
   - [ ] Verify tool calls extracted: `toolCounts.edit`, `toolCounts.read`, `toolCounts.bash`
   - [ ] Verify skills extracted: regex `/\/[\w:-]+/g` on user messages
   - [ ] Verify message count calculated
   - [ ] Verify last message captured

2. **Sessions API** (`src/server/sessions.ts`)
   - [ ] Verify `SessionInfo` includes all fields
   - [ ] Verify aggregation logic for `toolCounts`
   - [ ] Verify `skillsUsed` array populated

3. **Frontend hooks** (`src/hooks/use-projects.ts`, `use-session.ts`)
   - [ ] Verify API response shape matches expected types
   - [ ] Check for silent failures in data transformation

4. **Components**
   - [ ] Verify SessionCard receives props
   - [ ] Verify Sidebar stats calculation uses correct fields

---

## Implementation Order

| # | Task | Files | Complexity |
|---|------|-------|------------|
| 1 | Debug & fix data pipeline | `parser.ts`, `sessions.ts` | Medium |
| 2 | Remove active session indicators | `SessionCard.tsx`, `Sidebar.tsx` | Low |
| 3 | Add messageCount, turnCount to API | `parser.ts`, `sessions.ts`, types | Low |
| 4 | Enhance SessionCard display | `SessionCard.tsx` | Medium |
| 5 | Install & integrate react-virtuoso | `package.json`, `ConversationView.tsx` | Medium |
| 6 | Create XML card components | `components/XmlCard.tsx`, `Message.tsx` | High |

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/server/parser.ts` | Fix tool/skill extraction, add messageCount/turnCount |
| `src/server/sessions.ts` | Verify data aggregation, add new fields to API |
| `src/components/SessionCard.tsx` | Remove active indicators, add new data display |
| `src/components/Sidebar.tsx` | Remove activeCount, verify tool stats binding |
| `src/components/ConversationView.tsx` | Replace map with Virtuoso |
| `src/components/Message.tsx` | Add XML detection and card rendering |
| `src/components/XmlCard.tsx` | New component for progressive disclosure cards |
| `package.json` | Add react-virtuoso dependency |

---

## Success Criteria

- [ ] SessionCard shows: started phrase, ended phrase, files, tool counts, message count, turns, skills
- [ ] No "Active" indicators anywhere in UI
- [ ] Sidebar tool stats show real numbers (not 0)
- [ ] StatsDashboard skills show real numbers (not 0)
- [ ] Long conversations load quickly with virtualization
- [ ] XML content renders as collapsible cards
- [ ] Click SessionCard → navigate to full chat view
