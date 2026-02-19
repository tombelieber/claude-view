# Action Log Tab — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a 5th "Log" tab to SessionDetailPanel that shows a filterable, developer-focused action timeline with paired tool_use/tool_result rows, timing, and expandable raw JSON.

**Architecture:** Pure frontend transform of existing WebSocket data. Extract shared message state from `RichTerminalPane` into a `useSessionMessages` hook so both Terminal and Log tabs consume the same WebSocket connection. The Log tab uses `useMemo` to transform `RichMessage[]` into `ActionItem[]` and renders via `react-virtuoso`.

**Tech Stack:** React, TypeScript, react-virtuoso (already installed), Tailwind CSS, Lucide icons

**Design doc:** `docs/plans/2026-02-19-action-log-tab-design.md`

---

### Task 1: Extract `useSessionMessages` hook

Currently `RichTerminalPane` (src/components/live/RichTerminalPane.tsx) owns the WebSocket connection and `messages` state internally. Since the Log tab needs the same messages, we extract this into a reusable hook so `SessionDetailPanel` owns the connection and both tabs share data.

**Files:**
- Create: `src/hooks/use-session-messages.ts`
- Modify: `src/components/live/RichTerminalPane.tsx`
- Modify: `src/components/live/SessionDetailPanel.tsx`

**Step 1: Create the hook**

Create `src/hooks/use-session-messages.ts`:

```typescript
import { useState, useCallback } from 'react'
import { parseRichMessage, type RichMessage } from '../components/live/RichPane'
import { useTerminalSocket, type ConnectionState } from './use-terminal-socket'

export interface UseSessionMessagesResult {
  messages: RichMessage[]
  bufferDone: boolean
  connectionState: ConnectionState
}

export function useSessionMessages(sessionId: string, enabled: boolean): UseSessionMessagesResult {
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    const parsed = parseRichMessage(data)
    if (parsed) {
      setMessages((prev) => [...prev, parsed])
    }
  }, [])

  const handleConnectionChange = useCallback((state: ConnectionState) => {
    if (state === 'connected') {
      setBufferDone(true)
    }
  }, [])

  const { connectionState } = useTerminalSocket({
    sessionId,
    mode: 'rich',
    enabled,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  return { messages, bufferDone, connectionState }
}
```

**Step 2: Simplify RichTerminalPane to accept messages as props**

Modify `src/components/live/RichTerminalPane.tsx` to accept messages from outside instead of managing its own WebSocket:

```typescript
import { RichPane, type RichMessage } from './RichPane'

interface RichTerminalPaneProps {
  messages: RichMessage[]
  bufferDone: boolean
  isVisible: boolean
  verboseMode: boolean
}

export function RichTerminalPane({ messages, bufferDone, isVisible, verboseMode }: RichTerminalPaneProps) {
  return <RichPane messages={messages} isVisible={isVisible} verboseMode={verboseMode} bufferDone={bufferDone} />
}
```

**Step 3: Update SessionDetailPanel to own the hook**

In `src/components/live/SessionDetailPanel.tsx`, import and call the hook, then pass messages to `RichTerminalPane`:

```typescript
import { useSessionMessages } from '../../hooks/use-session-messages'

// Inside the component:
const { messages: richMessages, bufferDone } = useSessionMessages(session.id, true)

// In the terminal tab render:
{activeTab === 'terminal' && (
  <RichTerminalPane
    messages={richMessages}
    bufferDone={bufferDone}
    isVisible={true}
    verboseMode={verboseMode}
  />
)}
```

**Step 4: Verify Terminal tab still works**

Run: `bun run dev` and open Mission Control, click a session, verify Terminal tab shows messages as before.

**Step 5: Check SubAgentDrillDown**

`SubAgentDrillDown` also uses `RichTerminalPane` — check if it needs updating. It likely creates its own messages for the sub-agent, so it should keep its own `useSessionMessages` call internally. If it currently uses the old `RichTerminalPane` interface, update it to use `useSessionMessages` + the new props-based `RichTerminalPane`.

Search: `grep -rn "RichTerminalPane" src/` to find all consumers.

**Step 6: Commit**

```bash
git add src/hooks/use-session-messages.ts src/components/live/RichTerminalPane.tsx src/components/live/SessionDetailPanel.tsx
git commit -m "refactor: extract useSessionMessages hook from RichTerminalPane"
```

---

### Task 2: Create `ActionItem` type and `useActionItems` transform hook

**Files:**
- Create: `src/components/live/action-log/types.ts`
- Create: `src/components/live/action-log/use-action-items.ts`

**Step 1: Define ActionItem type**

Create `src/components/live/action-log/types.ts`:

```typescript
export type ActionCategory = 'skill' | 'mcp' | 'builtin' | 'agent' | 'error'

export interface ActionItem {
  id: string                  // sequential index as string
  timestamp?: number          // Unix seconds (from RichMessage.ts)
  duration?: number           // ms between tool_use → tool_result
  category: ActionCategory
  toolName: string            // raw tool name: "Skill", "mcp__sentry__getIssues", "Edit"
  label: string               // human-readable: "Edit src/App.tsx", "npm test"
  status: 'success' | 'error' | 'pending'
  input?: string              // tool_use input JSON string
  output?: string             // tool_result content
}

export interface TurnSeparator {
  id: string
  type: 'turn'
  role: 'user' | 'assistant'
  content: string             // truncated message text
  timestamp?: number
}

export type TimelineItem = ActionItem | TurnSeparator

export function isTurnSeparator(item: TimelineItem): item is TurnSeparator {
  return 'type' in item && (item as TurnSeparator).type === 'turn'
}
```

**Step 2: Create the transform hook**

Create `src/components/live/action-log/use-action-items.ts`:

```typescript
import { useMemo } from 'react'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator, TimelineItem, ActionCategory } from './types'

function categorize(toolName: string): ActionCategory {
  if (toolName === 'Skill') return 'skill'
  if (toolName.startsWith('mcp__') || toolName.startsWith('mcp_')) return 'mcp'
  if (toolName === 'Task') return 'agent'
  return 'builtin'
}

function makeLabel(toolName: string, input?: string): string {
  if (!input) return toolName

  try {
    const parsed = JSON.parse(input)

    // File operations
    if (toolName === 'Edit' || toolName === 'Write' || toolName === 'Read') {
      const fp = parsed.file_path || parsed.path || ''
      // Show just filename + parent dir
      const parts = fp.split('/')
      const short = parts.length > 2 ? `.../${parts.slice(-2).join('/')}` : fp
      return `${toolName} ${short}`
    }

    // Bash
    if (toolName === 'Bash') {
      const cmd = parsed.command || parsed.cmd || ''
      // First line, truncated
      const firstLine = cmd.split('\n')[0]
      return firstLine.length > 60 ? firstLine.slice(0, 57) + '...' : firstLine
    }

    // Search
    if (toolName === 'Grep') {
      return `Grep "${parsed.pattern || ''}"`
    }
    if (toolName === 'Glob') {
      return `Glob ${parsed.pattern || ''}`
    }

    // Skill
    if (toolName === 'Skill') {
      return `Skill: ${parsed.skill || parsed.name || 'unknown'}`
    }

    // Task (sub-agent)
    if (toolName === 'Task') {
      const desc = parsed.description || parsed.prompt || ''
      return desc.length > 50 ? `Task: ${desc.slice(0, 47)}...` : `Task: ${desc}`
    }

    // MCP tools — show the tool name after mcp__ prefix
    if (toolName.startsWith('mcp__')) {
      const parts = toolName.split('__')
      const shortName = parts.length >= 3 ? `${parts[1]}:${parts[2]}` : toolName
      return shortName
    }

    return toolName
  } catch {
    return toolName
  }
}

export function useActionItems(messages: RichMessage[]): TimelineItem[] {
  return useMemo(() => {
    const items: TimelineItem[] = []
    let actionIndex = 0
    // Track pending tool_use items for pairing with tool_result
    const pendingToolUses: ActionItem[] = []

    for (const msg of messages) {
      // Turn separators for user/assistant messages
      if (msg.type === 'user' || msg.type === 'assistant') {
        const text = msg.content.trim()
        if (text) {
          items.push({
            id: `turn-${items.length}`,
            type: 'turn',
            role: msg.type,
            content: text.length > 100 ? text.slice(0, 97) + '...' : text,
            timestamp: msg.ts,
          } satisfies TurnSeparator)
        }
        continue
      }

      // Tool use → create pending action
      if (msg.type === 'tool_use' && msg.name) {
        const action: ActionItem = {
          id: `action-${actionIndex++}`,
          timestamp: msg.ts,
          category: categorize(msg.name),
          toolName: msg.name,
          label: makeLabel(msg.name, msg.input),
          status: 'pending',
          input: msg.input,
        }
        items.push(action)
        pendingToolUses.push(action)
        continue
      }

      // Tool result → pair with most recent pending tool_use
      if (msg.type === 'tool_result') {
        const pending = pendingToolUses.pop()
        if (pending) {
          pending.output = msg.content
          // Calculate duration if both have timestamps
          if (pending.timestamp && msg.ts) {
            pending.duration = Math.round((msg.ts - pending.timestamp) * 1000)
          }
          // Determine status from content
          const isError = msg.content.toLowerCase().includes('error') ||
                          msg.content.toLowerCase().includes('failed') ||
                          msg.content.toLowerCase().includes('exception')
          pending.status = isError ? 'error' : 'success'
        }
        continue
      }

      // Errors
      if (msg.type === 'error') {
        items.push({
          id: `action-${actionIndex++}`,
          timestamp: msg.ts,
          category: 'error',
          toolName: 'Error',
          label: msg.content.length > 60 ? msg.content.slice(0, 57) + '...' : msg.content,
          status: 'error',
          output: msg.content,
        } satisfies ActionItem)
      }
    }

    return items
  }, [messages])
}
```

**Step 3: Commit**

```bash
git add src/components/live/action-log/
git commit -m "feat: add ActionItem types and useActionItems transform hook"
```

---

### Task 3: Create `ActionFilterChips` component

**Files:**
- Create: `src/components/live/action-log/ActionFilterChips.tsx`

**Step 1: Build the component**

```tsx
import { cn } from '../../../lib/utils'
import type { ActionCategory } from './types'

const CATEGORIES: { id: ActionCategory | 'all'; label: string; color: string }[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'skill', label: 'Skill', color: 'bg-purple-500/10 text-purple-400 border-purple-500/30' },
  { id: 'mcp', label: 'MCP', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'builtin', label: 'Builtin', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'agent', label: 'Agent', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30' },
  { id: 'error', label: 'Error', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
]

interface ActionFilterChipsProps {
  counts: Record<ActionCategory, number>
  activeFilter: ActionCategory | 'all'
  onFilterChange: (filter: ActionCategory | 'all') => void
}

export function ActionFilterChips({ counts, activeFilter, onFilterChange }: ActionFilterChipsProps) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0)

  return (
    <div className="flex items-center gap-1.5 px-3 py-2 overflow-x-auto flex-shrink-0">
      {CATEGORIES.map((cat) => {
        const count = cat.id === 'all' ? total : counts[cat.id] ?? 0
        const isActive = activeFilter === cat.id
        return (
          <button
            key={cat.id}
            onClick={() => onFilterChange(cat.id)}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium border transition-colors cursor-pointer whitespace-nowrap',
              isActive ? cat.color : 'bg-transparent text-gray-500 border-gray-700 hover:border-gray-600',
            )}
          >
            {cat.label}
            {count > 0 && (
              <span className="font-mono tabular-nums">{count}</span>
            )}
          </button>
        )
      })}
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add src/components/live/action-log/ActionFilterChips.tsx
git commit -m "feat: add ActionFilterChips component"
```

---

### Task 4: Create `ActionRow` component

**Files:**
- Create: `src/components/live/action-log/ActionRow.tsx`

**Step 1: Build the component**

```tsx
import { useState, useCallback } from 'react'
import { ChevronRight, ChevronDown, Copy, Check } from 'lucide-react'
import { cn } from '../../../lib/utils'
import type { ActionItem } from './types'

// --- Category badge colors ---
const CATEGORY_BADGE: Record<string, string> = {
  skill: 'bg-purple-500/10 text-purple-400',
  mcp: 'bg-blue-500/10 text-blue-400',
  builtin: 'bg-gray-500/10 text-gray-400',
  agent: 'bg-indigo-500/10 text-indigo-400',
  error: 'bg-red-500/10 text-red-400',
}

// --- Status dot colors ---
function statusDotColor(status: ActionItem['status']): string {
  switch (status) {
    case 'success': return 'bg-green-500'
    case 'error': return 'bg-red-500'
    case 'pending': return 'bg-amber-400 animate-pulse'
  }
}

// --- Duration formatting ---
function formatDuration(ms: number | undefined): { text: string; color: string } | null {
  if (ms == null) return null
  const secs = ms / 1000
  const text = secs >= 60 ? `${(secs / 60).toFixed(1)}m` : `${secs.toFixed(1)}s`
  const color = secs > 30 ? 'text-red-400' : secs > 5 ? 'text-amber-400' : 'text-gray-500 dark:text-gray-500'
  return { text, color }
}

// --- Copy button ---
function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [text])

  return (
    <button
      onClick={handleCopy}
      className="text-gray-500 hover:text-gray-300 transition-colors p-0.5"
      title="Copy to clipboard"
    >
      {copied ? <Check className="w-3 h-3 text-green-400" /> : <Copy className="w-3 h-3" />}
    </button>
  )
}

// --- ActionRow ---
interface ActionRowProps {
  action: ActionItem
}

export function ActionRow({ action }: ActionRowProps) {
  const [expanded, setExpanded] = useState(false)
  const duration = formatDuration(action.duration)
  const badgeClass = CATEGORY_BADGE[action.category] || CATEGORY_BADGE.builtin

  // Short badge label
  const badgeLabel = action.category === 'mcp'
    ? 'mcp'
    : action.category === 'builtin'
      ? action.toolName
      : action.category === 'skill'
        ? 'Skill'
        : action.category === 'agent'
          ? 'Task'
          : 'Error'

  return (
    <div
      className={cn(
        'border-b border-gray-800/50',
        action.status === 'error' && 'bg-red-500/5',
      )}
    >
      {/* Collapsed row */}
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-800/30 transition-colors cursor-pointer"
      >
        {/* Status dot */}
        <span className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', statusDotColor(action.status))} />

        {/* Category badge */}
        <span className={cn('text-[10px] font-mono px-1.5 py-0.5 rounded flex-shrink-0 min-w-[40px] text-center', badgeClass)}>
          {badgeLabel}
        </span>

        {/* Label */}
        <span className="text-xs text-gray-300 truncate flex-1 font-mono" title={action.label}>
          {action.label}
        </span>

        {/* Duration */}
        {duration && (
          <span className={cn('text-[10px] font-mono tabular-nums flex-shrink-0', duration.color)}>
            {duration.text}
          </span>
        )}
        {action.status === 'pending' && (
          <span className="text-[10px] text-amber-400 font-mono flex-shrink-0">...</span>
        )}

        {/* Expand chevron */}
        {(action.input || action.output) && (
          expanded
            ? <ChevronDown className="w-3 h-3 text-gray-500 flex-shrink-0" />
            : <ChevronRight className="w-3 h-3 text-gray-500 flex-shrink-0" />
        )}
      </button>

      {/* Expanded detail */}
      {expanded && (
        <div className="px-3 pb-3 space-y-2">
          {action.input && (
            <div>
              <div className="flex items-center justify-between mb-1">
                <span className="text-[9px] font-medium text-gray-500 uppercase tracking-wider">Input</span>
                <CopyButton text={action.input} />
              </div>
              <pre className="text-[10px] font-mono text-gray-300 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
                {formatJson(action.input)}
              </pre>
            </div>
          )}
          {action.output && (
            <div>
              <div className="flex items-center justify-between mb-1">
                <span className="text-[9px] font-medium text-gray-500 uppercase tracking-wider">Output</span>
                <CopyButton text={action.output} />
              </div>
              <pre className="text-[10px] font-mono text-gray-300 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
                {action.output.length > 2000 ? action.output.slice(0, 2000) + '\n... (truncated)' : action.output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function formatJson(input: string): string {
  try {
    return JSON.stringify(JSON.parse(input), null, 2)
  } catch {
    return input
  }
}
```

**Step 2: Commit**

```bash
git add src/components/live/action-log/ActionRow.tsx
git commit -m "feat: add ActionRow component with expand/collapse and copy"
```

---

### Task 5: Create `TurnSeparatorRow` component

**Files:**
- Create: `src/components/live/action-log/TurnSeparatorRow.tsx`

**Step 1: Build the component**

```tsx
import { User, Bot } from 'lucide-react'

interface TurnSeparatorRowProps {
  role: 'user' | 'assistant'
  content: string
}

export function TurnSeparatorRow({ role, content }: TurnSeparatorRowProps) {
  const Icon = role === 'user' ? User : Bot
  const label = role === 'user' ? 'User' : 'Assistant'

  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      <div className="h-px flex-1 bg-gray-800" />
      <Icon className="w-3 h-3 text-gray-600 flex-shrink-0" />
      <span className="text-[10px] text-gray-600 font-medium flex-shrink-0">{label}:</span>
      <span className="text-[10px] text-gray-600 truncate max-w-[250px]" title={content}>
        {content}
      </span>
      <div className="h-px flex-1 bg-gray-800" />
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add src/components/live/action-log/TurnSeparatorRow.tsx
git commit -m "feat: add TurnSeparatorRow component"
```

---

### Task 6: Create `ActionLogTab` main component

**Files:**
- Create: `src/components/live/action-log/ActionLogTab.tsx`
- Create: `src/components/live/action-log/index.ts`

**Step 1: Build the main tab component**

Create `src/components/live/action-log/ActionLogTab.tsx`:

```tsx
import { useState, useMemo, useRef, useCallback, useEffect } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { ArrowDown } from 'lucide-react'
import type { RichMessage } from '../RichPane'
import { useActionItems } from './use-action-items'
import { ActionFilterChips } from './ActionFilterChips'
import { ActionRow } from './ActionRow'
import { TurnSeparatorRow } from './TurnSeparatorRow'
import { isTurnSeparator } from './types'
import type { ActionCategory } from './types'

interface ActionLogTabProps {
  messages: RichMessage[]
  bufferDone: boolean
}

export function ActionLogTab({ messages, bufferDone }: ActionLogTabProps) {
  const allItems = useActionItems(messages)
  const [activeFilter, setActiveFilter] = useState<ActionCategory | 'all'>('all')
  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [atBottom, setAtBottom] = useState(true)
  const [showNewIndicator, setShowNewIndicator] = useState(false)
  const prevCountRef = useRef(0)

  // Category counts (actions only, not turn separators)
  const counts = useMemo(() => {
    const c: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, error: 0 }
    for (const item of allItems) {
      if (!isTurnSeparator(item)) {
        c[item.category]++
      }
    }
    return c
  }, [allItems])

  // Filtered items
  const filteredItems = useMemo(() => {
    if (activeFilter === 'all') return allItems
    return allItems.filter((item) => {
      if (isTurnSeparator(item)) return true // always show turn separators
      return item.category === activeFilter
    })
  }, [allItems, activeFilter])

  // Show "new actions" indicator when not at bottom
  useEffect(() => {
    if (filteredItems.length > prevCountRef.current && !atBottom) {
      setShowNewIndicator(true)
    }
    prevCountRef.current = filteredItems.length
  }, [filteredItems.length, atBottom])

  // Scroll to bottom on bufferDone
  useEffect(() => {
    if (bufferDone && virtuosoRef.current) {
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({ index: filteredItems.length - 1, behavior: 'auto' })
      })
    }
  }, [bufferDone]) // intentionally exclude filteredItems.length to avoid loop

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({ index: filteredItems.length - 1, behavior: 'smooth' })
    setShowNewIndicator(false)
  }, [filteredItems.length])

  return (
    <div className="flex flex-col h-full">
      {/* Filter chips */}
      <ActionFilterChips counts={counts} activeFilter={activeFilter} onFilterChange={setActiveFilter} />

      {/* Timeline */}
      <div className="flex-1 min-h-0 relative">
        {filteredItems.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-gray-500">
            No actions yet
          </div>
        ) : (
          <Virtuoso
            ref={virtuosoRef}
            data={filteredItems}
            atBottomStateChange={setAtBottom}
            followOutput={atBottom ? 'smooth' : false}
            itemContent={(_, item) =>
              isTurnSeparator(item) ? (
                <TurnSeparatorRow role={item.role} content={item.content} />
              ) : (
                <ActionRow action={item} />
              )
            }
          />
        )}

        {/* New actions indicator */}
        {showNewIndicator && !atBottom && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-3 left-1/2 -translate-x-1/2 inline-flex items-center gap-1 px-3 py-1.5 rounded-full bg-indigo-600 text-white text-xs font-medium shadow-lg hover:bg-indigo-500 transition-colors cursor-pointer z-10"
          >
            <ArrowDown className="w-3 h-3" />
            New actions
          </button>
        )}
      </div>
    </div>
  )
}
```

**Step 2: Create barrel export**

Create `src/components/live/action-log/index.ts`:

```typescript
export { ActionLogTab } from './ActionLogTab'
```

**Step 3: Commit**

```bash
git add src/components/live/action-log/
git commit -m "feat: add ActionLogTab with virtualized timeline and filter chips"
```

---

### Task 7: Wire Log tab into SessionDetailPanel

**Files:**
- Modify: `src/components/live/SessionDetailPanel.tsx`

**Step 1: Update TabId and TABS array**

In `src/components/live/SessionDetailPanel.tsx`:

1. Add import at top:
```typescript
import { ScrollText } from 'lucide-react'
import { ActionLogTab } from './action-log'
```

2. Update `TabId` type (line 19):
```typescript
type TabId = 'overview' | 'terminal' | 'log' | 'sub-agents' | 'cost'
```

3. Add to `TABS` array (after the terminal entry, around line 32):
```typescript
const TABS: { id: TabId; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard },
  { id: 'terminal', label: 'Terminal', icon: Terminal },
  { id: 'log', label: 'Log', icon: ScrollText },
  { id: 'sub-agents', label: 'Sub-Agents', icon: Users },
  { id: 'cost', label: 'Cost', icon: DollarSign },
]
```

**Step 2: Add Log tab render block**

After the Terminal tab conditional render (after line ~424), add:

```tsx
{/* ---- Log tab ---- */}
{activeTab === 'log' && (
  <ActionLogTab messages={richMessages} bufferDone={bufferDone} />
)}
```

**Step 3: Verify end-to-end**

1. Run `bun run dev`
2. Open Mission Control
3. Click a session (side panel opens)
4. Verify 5 tabs visible: Overview, Terminal, Log, Sub-Agents, Cost
5. Click "Log" tab — verify action timeline renders with filter chips
6. Verify Terminal tab still works
7. Click through filter chips — verify filtering works
8. Click an action row — verify expand/collapse with input/output
9. Verify copy buttons work

**Step 4: Commit**

```bash
git add src/components/live/SessionDetailPanel.tsx
git commit -m "feat: wire ActionLogTab as 5th tab in SessionDetailPanel"
```

---

### Task 8: Handle edge cases and polish

**Files:**
- Modify: `src/components/live/action-log/use-action-items.ts`
- Modify: `src/components/live/action-log/ActionRow.tsx`

**Step 1: Handle tool_result error detection more robustly**

The current error detection in `use-action-items.ts` checks for "error"/"failed" in content, which may produce false positives (e.g., a tool that fixed an error). Improve by also checking if the tool_result includes standard error markers from Claude Code:

```typescript
// In the tool_result handling section of useActionItems:
const isError = msg.content.startsWith('Error:') ||
                msg.content.startsWith('FAILED') ||
                msg.content.includes('exit code') ||
                msg.content.includes('Command failed')
```

**Step 2: Add timestamp display to expanded rows**

In `ActionRow.tsx`, show the timestamp when expanded:

```tsx
// Inside the expanded detail section, before input:
{action.timestamp && action.timestamp > 0 && (
  <div className="text-[9px] text-gray-600 font-mono mb-1">
    {new Date(action.timestamp * 1000).toLocaleTimeString()}
  </div>
)}
```

**Step 3: Commit**

```bash
git add src/components/live/action-log/
git commit -m "fix: improve error detection and add timestamp to expanded rows"
```

---

### Task 9: Final integration verification

**No code changes — just testing.**

**Step 1: Test with a real active session**

1. Start a Claude Code session in a terminal
2. Give it a task that uses multiple tools (e.g., "find all TODO comments in this codebase and list them")
3. Open Mission Control → click the session → click "Log" tab
4. Verify:
   - Actions appear in real-time as the agent works
   - Filter chips update counts live
   - Turn separators show user prompts between action groups
   - Expanding a row shows input/output with copy buttons
   - Duration badges appear after tool_result arrives
   - Error rows have red tint
   - Auto-scroll works when at bottom
   - "New actions" pill appears when scrolled up

**Step 2: Test with MCP tools**

If any MCP servers are configured, verify:
- MCP tool calls show as `mcp` category
- Badge shows the server:tool format
- Input/output can be inspected

**Step 3: Test with Skill invocations**

Run a skill (e.g., `/commit`) and verify:
- Skill calls show as `skill` category
- Label shows "Skill: commit" or similar
