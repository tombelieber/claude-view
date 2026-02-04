---
status: superseded
date: 2026-01-29
---

# Comprehensive Conversation UI Redesign
## Full 7-Type JSONL Parser + Hierarchy + Event Rendering

**Status:** Superseded — substance implemented, remaining gaps covered by 2026-02-02-thread-visualization-polish.md
**Date:** 2026-01-29
**Scope:** Complete audit + redesign of MessageTyped, XmlCard, and conversation threading
**Principles:** UI/UX Pro Max + Semantic type-first hierarchy + Accessible ARIA + Master craftsmanship

---

## 🎯 Executive Summary

The current UI implementation covers 7 message types but has **4 critical gaps**:

1. **XML Card Coverage** — `tool_call` and `unknown` types render as codeblock dumps instead of semantic UI
2. **Message Type Subtypes** — System/progress events have no specialized rendering (10 subtypes hidden)
3. **Conversation Hierarchy** — Parent-child relationships (`parentUuid`, `agentId`, tool chains) are invisible
4. **Missing Line Types** — `queue-operation`, `file-history-snapshot`, `summary` have zero UI

**This redesign delivers:**
- ✅ Full 10 XML card types with dedicated semantic UI (no codeblocks)
- ✅ All 10 system/progress subtypes rendered with semantic meaning
- ✅ Conversation threading with visual parent-child relationships
- ✅ Complete line type coverage (queue-ops, snapshots, summaries)

---

## 🏗️ Architecture: Three Parallel Redesign Tracks

### **Track 1: XML Card Completeness** (~8 semantic components)
**Goal:** Replace all codeblock fallbacks with crafted UI
**Gap:** `tool_call` (critical), `unknown` (fallback)

#### Current State
```
tool_call → CodeBlock (XML dump) ❌
unknown   → CodeBlock (XML dump) ❌
```

#### Designed State
```
tool_call        → ToolCallCard (semantic)        ✅
unknown          → StructuredDataCard (semantic)  ✅
```

**New Components Needed:**
1. **ToolCallCard** — What tool was called, with params + outcome
2. **StructuredDataCard** — Generic semantic XML rendering (not codeblock)

---

### **Track 2: System/Progress Event Subtypes** (~10 specialized cards)
**Goal:** Each subtype gets its own visual language
**Gap:** 5 system subtypes + 5 progress subtypes render as generic metadata cards

#### System Subtypes (Render Purpose-Built UI)
| Subtype | Current UI | Designed Component | Key Fields |
|---------|-----------|-------------------|-----------|
| `turn_duration` | Generic metadata | TurnDurationCard | durationMs, timestamps |
| `api_error` | Generic metadata | ApiErrorCard | error, retryAttempt, maxRetries |
| `compact_boundary` | Generic metadata | CompactBoundaryCard | trigger, preTokens |
| `hook_summary` | Generic metadata | HookSummaryCard | hookCount, hookErrors, durationMs |
| `local_command` | Generic metadata | LocalCommandCard | command description |

#### Progress Subtypes (Render Execution Flow)
| Subtype | Current UI | Designed Component | Key Fields |
|---------|-----------|-------------------|-----------|
| `agent_progress` | Generic metadata | AgentProgressCard | prompt, agentId, model, tokens |
| `bash_progress` | Generic metadata | BashProgressCard | command, output, exit code |
| `hook_progress` | Generic metadata | HookProgressCard | hookEvent, hookName, command |
| `mcp_progress` | Generic metadata | McpProgressCard | MCP server, method, params |
| `waiting_for_task` | Generic metadata | TaskQueueCard | wait duration, queue position |

---

### **Track 3: Conversation Hierarchy Visualization**
**Goal:** Show parent-child relationships visually
**Gap:** Messages are a flat list; parent-child connections invisible

#### Relationship Types to Visualize

**1. Message Threading** (`parentUuid`)
```
User message #1
└─ Assistant response
   └─ Tool call (tool_use_id)
      └─ Tool result
         └─ Follow-up assistant response
```

**2. Sub-Agent Spawning** (`agentId`, `toolUseID`)
```
User: "Run this task"
├─ Assistant calls Task tool
│  └─ Tool Invocation
│     └─ [Agent #1 spawned]
│        ├─ Agent #1 prompt
│        ├─ Agent #1 progress events
│        └─ Agent #1 response
```

**3. Background Task Chains** (`data.type: agent_progress`)
```
Progress Event: Agent started
├─ Sub-event: Bash command 1
├─ Sub-event: MCP call
└─ Progress Event: Agent completed
```

#### Design Pattern: Indented Tree Structure
```
┌─ Message Type Badge [indent 0]
│
├─ [indent 12px if has parent]
│  ├─ └─ Vertical connector line (left edge)
│  └─ Child message type badge
│
└─ [indent 24px if grandchild]
   └─ └─ └─ Grandchild with even more indent
```

**Visual Realization:**
- Parent messages: No left indent
- Child messages: 12px left indent + subtle gray vertical line
- Grandchildren: 24px indent + nested connectors
- Connectors: Subtle gray (#9CA3AF) dashed lines on left border
- Hover effect: Highlight entire thread chain

---

### **Track 4: Missing Line Types Coverage** (~3 components)
**Goal:** Queue operations, file snapshots, and summaries get dedicated UI
**Gap:** Zero visual representation

#### New Components
| Line Type | % of data | Component | Key Fields |
|-----------|-----------|-----------|-----------|
| `queue-operation` | 11.4% | MessageQueueEventCard | operation (enqueue/dequeue), timestamp, content preview |
| `file-history-snapshot` | 1.6% | FileSnapshotCard | file backups, snapshot time, file count |
| `summary` | 0.8% | SessionSummaryCard | auto-generated text, leaf UUID, searchable |

---

## 🎨 Design System (From UI/UX Pro Max)

### Color Palette (7-Type System Refined)

**Message Type Colors:**
```
user          → #93c5fd  (blue-400)         [Input]
assistant     → #fdba74  (amber-300)        [Output]
tool_use      → #d8b4fe  (purple-300)       [Action]
tool_result   → #86efac  (green-300)        [Success]
system        → #fcbf49  (yellow-400)       [Operations]
progress      → #a5b4fc  (indigo-400)       [Background Work]
summary       → #fb923c  (orange-400)       [Synthesis]
```

**Accent Colors:**
```
Primary:      #4F46E5  (indigo-600)         [Focus, Links]
Secondary:    #6366F1  (indigo-500)         [Hover States]
CTA:          #F97316  (orange-500)         [Actions, Expansions]
Background:   #EEF2FF  (indigo-50)          [Light bg]
Text:         #312E81  (indigo-900)         [Primary text]
```

**Status Colors:**
```
Error:        #EF4444  (red-500)            [Errors, Failed]
Success:      #10B981  (emerald-500)        [Completed]
Warning:      #F59E0B  (amber-500)          [Retries, Pending]
Info:         #0EA5E9  (cyan-500)           [Notifications]
```

### Typography Scale (Clarity + Readability)

```
h1  14px semibold  (#312E81)  Message type label + headers
h2  13px medium    (#475569)  Subheading in metadata
body 13px regular  (#64748B)  Main content
meta 12px mono     (#94A3B8)  Timestamps, tokens, IDs
code 12px mono     (#1E293B)  Code blocks, file paths
```

### Spacing Grid (4px Base, Aligned)

```
--space-xs   = 4px    [Tight gaps between elements]
--space-sm   = 8px    [Icon-to-label gaps]
--space-md   = 16px   [Padding within cards]
--space-lg   = 24px   [Section padding]
--space-xl   = 32px   [Large gaps between messages]
```

### Shadow Depths (OLED-Optimized)

```
--shadow-sm  = 0 1px 2px rgba(0,0,0,0.05)   [Subtle lift]
--shadow-md  = 0 4px 6px rgba(0,0,0,0.1)    [Cards, buttons]
--shadow-lg  = 0 10px 15px rgba(0,0,0,0.1)  [Modals, overlays]
```

---

## 🧩 Component Inventory

### **XML Card Components** (10 types, all semantic)

#### 1. **ObservedFromPrimarySession**
- Status: ✅ Already crafted (ToolCallCard)
- Shows: Primary action, file modified, working directory
- Collapsible: Yes

#### 2. **Observation** ✅
- Status: ✅ Already crafted
- Shows: Facts, narrative, files read
- Collapsible: Yes

#### 3. **ToolCall** ❌ → NEW
```tsx
<ToolCallCard
  name: string              // "Read", "Edit", "Bash", etc.
  input: object             // Tool parameters
  description: string       // What was attempted
  parameters?: object       // Parsed input details
  icon: React.ReactNode    // Tool icon
/>
```
- Shows: Tool name, target file/command, input summary
- Collapsible: Yes
- Color: Purple accent (#d8b4fe)

#### 4. **LocalCommand** ✅
- Status: ✅ Already crafted (terminal output)
- Shows: stdout/stderr inline
- Collapsible: No

#### 5. **TaskNotification** ✅
- Status: ✅ Already crafted (agent status)
- Shows: Task ID, status, summary, result
- Collapsible: Yes

#### 6. **Command** ✅
- Status: ✅ Already crafted (indigo command card)
- Shows: Command name, args
- Collapsible: Yes

#### 7. **ToolError** ✅
- Status: ✅ Already crafted (red error card)
- Shows: Error message + stack
- Collapsible: No

#### 8. **UntrustedData** ✅
- Status: ✅ Already crafted (amber dashed border)
- Shows: External content with warning
- Collapsible: Yes

#### 9. **ToolCallError** (Renamed from tool_use_error)
- Status: ✅ Existing
- Note: Same as ToolError

#### 10. **StructuredDataCard** ❌ → NEW (Generic fallback)
```tsx
<StructuredDataCard
  xml: string               // Raw XML content
  type: string              // For semantic class
/>
```
- Shows: Generic XML rendering (parsed + formatted, not codeblock)
- Collapsible: Yes by default (if >10 lines)
- Color: Gray with subtle borders

### **System Event Components** (5 types)

#### 1. **TurnDurationCard**
```tsx
<TurnDurationCard
  durationMs: number
  startTime?: string
  endTime?: string
/>
```
- Shows: "Turn completed in 245ms" as timing badge
- Visual: Amber bar with milliseconds + timestamp
- No collapse

#### 2. **ApiErrorCard**
```tsx
<ApiErrorCard
  error: object              // Error details
  retryAttempt: number
  maxRetries: number
  retryInMs?: number
/>
```
- Shows: Error code, message, retry count (1/3), backoff time
- Visual: Red left border, error icon, stacked layout
- Collapsible: Yes

#### 3. **CompactBoundaryCard**
```tsx
<CompactBoundaryCard
  trigger: string            // "auto" or "manual"
  preTokens: number
  postTokens?: number
/>
```
- Shows: "Context compacted: 8,000 → 4,500 tokens (auto-triggered)"
- Visual: Indigo divider line across full width
- No collapse

#### 4. **HookSummaryCard**
```tsx
<HookSummaryCard
  hookCount: number
  hookInfos: string[]
  hookErrors?: string[]
  durationMs?: number
  preventedContinuation?: boolean
/>
```
- Shows: "4 hooks executed (1 error)" with list of hooks below
- Visual: Amber left border, hook icons per command
- Collapsible: Yes

#### 5. **LocalCommandEventCard**
```tsx
<LocalCommandEventCard
  content: string            // Command description
/>
```
- Shows: Command description as single-line event
- Visual: Terminal icon, gray text
- No collapse

### **Progress Event Components** (5 types)

#### 1. **AgentProgressCard**
```tsx
<AgentProgressCard
  agentId: string
  prompt: string
  model: string
  tokens?: { input: number; output: number }
  normalizedMessages?: number
/>
```
- Shows: "Agent #1 (gpt-4) → prompt (500 tokens used)"
- Visual: Robot icon, indigo left border, nested indentation
- Collapsible: Yes (shows prompt on expand)
- Special: Indicates spawn point for sub-agent

#### 2. **BashProgressCard**
```tsx
<BashProgressCard
  command: string
  output?: string
  exitCode?: number
  duration?: number
/>
```
- Shows: "$ bash command → exit 0 (342ms)"
- Visual: Terminal icon, green if success else red
- Collapsible: Yes (shows output on expand)

#### 3. **HookProgressCard**
```tsx
<HookProgressCard
  hookEvent: string          // "SessionStart", "PreToolUse", etc.
  hookName: string
  command: string
  output?: string
/>
```
- Shows: "Hook: SessionStart → command-name"
- Visual: Hook icon, amber left border
- Collapsible: Yes

#### 4. **McpProgressCard**
```tsx
<McpProgressCard
  server: string
  method: string
  params?: object
  result?: object
/>
```
- Shows: "MCP: server.method (params)"
- Visual: Plugin icon, purple left border
- Collapsible: Yes

#### 5. **TaskQueueCard**
```tsx
<TaskQueueCard
  waitDuration?: number
  position?: number
  queueLength?: number
/>
```
- Shows: "Waiting for task... (position 3/8, 1.2s)"
- Visual: Clock icon, gray left border
- Collapsible: No

### **Message Type Components** (Updates to MessageTyped)

#### Update: **MessageTyped** (Enhanced Version)
```tsx
<MessageTyped
  message: MessageType
  messageIndex?: number
  messageType?: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress' | 'summary'
  metadata?: Record<string, any>
  parentUuid?: string        // NEW: for threading
  indent?: number            // NEW: for hierarchy (0, 12, 24, etc.)
  isChildMessage?: boolean   // NEW: show connector line
/>
```

**New Visual Features:**
- Left indent if `parentUuid` exists (12px per level)
- Dashed vertical connector line on left border (subtle gray)
- Hover effect highlights entire thread chain
- ARIA attributes for screen reader threading

---

## 📊 Queue Operation & File Snapshot Components

### **MessageQueueEventCard** (NEW)
```tsx
<MessageQueueEventCard
  operation: 'enqueue' | 'dequeue'
  timestamp: string
  content?: string          // Message preview
  queueId?: string
/>
```
- Shows: "Message enqueued at 14:32:15" or "Message processed"
- Visual: Queue icon, gray left border, minimal
- No collapse
- Use: Show queue lifecycle for debugging

### **FileSnapshotCard** (NEW)
```tsx
<FileSnapshotCard
  fileCount: number
  timestamp: string
  files: string[]           // File names under snapshot
  isIncremental: boolean
/>
```
- Shows: "4 files backed up at 14:32" with file list
- Visual: Archive icon, blue left border, collapsible list
- Collapsible: Yes (defaults collapsed if >10 files)

### **SessionSummaryCard** (NEW)
```tsx
<SessionSummaryCard
  summary: string           // Auto-generated text
  leafUuid: string          // For linking
  wordCount: number
/>
```
- Shows: "Session summary: [first 150 chars]..." as searchable card
- Visual: BookOpen icon, rose left border, expandable text
- Collapsible: Yes (shows full summary on expand)

---

## ✅ Testing & Acceptance Criteria (TDD Compliance)

**All components must pass their acceptance criteria BEFORE being considered "done".**

### **Test Execution Model**

Each component has:
1. **Unit Tests** — Component renders correctly in isolation
2. **Integration Tests** — Component works with MessageTyped parent
3. **Edge Case Tests** — Null, missing, malformed data
4. **Accessibility Tests** — ARIA, keyboard nav, focus states
5. **Manual Verification** — Browser, mobile, dark mode

**Component is DONE when:** All tests pass + manual verification passes.

---

### **XML Card Components - Acceptance Criteria**

#### **ToolCallCard** (NEW - CRITICAL)
```typescript
// Unit Tests (Must Pass)
✅ Renders tool name ("Read", "Edit", "Bash", etc.)
✅ Renders input parameters (file_path, command, pattern)
✅ Shows description of what was attempted
✅ Collapses by default (summary visible)
✅ Expands to show full details on click
✅ Handles missing parameters gracefully (no crash)
✅ Truncates very long paths (no horizontal scroll)
✅ Shows icon with aria-hidden="true" (not announced)
✅ Copy button works (navigator.clipboard)
✅ Expand button has aria-expanded attribute

// Integration Tests (Must Pass)
✅ Renders inside MessageTyped as semantic card (NOT codeblock)
✅ Inherits correct color from XML type (purple accent)
✅ Thread indent applied correctly if parent message exists
✅ Multiple ToolCallCards inline don't overlap

// Edge Cases (Must Pass)
✅ Parameters undefined → renders "No parameters"
✅ Name empty string → renders gracefully
✅ Description >500 chars → wraps without truncation
✅ Special chars in path (spaces, unicode) → escaped correctly

// Accessibility (Must Pass)
✅ Button keyboard-focusable (Tab key)
✅ Enter key expands/collapses
✅ Screen reader announces "Tool Call" + name
✅ Focus ring visible on button

// Manual Verification (Checklist)
☑ Dark mode: Text readable, icon visible, border clear
☑ Mobile 375px: No horizontal scroll, touch target >44px
☑ Chrome/Firefox/Safari: Renders identically
☑ Keyboard: Tab to button, Enter to expand
```

#### **StructuredDataCard** (NEW - Fallback)
```typescript
// Unit Tests (Must Pass)
✅ Renders generic XML without codeblock <pre> tag
✅ Parses and formats XML with syntax highlighting (if supported)
✅ Collapses if >10 lines, expands on click
✅ Shows line count indicator ("12 lines...")
✅ Handles invalid XML gracefully (shows error message)
✅ Truncates very long content

// Integration Tests (Must Pass)
✅ Used only for unknown/unsupported XML types
✅ Never used if specific card exists (ToolCall, Observation, etc.)
✅ Renders inside MessageTyped

// Edge Cases (Must Pass)
✅ Empty XML string → shows "No content"
✅ Malformed XML → shows helpful error, doesn't crash
✅ XML >1000 chars → truncated with "..." indicator

// Accessibility (Must Pass)
✅ aria-label describes content type
✅ Keyboard navigable
☑ Manual: Dark mode readable
```

#### **Observation** (Existing - Enhance)
```typescript
// Additional Tests (Must Pass for Redesign)
✅ Facts list shows first 3, collapses others
✅ Files read displayed as comma-separated list
✅ Type badge rendered correctly
✅ aria-level reflects nesting (for screen readers)

// Manual: Dark mode, mobile verified
```

#### Other 7 XML Cards (LocalCommand, TaskNotification, Command, ToolError, UntrustedData, observed_from_primary_session, hidden)
```typescript
// Each must pass:
✅ Renders with correct icon + color
✅ Collapsible if needed
✅ Accessible (ARIA, keyboard nav, focus)
✅ Dark mode verified
✅ Mobile 375px verified
```

---

### **System/Progress Event Components - Acceptance Criteria**

#### **TurnDurationCard** (NEW)
```typescript
// Unit Tests (Must Pass)
✅ Renders duration in milliseconds ("245ms")
✅ Shows timestamp if provided
✅ No collapse (always expanded)
✅ Yellow/amber color accent

// Integration Tests (Must Pass)
✅ Renders in system message container
✅ Positioned correctly in message thread

// Edge Cases (Must Pass)
✅ durationMs = 0 → shows "0ms"
✅ durationMs undefined → shows "duration unknown"

// Manual: Dark mode, timestamp readable
```

#### **ApiErrorCard** (NEW)
```typescript
// Unit Tests (Must Pass)
✅ Shows error code + message
✅ Shows retry count ("Retry 2/3")
✅ Shows backoff delay if provided
✅ Red left border (error color)
✅ Expandable to show full error stack

// Integration Tests (Must Pass)
✅ Renders in system message

// Edge Cases (Must Pass)
✅ error object empty → shows "Unknown error"
✅ retryAttempt > maxRetries → highlights warning

// Manual: Dark mode, error readable
```

#### **AgentProgressCard** (NEW - COMPLEX)
```typescript
// Unit Tests (Must Pass)
✅ Renders agent ID
✅ Shows prompt (first 100 chars + "...")
✅ Shows model (claude-opus, haiku, etc.)
✅ Calculates total tokens (input + output + cache)
✅ Shows message count if provided
✅ Robot icon present + aria-hidden="true"
✅ Indentation applied correctly

// Integration Tests (Must Pass)
✅ Renders in progress message
✅ Parent-child relationship shows indent
✅ Can be nested (grandchild indent = double)
✅ Hover highlights entire thread chain

// Edge Cases (Must Pass)
✅ tokens undefined → no token display
✅ prompt very long (>1000 chars) → truncated
✅ agentId undefined → shows generic "Sub-agent"
✅ normalizedMessages = 0 → handled gracefully

// Accessibility (Must Pass)
✅ aria-level matches nesting depth
✅ Screen reader announces "Agent task: [prompt]"
✅ Keyboard nav (Tab through threads)

// Manual: Dark mode, indent clear, hover works
```

#### **BashProgressCard, HookProgressCard, McpProgressCard, TaskQueueCard** (NEW)
```typescript
// Each must pass minimum tests:
✅ Renders title + status
✅ Shows icon with correct color
✅ Expandable (if needed)
✅ ARIA labels present
✅ Dark mode verified
✅ Keyboard navigable
```

---

### **Message Container Components - Acceptance Criteria**

#### **MessageTyped Enhanced (Threading)**
```typescript
// Unit Tests (Must Pass)
✅ Renders correct type badge (user, assistant, system, etc.)
✅ Shows timestamp
✅ Copy button works
✅ Applies left border with correct color per type
✅ Shows indent if parentUuid provided
✅ Shows dashed connector line for child messages
✅ Renders thinking block if present
✅ Renders tool calls summary if present

// Threading Tests (Must Pass - NEW)
✅ Child message indented 12px from parent
✅ Grandchild indented 24px
✅ Dashed gray line connects parent-child
✅ Hover highlights entire thread chain (all ancestors + descendants)
✅ aria-level = nesting depth

// Integration Tests (Must Pass)
✅ XML cards render correctly inside
✅ System/progress events dispatch to correct card
✅ Multiple messages in thread render with proper structure

// Edge Cases (Must Pass)
✅ content undefined → shows metadata instead (system events)
✅ parentUuid not found → renders as root message (no indent)
✅ Very deep nesting (10+ levels) → still renders correctly
✅ metadata = null → no metadata card shown

// Accessibility (Must Pass)
✅ Tab order: header → content → children
✅ Arrow keys navigate thread (if implemented)
✅ Enter expands/collapses (if collapsible)
✅ Screen reader announces message type + nesting level

// Manual: Dark mode, mobile 375px, keyboard nav
```

#### **MessageQueueEventCard** (NEW)
```typescript
// Unit Tests (Must Pass)
✅ Shows operation ("enqueued" or "processed")
✅ Shows timestamp
✅ Shows content preview (first 50 chars)
✅ Gray color (neutral)

// Manual: Dark mode, timestamp readable
```

#### **FileSnapshotCard** (NEW)
```typescript
// Unit Tests (Must Pass)
✅ Shows file count
✅ Shows timestamp
✅ Lists files (collapsible if >10)
✅ Shows incremental flag (if partial)

// Manual: Dark mode, file list readable
```

---

### **Global Acceptance Criteria (All Components)**

```typescript
// Dark Mode (CRITICAL - All Components)
✅ Text contrast ≥ 4.5:1 (readable in dark)
✅ Icons visible (not washed out)
✅ Borders visible (not too subtle)
✅ Backgrounds not pure black/white (use #0F172A, #F8FAFC)

// Mobile 375px (CRITICAL - All Components)
✅ No horizontal scroll
✅ Touch targets ≥ 44x44px
✅ Text readable (not <14px)
✅ Indentation preserved (margins reduced if needed)

// Keyboard Navigation (CRITICAL - All Components)
✅ Tab reaches all interactive elements
✅ Shift+Tab reverses focus
✅ Enter activates buttons
✅ Escape closes expandables
✅ Arrow keys navigate lists (if applicable)

// Browser Coverage (All Components)
✅ Chrome/Chromium (latest)
✅ Firefox (latest)
✅ Safari (latest, macOS)
✅ Mobile Safari (iOS latest)

// Accessibility (All Components)
✅ No aria- attributes on non-interactive <div>
✅ Use semantic HTML (<button>, <a>, not <div role="button">)
✅ Focus states visible (focus:ring-2)
✅ Icons decorated: aria-hidden="true"
✅ Icons functional: aria-label="Close"
✅ Landmark roles for major sections

// Performance (All Components)
✅ Component render <50ms
✅ No unnecessary re-renders (memo if needed)
✅ No memory leaks (cleanup in useEffect)

// Code Quality (All Components)
✅ TypeScript strict mode
✅ No any types (explicitly typed props)
✅ PropTypes or interface validation
✅ Named exports (not default)
```

---

### **Test Execution Schedule**

```
Phase 1: XML Cards (48 tests)
├─ ToolCallCard: 10 tests → DONE before implementation
├─ StructuredDataCard: 8 tests → DONE before implementation
├─ 8 existing cards: 30 tests → DONE before enhancement
└─ Manual verification → DONE day of release

Phase 2: Event Cards (45 tests)
├─ System events (5 types): 15 tests
├─ Progress events (5 types): 20 tests
└─ Manual verification → DONE day of release

Phase 3: Threading (24 tests)
├─ MessageTyped threading: 12 tests
├─ Queue/snapshot/summary: 12 tests
└─ Manual verification → DONE day of release

Integration & Edge Cases: +28 tests throughout
```

---

### **Definition of "Done" Per Component**

A component is production-ready only when:

```
✅ All unit tests pass
✅ All integration tests pass
✅ All edge case tests pass
✅ ARIA/accessibility tests pass
✅ Dark mode manual verification complete
✅ Mobile 375px manual verification complete
✅ 3 browsers tested (Chrome, Firefox, Safari)
✅ Keyboard navigation verified
✅ TypeScript strict mode passes
✅ Code review approved
✅ No console errors/warnings
```

---

### **Test Metrics Dashboard**

Track during implementation:

```
Unit Tests:           [████████░░] 45/48 passing
Integration Tests:    [██████░░░░] 20/28 passing
Edge Cases:           [████░░░░░░] 8/15 passing
Accessibility:        [████████░░] 12/15 passing
Manual Verification:  [██░░░░░░░░] 2/20 browsers
─────────────────────────────────────────────────
Overall Coverage:     [████████░░] 75% → target 80%
Blockers:             0 critical, 2 medium
Ready for Release:    ❌ (75% coverage, need 80%)
```

---

## 🔗 Conversation Threading Design

### **Visual Hierarchy Rules**

```
Message #1 (indent: 0)
├─ Left border: 4px, message type color
├─ No connector line (root)
└─ Header: Icon + Type badge + Timestamp

   └─ Message #2 (indent: 12px, parentUuid: #1)
      ├─ Left border: Dashed gray line from parent
      ├─ Connector: Subtle 1px dashed line (visual parent link)
      └─ Header: Icon + Type badge + Timestamp

      └─ Message #3 (indent: 24px, parentUuid: #2)
         ├─ Left border: Double dashed gray (nested)
         └─ Header: Icon + Type badge + Timestamp
```

### **Interactive Behaviors**

| Interaction | Behavior |
|-------------|----------|
| **Hover message** | Entire thread chain highlights (all ancestors + descendants) |
| **Click indent indicator** | Collapse/expand all children in that thread |
| **Focus (keyboard)** | Tab through messages in tree order (BFS or DFS configurable) |
| **Mobile** | Indentation reduced to 8px to save space |

### **ARIA Attributes for Accessibility**

```tsx
<MessageTyped
  role="article"
  aria-label={`${messageType} message at ${timestamp}`}
  aria-level={indentLevel}  // Heading level for hierarchy
  aria-expanded={isExpanded}
  aria-owns={childMessageIds}
  aria-describedby={`metadata-${uuid}`}
>
```

---

## 🎯 Implementation Phases

### **Phase 1: XML Card Completeness** (Critical Path)
**Effort:** ~8 hours (2 components + integration)

1. **ToolCallCard** — Dedicated UI for tool invocations
2. **StructuredDataCard** — Generic semantic XML fallback
3. Update **XmlCard** to use new components
4. Remove all codeblock fallbacks

### **Phase 2: System/Progress Event Subtypes** (High Value)
**Effort:** ~12 hours (10 components + integration)

1. Create all 5 system event components
2. Create all 5 progress event components
3. Update **MessageTyped** to dispatch to proper component
4. Add metadata field parsing from parser

### **Phase 3: Conversation Hierarchy** (UX Multiplier)
**Effort:** ~10 hours (threading logic + visual connectors)

1. Add `parentUuid` prop to message flow
2. Implement indent + connector rendering
3. Add hover thread highlighting
4. Keyboard navigation for threads
5. ARIA labels for screen readers

### **Phase 4: Queue Operations & Snapshots** (Completeness)
**Effort:** ~4 hours (3 components)

1. **MessageQueueEventCard**
2. **FileSnapshotCard**
3. **SessionSummaryCard**
4. Integration into SessionView

---

## ✅ Quality Standards (Master Craftsmanship)

### **Pre-Delivery Checklist**

- [ ] **No codeblock XML dumps** — All XML types have semantic components
- [ ] **10 system/progress subtypes rendered** — Each with purpose-built UI
- [ ] **Hierarchy visible** — Indentation + connectors show parent-child
- [ ] **All line types covered** — queue-op, snapshot, summary have UI
- [ ] **No emojis as icons** — SVG icons only (Lucide React)
- [ ] **Cursor pointer on interactive** — All clickable elements
- [ ] **Hover feedback smooth** — 150-300ms transitions
- [ ] **Focus states visible** — ARIA + keyboard nav working
- [ ] **prefers-reduced-motion respected** — No unnecessary animations
- [ ] **Grid aligned** — All measurements on 4px grid
- [ ] **Color contrast** — 4.5:1 minimum in all modes
- [ ] **Mobile responsive** — 375px+ viewport
- [ ] **Performance** — Virtual scrolling for 500+ messages

---

## 🚀 Next Steps

1. **Review this design** with team — Validate approach
2. **Approve prioritization** — Phases 1-4 or different order?
3. **Set up git worktree** — Isolated branch for implementation
4. **Create implementation plan** — Detailed step-by-step breakdown
5. **Begin Phase 1** — XML card completeness first

---

## 📚 Reference Documents

**Design & Architecture:**
- `docs/DESIGN_PHILOSOPHY_7TYPE_UI.md` — Chromatic information systems
- `docs/STYLE_GUIDE_7TYPE_UI.md` — Complete typography + spacing specs
- `design-system/claude-view-7-type-conversation-ui/MASTER.md` — Design system
- `docs/plans/archived/2026-01-29-jsonl-parser-spec.md` — Parser schema

**Implementation:**
- `src/components/MessageTyped.tsx` — Current implementation
- `src/components/XmlCard.tsx` — Current XML rendering
- `src/components/__tests__/` — Test file location

**Testing (Detailed Instructions):**
- `docs/2026-01-29-UI-TESTING-STRATEGY.md` — Full testing guide with:
  - Jest + React Testing Library setup instructions
  - 4 copy-paste test templates
  - GitHub Actions CI/CD pipeline
  - 132 test case checklist
  - Manual testing protocols

---

## 🚀 TDD Implementation Workflow

**Before implementing ANY component:**

1. **Write tests first** (following acceptance criteria in this doc)
2. **Verify tests fail** (red phase)
3. **Implement component** (green phase)
4. **Refactor if needed** (blue phase)
5. **Manual verification** (dark mode, mobile, keyboard)
6. **Mark component DONE** (all tests + manual pass)

**Example for ToolCallCard:**
```bash
# Step 1: Write tests (from acceptance criteria above)
touch src/components/__tests__/ToolCallCard.test.tsx
# ... copy test template from testing strategy ...

# Step 2: Run tests (should fail)
npm test ToolCallCard

# Step 3: Implement component
# src/components/ToolCallCard.tsx

# Step 4: Tests pass
npm test ToolCallCard -- --coverage

# Step 5: Manual verification
# Chrome dark mode ✅
# Firefox mobile ✅
# Safari ✅
# Keyboard nav ✅

# Step 6: Mark done
git commit -m "feat: add ToolCallCard with full test coverage"
```

---

## ✅ Quality Checkpoints

| Checkpoint | Criteria | Owner |
|-----------|----------|-------|
| Unit Tests | ≥80% line coverage | Jest |
| Integration Tests | All 28 tests pass | RTL |
| Manual Verification | Dark/mobile/keyboard | Developer |
| Accessibility | ARIA + semantic HTML | Developer + manual |
| Code Review | TypeScript + style | Before merge |
| Pre-Release | All manual checks | Developer |

---

**Designed with:** UI/UX Pro Max + Semantic HTML + WCAG AAA Accessibility
**Grid:** 4px base alignment + master-level craftsmanship
**Status:** TDD-Ready (tests define acceptance criteria)
**Next:** Approve → Setup Jest → Write tests → Implement components
