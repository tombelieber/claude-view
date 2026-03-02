# ChatInputBar + Interactive Cards — Implementation Plan

> **Status:** DONE (2026-03-02) — all 24 tasks implemented, shippable audit passed (SHIP IT)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a shared chat input bar matching the Claude Code VSCode extension UX, plus 4 interactive message cards, and wire them into DashboardChat.

**Architecture:** Plain `<textarea>` with auto-grow + Radix UI Popover for slash commands + Lucide icons. Interactive cards rendered inline in the chat message stream. One new dependency: `@radix-ui/react-alert-dialog` (see Task 0).

**Tech Stack:** React 19, Radix UI (`react-popover`, `react-tooltip`, `react-alert-dialog`), Lucide React, Tailwind CSS, Vitest

**Design doc:** `docs/plans/2026-02-28-chat-input-bar-design.md`

### Completion Summary

| Task(s) | Commit | Description |
|---------|--------|-------------|
| 0-7 | `7980aeb6` | ChatInputBar sub-components (commands, gauge, cost, mode, model, attach, slash popover) |
| 8 | `04c71ac8` | ChatInputBar main component with dormant state machine |
| 9-14 | `f4e13edc` | Interactive card components (shell, question, permission, plan, elicitation) |
| 15-16.5 | `2cd3c1f1` | ControlCallbacks plumbing, TakeoverConfirmDialog, LiveSession controlId |
| 17-19 | `3ff59389` | Wire into RichPane, MonitorPane slots, SessionDetailPanel |
| 20-22 | `b1cea46d` | ConversationView resume, NewSessionInput, DashboardChat deletion |
| Audit fixes | `f8aba7d4` | Missing dependency commit, error handling, stable React keys |

Shippable audit: tsc 0 errors, build 7/7, tests 1140/1142 (2 pre-existing), 0 blockers, 12 warnings (all pre-existing).

---

## Task 0: Install Missing Package

**Files:**
- Modify: `apps/web/package.json` (via bun add)

**Why first:** `@radix-ui/react-alert-dialog` is NOT in `apps/web/package.json`. Task 16
(`TakeoverConfirmDialog`) imports from it and will fail to compile without it. Confirmed absent
by inspecting `apps/web/package.json` — only `react-dialog`, `react-popover`, `react-tabs`, and
`react-tooltip` are present.

**Step 1: Install the package**

Run: `cd apps/web && bun add @radix-ui/react-alert-dialog`

**Step 2: Verify import resolves**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -5`
Expected: 0 new errors (same baseline as before Task 0)

**Step 3: Commit**

```bash
git add apps/web/package.json bun.lock
git commit -m "chore(web): add @radix-ui/react-alert-dialog dependency"
```

---

## Task 1: Slash Command Data

**Files:**
- Create: `apps/web/src/components/chat/commands.ts`
- Test: `apps/web/src/components/chat/commands.test.ts`

**Step 1: Write the test**

```ts
// commands.test.ts
import { describe, expect, it } from 'vitest'
import { COMMANDS, filterCommands } from './commands'

describe('commands', () => {
  it('exports at least 10 commands', () => {
    expect(COMMANDS.length).toBeGreaterThanOrEqual(10)
  })

  it('every command has name, description, and category', () => {
    for (const cmd of COMMANDS) {
      expect(cmd.name).toBeTruthy()
      expect(cmd.description).toBeTruthy()
      expect(cmd.category).toBeTruthy()
    }
  })

  it('filters by prefix', () => {
    const results = filterCommands('com')
    expect(results.every((c) => c.name.includes('com'))).toBe(true)
  })

  it('returns all commands for empty query', () => {
    expect(filterCommands('')).toEqual(COMMANDS)
  })
})
```

**Step 2: Run test to verify it fails**

Run: `cd apps/web && bunx vitest run src/components/chat/commands.test.ts`
Expected: FAIL — module not found

**Step 3: Write the implementation**

```ts
// commands.ts
export interface SlashCommand {
  name: string
  description: string
  category: 'mode' | 'session' | 'action' | 'info'
}

export const COMMANDS: SlashCommand[] = [
  // Mode commands
  { name: 'plan', description: 'Enter plan mode', category: 'mode' },
  { name: 'code', description: 'Enter code mode', category: 'mode' },
  { name: 'ask', description: 'Enter ask mode', category: 'mode' },
  // Session commands
  { name: 'compact', description: 'Compact conversation context', category: 'session' },
  { name: 'clear', description: 'Clear conversation', category: 'session' },
  // Action commands
  { name: 'review', description: 'Review recent changes', category: 'action' },
  { name: 'commit', description: 'Create a git commit', category: 'action' },
  { name: 'test', description: 'Run tests', category: 'action' },
  { name: 'debug', description: 'Start debugging', category: 'action' },
  { name: 'deploy', description: 'Deploy the project', category: 'action' },
  // Info commands
  { name: 'help', description: 'Get help with Claude', category: 'info' },
  { name: 'cost', description: 'Show session cost breakdown', category: 'info' },
  { name: 'status', description: 'Show session status', category: 'info' },
  { name: 'context', description: 'Show context window usage', category: 'info' },
]

export function filterCommands(query: string): SlashCommand[] {
  if (!query) return COMMANDS
  const lower = query.toLowerCase()
  return COMMANDS.filter(
    (c) => c.name.includes(lower) || c.description.toLowerCase().includes(lower),
  )
}
```

**Step 4: Run test to verify it passes**

Run: `cd apps/web && bunx vitest run src/components/chat/commands.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add apps/web/src/components/chat/commands.ts apps/web/src/components/chat/commands.test.ts
git commit -m "feat(chat): add slash command data and filter"
```

---

## Task 2: ChatContextGauge Sub-component

> **Audit fix B7:** Renamed from `ContextGauge` → `ChatContextGauge`. An existing
> `apps/web/src/components/live/ContextGauge.tsx` has a completely different API
> (`contextWindowTokens`, `model`, `group`). Same component name in two directories causes
> auto-import confusion and DevTools ambiguity.

**Files:**
- Create: `apps/web/src/components/chat/ChatContextGauge.tsx`

**Step 1: Write the component**

```tsx
// ChatContextGauge.tsx
import * as Tooltip from '@radix-ui/react-tooltip'

interface ChatContextGaugeProps {
  percent: number // 0-100
}

export function ChatContextGauge({ percent }: ChatContextGaugeProps) {
  const clamped = Math.min(Math.max(percent, 0), 100)
  const color =
    clamped > 80 ? 'bg-red-500' : clamped > 60 ? 'bg-amber-500' : 'bg-blue-500'

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <div className="flex items-center gap-1.5 cursor-default">
            <div className="w-16 h-1.5 bg-gray-700 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all duration-200 ${color}`}
                style={{ width: `${clamped}%` }}
              />
            </div>
            <span className="text-[10px] tabular-nums font-mono text-gray-400">
              {Math.round(clamped)}%
            </span>
          </div>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            className="rounded px-2 py-1 text-xs bg-gray-800 text-gray-200 border border-gray-700"
            sideOffset={5}
          >
            Context window: {Math.round(clamped)}% used
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
```

**Step 2: Verify** — no unit test needed for pure display. Verified during Task 8 integration.

**Step 3: Commit**

```bash
git add apps/web/src/components/chat/ChatContextGauge.tsx
git commit -m "feat(chat): add ChatContextGauge sub-component"
```

---

## Task 3: CostPreview Sub-component

**Files:**
- Create: `apps/web/src/components/chat/CostPreview.tsx`

**Step 1: Write the component**

```tsx
// CostPreview.tsx
import * as Tooltip from '@radix-ui/react-tooltip'

interface CostPreviewProps {
  cached: number
  uncached: number
}

export function CostPreview({ cached, uncached }: CostPreviewProps) {
  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <span className="text-[10px] tabular-nums font-mono text-gray-400 cursor-default">
            ~${cached.toFixed(2)}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            className="rounded px-2 py-1 text-xs bg-gray-800 text-gray-200 border border-gray-700"
            sideOffset={5}
          >
            ~${cached.toFixed(2)} cached / ~${uncached.toFixed(2)} uncached
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/CostPreview.tsx
git commit -m "feat(chat): add CostPreview sub-component"
```

---

## Task 4: ModeSwitch Sub-component

**Files:**
- Create: `apps/web/src/components/chat/ModeSwitch.tsx`

**Step 1: Write the component**

Uses Radix `Popover` (already installed) — not `DropdownMenu` (would need new dep).

```tsx
// ModeSwitch.tsx
import * as Popover from '@radix-ui/react-popover'
import { useState } from 'react'

type Mode = 'plan' | 'code' | 'ask'

interface ModeSwitchProps {
  mode: Mode
  onModeChange: (mode: Mode) => void
  disabled?: boolean
}

const MODE_CONFIG: Record<Mode, { label: string; color: string; activeColor: string }> = {
  plan: { label: 'Plan', color: 'text-amber-400', activeColor: 'bg-amber-500/20 border-amber-500/40' },
  code: { label: 'Code', color: 'text-green-400', activeColor: 'bg-green-500/20 border-green-500/40' },
  ask: { label: 'Ask', color: 'text-blue-400', activeColor: 'bg-blue-500/20 border-blue-500/40' },
}

export function ModeSwitch({ mode, onModeChange, disabled }: ModeSwitchProps) {
  const [open, setOpen] = useState(false)
  const config = MODE_CONFIG[mode]

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={`inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded border cursor-pointer transition-colors ${config.activeColor} ${config.color} disabled:opacity-50`}
        >
          {config.label}
          <span className="text-[8px]">&#x25BE;</span>
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="bg-gray-900 border border-gray-700 rounded-lg shadow-lg py-1 min-w-[100px] z-50"
          sideOffset={4}
          align="start"
        >
          {(Object.keys(MODE_CONFIG) as Mode[]).map((m) => (
            <button
              type="button"
              key={m}
              onClick={() => { onModeChange(m); setOpen(false) }}
              className={`w-full text-left px-3 py-1.5 text-xs hover:bg-gray-800 cursor-pointer ${
                m === mode ? MODE_CONFIG[m].color + ' font-medium' : 'text-gray-300'
              }`}
            >
              {MODE_CONFIG[m].label}
            </button>
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/ModeSwitch.tsx
git commit -m "feat(chat): add ModeSwitch sub-component"
```

---

## Task 5: ModelSelector Sub-component

**Files:**
- Create: `apps/web/src/components/chat/ModelSelector.tsx`

**Step 1: Write the component**

```tsx
// ModelSelector.tsx
import * as Popover from '@radix-ui/react-popover'
import { useState } from 'react'

interface ModelOption {
  id: string
  label: string
}

interface ModelSelectorProps {
  model: string
  onModelChange: (model: string) => void
  models?: ModelOption[]
  disabled?: boolean
}

const DEFAULT_MODELS: ModelOption[] = [
  { id: 'claude-opus-4-6', label: 'Opus 4.6' },
  { id: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
  { id: 'claude-haiku-4-5', label: 'Haiku 4.5' },
]

export function ModelSelector({ model, onModelChange, models, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false)
  const options = models ?? DEFAULT_MODELS
  const current = options.find((m) => m.id === model) ?? options[0]

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          disabled={disabled}
          className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] font-mono text-gray-400 rounded border border-gray-700 hover:border-gray-600 cursor-pointer transition-colors disabled:opacity-50"
        >
          {current.label}
          <span className="text-[8px]">&#x25BE;</span>
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="bg-gray-900 border border-gray-700 rounded-lg shadow-lg py-1 min-w-[120px] z-50"
          sideOffset={4}
          align="end"
        >
          {options.map((m) => (
            <button
              type="button"
              key={m.id}
              onClick={() => { onModelChange(m.id); setOpen(false) }}
              className={`w-full text-left px-3 py-1.5 text-xs font-mono hover:bg-gray-800 cursor-pointer ${
                m.id === model ? 'text-blue-400 font-medium' : 'text-gray-300'
              }`}
            >
              {m.label}
            </button>
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/ModelSelector.tsx
git commit -m "feat(chat): add ModelSelector sub-component"
```

---

## Task 6: AttachButton Sub-component

**Files:**
- Create: `apps/web/src/components/chat/AttachButton.tsx`

**Step 1: Write the component**

```tsx
// AttachButton.tsx
import { Paperclip, X } from 'lucide-react'
import { useRef } from 'react'

interface AttachButtonProps {
  onAttach: (files: File[]) => void
  disabled?: boolean
}

const ACCEPT = 'image/*,.txt,.md,.json,.ts,.tsx,.js,.jsx,.rs,.py,.css,.html'

/** Audit fix B11: Removed unused `attachments` and `onRemove` props — parent renders
 *  `<AttachmentChips>` separately. With `noUnusedParameters: true`, unused props cause tsc errors. */
export function AttachButton({ onAttach, disabled }: AttachButtonProps) {
  const inputRef = useRef<HTMLInputElement>(null)

  const handleClick = () => inputRef.current?.click()

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files?.length) {
      onAttach(Array.from(e.target.files))
      e.target.value = '' // reset so same file can be picked again
    }
  }

  return (
    <>
      <button
        type="button"
        onClick={handleClick}
        disabled={disabled}
        className="p-1.5 text-gray-400 hover:text-gray-200 rounded transition-colors cursor-pointer disabled:opacity-50"
        aria-label="Attach file"
      >
        <Paperclip className="w-4 h-4" />
      </button>
      <input
        ref={inputRef}
        type="file"
        multiple
        accept={ACCEPT}
        onChange={handleChange}
        className="hidden"
      />
      {/* Attachment chips rendered by parent — this component just provides the button + file input */}
    </>
  )
}

/** Render attachment chips (used by ChatInputBar) */
export function AttachmentChips({
  attachments,
  onRemove,
}: { attachments: File[]; onRemove: (index: number) => void }) {
  if (attachments.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1 px-3 pb-1">
      {attachments.map((file, i) => (
        <span
          key={`${file.name}-${i}`}
          className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] bg-gray-800 text-gray-300 rounded border border-gray-700"
        >
          {file.name}
          <button
            type="button"
            onClick={() => onRemove(i)}
            className="text-gray-500 hover:text-gray-300 cursor-pointer"
            aria-label={`Remove ${file.name}`}
          >
            <X className="w-3 h-3" />
          </button>
        </span>
      ))}
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/AttachButton.tsx
git commit -m "feat(chat): add AttachButton sub-component"
```

---

## Task 7: SlashCommandPopover Sub-component

**Files:**
- Create: `apps/web/src/components/chat/SlashCommandPopover.tsx`

This is the most complex sub-component. Keyboard navigation (Up/Down/Enter/Escape) and position tracking.

**Step 1: Write the component**

```tsx
// SlashCommandPopover.tsx
import { useCallback, useEffect, useRef, useState } from 'react'
import type { SlashCommand } from './commands'
import { filterCommands } from './commands'

interface SlashCommandPopoverProps {
  input: string                          // current textarea value
  open: boolean
  onSelect: (command: SlashCommand) => void
  onClose: () => void
  commands?: SlashCommand[]              // override default list
  anchorRef: React.RefObject<HTMLTextAreaElement | null>
}

export function SlashCommandPopover({
  input,
  open,
  onSelect,
  onClose,
  commands,
  anchorRef,
}: SlashCommandPopoverProps) {
  const [activeIndex, setActiveIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // Filter commands based on input after "/"
  const query = input.startsWith('/') ? input.slice(1) : ''
  const filtered = commands ? filterFromList(commands, query) : filterCommands(query)

  // Reset active index when filter changes
  useEffect(() => {
    setActiveIndex(0)
  }, [query])

  // Scroll active item into view
  useEffect(() => {
    if (!listRef.current) return
    const items = listRef.current.children
    if (items[activeIndex]) {
      (items[activeIndex] as HTMLElement).scrollIntoView({ block: 'nearest' })
    }
  }, [activeIndex])

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!open) return
      switch (e.key) {
        case 'ArrowUp':
          e.preventDefault()
          setActiveIndex((prev) => (prev > 0 ? prev - 1 : filtered.length - 1))
          break
        case 'ArrowDown':
          e.preventDefault()
          setActiveIndex((prev) => (prev < filtered.length - 1 ? prev + 1 : 0))
          break
        case 'Enter':
          e.preventDefault()
          if (filtered[activeIndex]) onSelect(filtered[activeIndex])
          break
        case 'Escape':
          e.preventDefault()
          onClose()
          break
      }
    },
    [open, filtered, activeIndex, onSelect, onClose],
  )

  // Attach keyboard listener to textarea
  useEffect(() => {
    const textarea = anchorRef.current
    if (!textarea || !open) return
    textarea.addEventListener('keydown', handleKeyDown)
    return () => textarea.removeEventListener('keydown', handleKeyDown)
  }, [anchorRef, open, handleKeyDown])

  if (!open || filtered.length === 0) return null

  return (
    <div
      className="absolute bottom-full left-0 right-0 mb-1 bg-gray-900 border border-gray-700 rounded-lg shadow-lg max-h-[240px] overflow-y-auto z-50"
      ref={listRef}
    >
      {filtered.map((cmd, i) => (
        <button
          type="button"
          key={cmd.name}
          onMouseEnter={() => setActiveIndex(i)}
          onClick={() => onSelect(cmd)}
          className={`w-full text-left px-3 py-2 flex items-center gap-3 cursor-pointer ${
            i === activeIndex ? 'bg-gray-800' : ''
          }`}
        >
          <span className="text-sm font-mono text-blue-400">/{cmd.name}</span>
          <span className="text-xs text-gray-400 truncate">{cmd.description}</span>
        </button>
      ))}
    </div>
  )
}

function filterFromList(commands: SlashCommand[], query: string): SlashCommand[] {
  if (!query) return commands
  const lower = query.toLowerCase()
  return commands.filter(
    (c) => c.name.includes(lower) || c.description.toLowerCase().includes(lower),
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/SlashCommandPopover.tsx
git commit -m "feat(chat): add SlashCommandPopover with keyboard navigation"
```

---

## Task 8: ChatInputBar Main Component

**Files:**
- Create: `apps/web/src/components/chat/ChatInputBar.tsx`
- Create: `apps/web/src/components/chat/index.ts`

**Step 1: Write ChatInputBar**

This composes all sub-components. The auto-growing textarea, slash command trigger, paste handler, and send/stop logic all live here.

```tsx
// ChatInputBar.tsx
import { ArrowUp, Square } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { AttachButton, AttachmentChips } from './AttachButton'
import { ChatContextGauge } from './ChatContextGauge'
import { CostPreview } from './CostPreview'
import { ModeSwitch } from './ModeSwitch'
import { ModelSelector } from './ModelSelector'
import { SlashCommandPopover } from './SlashCommandPopover'
import { COMMANDS } from './commands'
import type { SlashCommand } from './commands'

type Mode = 'plan' | 'code' | 'ask'

/** Dormant state machine: dormant → resuming → active → streaming → completed
 *
 * Mapping from useControlSession's ControlStatus:
 *   'connecting' | 'reconnecting'  → 'resuming'     (disabled, spinner implied)
 *   'active' | 'waiting_input'     → 'active'
 *   'waiting_permission'           → 'streaming'    (disable input while awaiting permission)
 *   'completed'                    → 'completed'
 *   'error' | 'disconnected'       → 'dormant'      (allow retry)
 */
/** Audit fix B6: exported so Task 18 can import it.
 *  Audit fix B12: added 'waiting_permission' — distinct from 'streaming' because the
 *  user sees a PermissionCard and should know Claude is paused, not responding. */
export type InputBarState =
  | 'dormant'
  | 'resuming'
  | 'active'
  | 'streaming'
  | 'waiting_permission'
  | 'completed'
  | 'controlled_elsewhere'
  | 'connecting'     // direct from ControlStatus when not yet mapped by parent
  | 'reconnecting'   // direct from ControlStatus when not yet mapped by parent

interface ChatInputBarProps {
  onSend: (message: string) => void
  onStop?: () => void
  /** Current state in the dormant → active lifecycle */
  state?: InputBarState
  placeholder?: string
  // Mode
  mode?: Mode
  onModeChange?: (mode: Mode) => void
  // Model
  model?: string
  onModelChange?: (model: string) => void
  // Context & cost
  // AUDIT FIX B8 (resolved): useControlSession returns contextUsage as 0–100 percent
  // (confirmed: ChatStatusBar.tsx line 2 documents "0-100 percentage", DashboardChat.tsx
  // line 70 passes it directly without multiplication). Pass directly — do NOT multiply.
  contextPercent?: number
  estimatedCost?: { cached: number; uncached: number } | null
  // Commands
  commands?: SlashCommand[]
  onCommand?: (command: string) => void
}

const STATE_CONFIG: Record<InputBarState, { placeholder: string; disabled: boolean; muted: boolean }> = {
  dormant:              { placeholder: 'Resume this session...', disabled: false, muted: true },
  resuming:             { placeholder: 'Resuming session...', disabled: true, muted: true },
  connecting:           { placeholder: 'Connecting...', disabled: true, muted: true },
  reconnecting:         { placeholder: 'Reconnecting...', disabled: true, muted: true },
  active:               { placeholder: 'Send a message... (or type / for commands)', disabled: false, muted: false },
  streaming:            { placeholder: 'Claude is responding...', disabled: true, muted: false },
  waiting_permission:   { placeholder: 'Waiting for permission response...', disabled: true, muted: false },
  completed:            { placeholder: 'Session ended', disabled: true, muted: true },
  controlled_elsewhere: { placeholder: 'Controlled in another tab', disabled: true, muted: true },
}

export function ChatInputBar({
  onSend,
  onStop,
  state = 'active',
  placeholder: customPlaceholder,
  mode = 'code',
  onModeChange,
  model = 'claude-sonnet-4-6',
  onModelChange,
  contextPercent,
  estimatedCost,
  commands,
  onCommand,
}: ChatInputBarProps) {
  const config = STATE_CONFIG[state]
  const isStreaming = state === 'streaming'
  const disabled = config.disabled
  const placeholder = customPlaceholder ?? config.placeholder
  const [input, setInput] = useState('')
  const [attachments, setAttachments] = useState<File[]>([])
  const [slashOpen, setSlashOpen] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Auto-grow textarea
  const handleInput = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value
    setInput(value)
    // Open slash popover when input starts with "/"
    setSlashOpen(value.startsWith('/'))
    // Auto-grow
    const el = e.target
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`
  }, [])

  const handleSend = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed) return
    onSend(trimmed)
    setInput('')
    setAttachments([])
    setSlashOpen(false)
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto'
    }
  }, [input, onSend])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Don't intercept if slash popover is handling keys
      if (slashOpen) return
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      }
      if (e.key === 'Escape' && isStreaming && onStop) {
        onStop()
      }
    },
    [handleSend, slashOpen, isStreaming, onStop],
  )

  const handleSlashSelect = useCallback(
    (cmd: SlashCommand) => {
      setSlashOpen(false)
      setInput('')
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto'
        textareaRef.current.focus()
      }
      // Mode commands change mode directly
      if (cmd.name === 'plan' || cmd.name === 'code' || cmd.name === 'ask') {
        onModeChange?.(cmd.name)
      } else {
        onCommand?.(cmd.name)
      }
    },
    [onModeChange, onCommand],
  )

  // Image paste handler
  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    const items = e.clipboardData.items
    const imageFiles: File[] = []
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile()
        if (file) imageFiles.push(file)
      }
    }
    if (imageFiles.length > 0) {
      setAttachments((prev) => [...prev, ...imageFiles])
    }
  }, [])

  const handleAttach = useCallback((files: File[]) => {
    setAttachments((prev) => [...prev, ...files])
  }, [])

  const handleRemoveAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index))
  }, [])

  const canSend = input.trim().length > 0 && !disabled

  return (
    <div className={`relative border-t px-4 py-3 transition-colors ${
      config.muted ? 'border-gray-800/50 bg-gray-950/50' : 'border-gray-800 bg-gray-950'
    }`}>
      {/* Slash command popover (positioned above) */}
      <SlashCommandPopover
        input={input}
        open={slashOpen}
        onSelect={handleSlashSelect}
        onClose={() => setSlashOpen(false)}
        commands={commands ?? COMMANDS}
        anchorRef={textareaRef}
      />

      {/* Main input chrome */}
      <div className="rounded-xl border border-gray-700 bg-gray-900 overflow-hidden focus-within:border-gray-600 transition-colors">
        {/* Top bar: mode + model */}
        <div className="flex items-center justify-between px-3 pt-2">
          {onModeChange ? (
            <ModeSwitch mode={mode} onModeChange={onModeChange} disabled={disabled || isStreaming} />
          ) : (
            <div />
          )}
          {onModelChange ? (
            <ModelSelector model={model} onModelChange={onModelChange} disabled={disabled || isStreaming} />
          ) : (
            <div />
          )}
        </div>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          disabled={disabled}
          placeholder={disabled ? 'Session ended' : placeholder}
          rows={1}
          className="w-full resize-none bg-transparent px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:outline-none disabled:opacity-50 motion-reduce:transition-none"
          style={{ maxHeight: '200px' }}
          aria-label="Chat message"
        />

        {/* Attachment chips */}
        <AttachmentChips attachments={attachments} onRemove={handleRemoveAttachment} />

        {/* Bottom bar: attach + gauge + cost + send */}
        <div className="flex items-center justify-between px-3 pb-2">
          <div className="flex items-center gap-3">
            <AttachButton
              onAttach={handleAttach}
              disabled={disabled}
            />
            {contextPercent != null && <ChatContextGauge percent={contextPercent} />}
            {estimatedCost && (
              <CostPreview cached={estimatedCost.cached} uncached={estimatedCost.uncached} />
            )}
          </div>

          {/* Send / Stop button */}
          {isStreaming ? (
            <button
              type="button"
              onClick={onStop}
              className="p-1.5 rounded-lg bg-red-600 hover:bg-red-700 text-white transition-colors cursor-pointer"
              aria-label="Stop generation"
            >
              <Square className="w-4 h-4" />
            </button>
          ) : (
            <button
              type="button"
              onClick={handleSend}
              disabled={!canSend}
              className="p-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
              aria-label="Send message"
            >
              <ArrowUp className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
```

**Step 2: Write barrel export**

```ts
// index.ts
export { ChatInputBar } from './ChatInputBar'
export type { SlashCommand } from './commands'
export { COMMANDS, filterCommands } from './commands'
```

**Step 3: Type-check**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

**Step 4: Commit**

```bash
git add apps/web/src/components/chat/ChatInputBar.tsx apps/web/src/components/chat/index.ts
git commit -m "feat(chat): add ChatInputBar main component composing all sub-components"
```

---

## Task 9: InteractiveCardShell (shared wrapper)

**Files:**

- Create: `apps/web/src/components/chat/cards/InteractiveCardShell.tsx`

**Step 1: Write the component**

Shared wrapper for all 4 interactive card types. Provides consistent:

- Color-coded header with icon + type badge
- Content area (children)
- Action bar (children)
- Resolved state: `opacity-60`, `pointer-events-none`, result badge in header

```tsx
// InteractiveCardShell.tsx
import type { ReactNode } from 'react'

type CardVariant = 'question' | 'permission' | 'plan' | 'elicitation'

interface InteractiveCardShellProps {
  variant: CardVariant
  header: string
  icon?: ReactNode
  resolved?: { label: string; variant: 'success' | 'denied' | 'neutral' }
  children: ReactNode
  actions?: ReactNode
}

const VARIANT_COLORS: Record<CardVariant, {
  border: string; bg: string; headerBg: string; headerText: string; icon: string
}> = {
  question: {
    border: 'border-purple-500/20',
    bg: 'bg-purple-900/10',
    headerBg: 'bg-purple-900/20',
    headerText: 'text-purple-400',
    icon: 'text-purple-400',
  },
  permission: {
    border: 'border-amber-500/20',
    bg: 'bg-amber-900/10',
    headerBg: 'bg-amber-900/20',
    headerText: 'text-amber-400',
    icon: 'text-amber-400',
  },
  plan: {
    border: 'border-blue-500/20',
    bg: 'bg-blue-900/10',
    headerBg: 'bg-blue-900/20',
    headerText: 'text-blue-400',
    icon: 'text-blue-400',
  },
  elicitation: {
    border: 'border-gray-700/50',
    bg: 'bg-gray-800/30',
    headerBg: 'bg-gray-800/40',
    headerText: 'text-gray-400',
    icon: 'text-gray-400',
  },
}

const RESOLVED_BADGE: Record<string, string> = {
  success: 'bg-green-500/20 text-green-400 border-green-500/30',
  denied: 'bg-red-500/20 text-red-400 border-red-500/30',
  neutral: 'bg-gray-500/20 text-gray-400 border-gray-500/30',
}

export function InteractiveCardShell({
  variant,
  header,
  icon,
  resolved,
  children,
  actions,
}: InteractiveCardShellProps) {
  const c = VARIANT_COLORS[variant]
  const isResolved = !!resolved

  return (
    <div
      className={`rounded-lg border ${c.border} ${c.bg} overflow-hidden transition-opacity duration-200 motion-reduce:transition-none ${
        isResolved ? 'opacity-60' : ''
      }`}
    >
      {/* Header */}
      <div className={`px-3 py-2 border-b ${c.border} ${c.headerBg} flex items-center gap-2`}>
        {icon && <span className={`${c.icon} flex-shrink-0`}>{icon}</span>}
        <span className={`text-[10px] font-mono ${c.headerText} uppercase tracking-wide`}>
          {header}
        </span>
        <div className="flex-1" />
        {resolved && (
          <span
            className={`text-[10px] px-1.5 py-0.5 rounded border ${RESOLVED_BADGE[resolved.variant]}`}
          >
            {resolved.label}
          </span>
        )}
      </div>

      {/* Content */}
      <div className={isResolved ? 'pointer-events-none' : ''}>
        {children}
      </div>

      {/* Action bar */}
      {actions && !isResolved && (
        <div className="flex items-center justify-end gap-2 px-3 py-2 border-t border-gray-700/30">
          {actions}
        </div>
      )}
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/InteractiveCardShell.tsx
git commit -m "feat(chat): add InteractiveCardShell shared wrapper"
```

---

## Task 10: AskUserQuestionCard (evolve existing display)

**Files:**

- Create: `apps/web/src/components/chat/cards/AskUserQuestionCard.tsx`
- Reference: `apps/web/src/components/live/AskUserQuestionDisplay.tsx` (existing, display-only)

**Step 1: Write the component**

Builds on the visual pattern from `AskUserQuestionDisplay` but adds interactivity. Uses `InteractiveCardShell` for consistent wrapper.

**Important — do NOT import `COLORS` from `AskUserQuestionDisplay`:**
The `COLORS` constant in `AskUserQuestionDisplay.tsx` is NOT exported (module-private). All styling
for `AskUserQuestionCard` comes from `InteractiveCardShell`'s `variant="question"` (purple color).
Do not attempt to re-use or import `COLORS` from the display component.

**Schema note:** `AskUserQuestionDisplay` uses `multiple` (boolean) for multi-select; the WebSocket
`AskUserQuestionMsg` type uses `multiSelect`. Both refer to the same concept. The card's `questions`
prop uses `multiSelect` to match the WS protocol. For display-only fallback, map `multiSelect → multiple`.

Key features:

- Single-select: radio buttons (controlled via `useState`)
- Multi-select: checkboxes (controlled via `useState<Set<number>>`)
- Optional markdown preview: shown to the right when any option has `markdown` field
- "Other" option: text input at bottom of option list
- Submit button: calls `onAnswer` with the selected option label(s)
- After submission: card resolves via `InteractiveCardShell` (grayed out, badge shown)

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/AskUserQuestionCard.tsx
git commit -m "feat(chat): add interactive AskUserQuestionCard"
```

---

## Task 11: PermissionCard (inline version)

**Files:**

- Modify: `apps/web/src/components/live/PermissionDialog.tsx` (export `getToolDisplay`)
- Create: `apps/web/src/components/chat/cards/PermissionCard.tsx`
- Reference: `apps/web/src/components/live/PermissionDialog.tsx` (existing modal)

**ARCHITECTURE NOTE — PermissionCard does NOT render inside RichPane:**
`PermissionRequestMsg` arrives over the **control WebSocket** (stored as `permissionRequest` in
`useControlSession` state), never in the JSONL stream. It is never a `RichMessage`. Therefore
`PermissionCard` cannot be wired inside `RichPane`.

Instead, `PermissionCard` renders as a **sibling to RichPane** in `MonitorPane` (and
`SessionDetailPanel`), passed via the `permissionCard` slot prop added in Task 18. The parent
that owns `useControlSession` builds the card and passes it down.

**Step 0: Export `getToolDisplay` from `PermissionDialog.tsx`**

`getToolDisplay` is currently module-private. `PermissionCard.tsx` lives in a different directory
and must import it. Change the declaration:

```ts
// In apps/web/src/components/live/PermissionDialog.tsx
// BEFORE (line ~141):
function getToolDisplay(...)

// AFTER:
export function getToolDisplay(...)
```

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -5`
Expected: 0 errors (export is additive, no callers break)

**Step 1: Write the component**

Inline card version of `PermissionDialog`. Uses `InteractiveCardShell` with `variant="permission"`.
Imports the now-exported `getToolDisplay` from `../../live/PermissionDialog`.

Key differences from the modal:

- Uses `InteractiveCardShell` instead of Radix Dialog overlay
- Same countdown logic (useEffect + setInterval)
- Same `getToolDisplay()` function for tool-specific previews
- Actions: Allow (green) + Deny (red) buttons
- After decision: resolves with "Allowed" (success) or "Denied" (denied) badge

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/PermissionCard.tsx
git commit -m "feat(chat): add inline PermissionCard"
```

---

## Task 12: PlanApprovalCard

**Files:**

- Create: `apps/web/src/components/chat/cards/PlanApprovalCard.tsx`

**Step 1: Write the component**

Uses `InteractiveCardShell` with `variant="plan"`. Plan content rendered as `<pre>` with `whitespace-pre-wrap`.

Key features:

- Actions: "Approve Plan" (blue) + "Request Changes" (gray)
- "Request Changes" expands a textarea for feedback text
- After decision: resolves with "Approved" or "Changes Requested" badge

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/PlanApprovalCard.tsx
git commit -m "feat(chat): add PlanApprovalCard"
```

---

## Task 13: ElicitationCard

**Files:**

- Create: `apps/web/src/components/chat/cards/ElicitationCard.tsx`

**Step 1: Write the component**

Uses `InteractiveCardShell` with `variant="elicitation"`. Simple: prompt text + text input + submit button. After submission, resolves with "Submitted" badge.

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/ElicitationCard.tsx
git commit -m "feat(chat): add ElicitationCard"
```

---

## Task 14: Cards barrel export

**Files:**

- Create: `apps/web/src/components/chat/cards/index.ts`

**Step 1: Write barrel export**

```ts
export { InteractiveCardShell } from './InteractiveCardShell'
export { AskUserQuestionCard } from './AskUserQuestionCard'
export { PermissionCard } from './PermissionCard'
export { PlanApprovalCard } from './PlanApprovalCard'
export { ElicitationCard } from './ElicitationCard'
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/index.ts
git commit -m "feat(chat): add cards barrel export"
```

---

## Task 15: ControlCallbacks type + useControlCallbacks hook

**Files:**

- Create: `apps/web/src/types/control-callbacks.ts`
- Create: `apps/web/src/hooks/use-control-callbacks.ts`

**Step 1: Define ControlCallbacks type**

This is the bridge between interactive cards (inside RichPane) and the sidecar (via useControlSession).

```ts
// control-callbacks.ts
export interface ControlCallbacks {
  /** Respond to AskUserQuestion card */
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  /** Respond to permission request (allow/deny) */
  respondPermission: (requestId: string, allowed: boolean) => void
  /** Respond to plan approval (approve/reject with optional feedback) */
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  /** Respond to elicitation dialog */
  submitElicitation: (requestId: string, response: string) => void
}
```

**Step 2: Add `sendRaw` method to `use-control-session.ts`**

> **Audit fix W1:** This step was originally Step 3. Reordered because Step 3 (now Step 3)
> imports `sendRaw`. If `sendRaw` doesn't exist yet, `useControlCallbacks` won't compile
> between steps.

The existing hook exposes `sendMessage(content: string)` for user chat messages and
`respondPermission(requestId, allowed)` for permissions. Neither handles generic JSON payloads.
Add a `sendRaw` method that sends arbitrary JSON — used by `useControlCallbacks` for card responses.

```ts
// In apps/web/src/hooks/use-control-session.ts
// Add after the existing sendMessage useCallback:

const sendRaw = useCallback((msg: Record<string, unknown>) => {
  const ws = wsRef.current
  if (!ws || ws.readyState !== WebSocket.OPEN) return
  ws.send(JSON.stringify(msg))
}, [])

// Update the return statement from:
return { ...state, sendMessage, respondPermission }
// To:
return { ...state, sendMessage, sendRaw, respondPermission }
```

**Step 3: Create hook that builds ControlCallbacks from useControlSession**

```ts
// use-control-callbacks.ts
import { useMemo } from 'react'
import type { ControlCallbacks } from '../types/control-callbacks'

/**
 * Builds a ControlCallbacks object from a control session's WebSocket send.
 * Returns undefined when no control session exists (read-only mode).
 *
 * Uses `sendRaw` (not `sendMessage`) — sendMessage only handles user chat messages
 * ({type:'user_message', content}). Card responses need arbitrary JSON (question_response, etc.).
 */
export function useControlCallbacks(
  sendRaw: ((msg: Record<string, unknown>) => void) | undefined,
  respondPermission: ((requestId: string, allowed: boolean) => void) | undefined,
): ControlCallbacks | undefined {
  return useMemo(() => {
    if (!sendRaw) return undefined
    return {
      answerQuestion: (requestId, answers) =>
        sendRaw({ type: 'question_response', requestId, answers }),
      respondPermission: (requestId, allowed) =>
        respondPermission?.(requestId, allowed),
      approvePlan: (requestId, approved, feedback) =>
        sendRaw({ type: 'plan_response', requestId, approved, feedback }),
      submitElicitation: (requestId, response) =>
        sendRaw({ type: 'elicitation_response', requestId, response }),
    }
  }, [sendRaw, respondPermission])
}
```

**Step 4: Type-check**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

**Step 5: Commit**

```bash
git add apps/web/src/types/control-callbacks.ts apps/web/src/hooks/use-control-callbacks.ts apps/web/src/hooks/use-control-session.ts
git commit -m "feat(chat): add ControlCallbacks type and useControlCallbacks hook"
```

---

## Task 16: TakeoverConfirmDialog + Session Origin Check

**Files:**

- Create: `apps/web/src/components/chat/TakeoverConfirmDialog.tsx`
- Modify: `apps/web/src/types/control.ts` (add `origin` field to `ControlSessionInfo`)

**Step 1: Create `ControlSessionInfo` in control types**

`ControlSessionInfo` does NOT currently exist in `apps/web/src/types/control.ts` — verified by
reading the file. The file contains `CostEstimate`, `ResumeResponse`, message types, and
`ServerMessage`. Add the new interface from scratch:

```ts
// In apps/web/src/types/control.ts — ADD (do not try to modify an existing interface):
export interface ControlSessionInfo {
  sessionId: string
  controlId: string
  status: 'idle' | 'running' | 'completed'
  origin: 'claude-view' | 'external'
}
```

**Step 2: Write TakeoverConfirmDialog**

Radix `AlertDialog` (already available) for confirming takeover of external sessions.

```tsx
// TakeoverConfirmDialog.tsx
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import { AlertTriangle } from 'lucide-react'

interface TakeoverConfirmDialogProps {
  open: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function TakeoverConfirmDialog({ open, onConfirm, onCancel }: TakeoverConfirmDialogProps) {
  return (
    <AlertDialog.Root open={open} onOpenChange={(o) => { if (!o) onCancel() }}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 bg-black/60 z-50" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[420px] bg-gray-900 border border-gray-700 rounded-xl p-6 z-50 shadow-xl">
          <div className="flex items-start gap-3 mb-4">
            <AlertTriangle className="w-5 h-5 text-amber-400 flex-shrink-0 mt-0.5" />
            <AlertDialog.Title className="text-sm font-medium text-gray-100">
              Take Control?
            </AlertDialog.Title>
          </div>
          <AlertDialog.Description className="text-xs text-gray-400 leading-relaxed mb-6 ml-8">
            This session was started outside claude-view (CLI / VS Code).
            Taking control will disconnect any active terminal input and
            route all interaction through this panel.
            The session's work will continue unaffected.
          </AlertDialog.Description>
          <div className="flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="px-3 py-1.5 text-xs text-gray-400 border border-gray-700 rounded-lg hover:bg-gray-800 cursor-pointer transition-colors"
              >
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="px-3 py-1.5 text-xs text-white bg-amber-600 hover:bg-amber-700 rounded-lg cursor-pointer transition-colors"
              >
                Take Control
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
```

**Step 3: Type-check**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

**Step 4: Commit**

```bash
git add apps/web/src/components/chat/TakeoverConfirmDialog.tsx apps/web/src/types/control.ts
git commit -m "feat(chat): add TakeoverConfirmDialog for external session takeover"
```

---

## Task 16.5: Add `controlId` to `LiveSession` type

**Files:**
- Modify: `apps/web/src/components/live/use-live-sessions.ts`

**Why required:** Tasks 18–19 need to check `session.controlId` to conditionally render the
ChatInputBar and PermissionCard. Currently `LiveSession` has no such field (confirmed: id, project,
projectPath, status, agentState, tokens, cost, etc. — no controlId).

**Step 1: Add the field to the `LiveSession` interface**

```ts
// In apps/web/src/components/live/use-live-sessions.ts
// Add to the LiveSession interface (after existing fields, e.g. after compactCount):
controlId?: string | null
```

**Sidecar prerequisite (tracked separately — not in this plan):**
The Rust SSE handler must emit `controlId` in `session_discovered` / `session_updated` events for
the field to be populated. Until that backend change ships, `controlId` will be `undefined` for all
sessions (feature flag off), and ChatInputBar will not appear — which is safe, not a regression.

**Step 2: Verify type-check passes**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors (optional field is additive)

**Step 3: Commit**

```bash
git add apps/web/src/components/live/use-live-sessions.ts
git commit -m "feat(live): add optional controlId field to LiveSession type"
```

---

## Task 17: Wire interactive cards into RichPane (with controlCallbacks prop)

**Files:**

- Modify: `apps/web/src/components/live/RichPane.tsx`
- Modify: `apps/web/src/components/live/PairedToolCard.tsx`

**Architecture note — what renders where:**

| Card | Where it renders | Why |
| --- | --- | --- |
| `AskUserQuestionCard` | Inside RichPane (via PairedToolCard) | Tool_use IS a RichMessage — arrives in JSONL stream |
| `PlanApprovalCard` | Inside RichPane (new ExitPlanMode detection) | Tool_use IS a RichMessage |
| `ElicitationCard` | Inside RichPane (new elicitation detection) | Arrives as progress message in JSONL |
| `PermissionCard` | **NOT inside RichPane** — sibling via MonitorPane slot | `permission_request` arrives on control WS, never JSONL |

**Step 1: Add `controlCallbacks` prop and import to `RichPane.tsx`**

```ts
// Add to imports at top of RichPane.tsx:
import type { ControlCallbacks } from '../../types/control-callbacks'

// Extend RichPaneProps interface (currently lines ~69-80):
export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  verboseMode?: boolean
  bufferDone?: boolean
  categoryCounts?: Record<ActionCategory, number>
  /** When provided, interactive cards (AskUserQuestion, PlanApproval, Elicitation) are active.
   *  When undefined (read-only session), cards render display-only (no action buttons). */
  controlCallbacks?: ControlCallbacks
}
```

**Step 2: Add `ExitPlanMode` to `displayMessages` compact-mode filter**

The `displayMessages` useMemo already shows `AskUserQuestion` tool_use in compact mode (lines
905–911). Add `ExitPlanMode` immediately after with the same pattern:

```ts
// After the AskUserQuestion block (line ~910), add:
if (m.type === 'tool_use' && m.name === 'ExitPlanMode')
  return true
```

**Step 3: Thread `controlCallbacks` down to `DisplayItemCard` → `PairedToolCard`**

```tsx
// Update renderItem to pass controlCallbacks:
const renderItem = useCallback(
  (index: number, item: DisplayItem) => (
    <div className="px-2 py-0.5">
      <DisplayItemCard
        item={item}
        index={index}
        verboseMode={verboseMode}
        controlCallbacks={controlCallbacks}
      />
    </div>
  ),
  [verboseMode, controlCallbacks],
)
```

Add `controlCallbacks?: ControlCallbacks` to `DisplayItemCard`'s props inline type and pass it
through to `PairedToolCard`.

**Step 4: Update `PairedToolCard.tsx` to use interactive `AskUserQuestionCard` when active**

> **Audit fix B1:** All `item.tool.*` references corrected → `toolUse.*`. `PairedToolCardProps`
> has `toolUse: RichMessage` and `toolResult: RichMessage | null` — there is no `item.tool`.
>
> **Audit fix B2:** `RichMessage` has no `toolUseId` field. Before this task, add
> `toolUseId?: string` to the `RichMessage` interface in `RichPane.tsx` (line ~48) and populate
> it from the JSONL `tool_use` block's `id` field in the parser (`parseRichMessages`).
>
> **Audit fix W3:** `planContent` now reads from `toolUse.inputData` (structured object) instead
> of `toolUse.input` (truncated summary string). `PlanApprovalCard` must handle the structured
> `inputData` and extract the plan content from it.

```tsx
// In PairedToolCard.tsx — add to imports:
import type { ControlCallbacks } from '../../types/control-callbacks'
import { AskUserQuestionCard } from '../chat/cards/AskUserQuestionCard'
import { PlanApprovalCard } from '../chat/cards/PlanApprovalCard'

// Add controlCallbacks to PairedToolCardProps:
// controlCallbacks?: ControlCallbacks

// In the AskUserQuestion rendering section (already checks isAskUserQuestionInput):
if (isAskUserQuestionInput(toolUse.inputData)) {
  if (controlCallbacks?.answerQuestion) {
    return (
      <AskUserQuestionCard
        inputData={toolUse.inputData}
        requestId={toolUse.toolUseId ?? ''}
        onAnswer={(requestId, answers) => controlCallbacks.answerQuestion(requestId, answers)}
      />
    )
  }
  // Fallback: display-only (existing behaviour, read-only sessions)
  return <AskUserQuestionDisplay inputData={toolUse.inputData} />
}

// Add ExitPlanMode handler nearby:
if (toolUse.name === 'ExitPlanMode') {
  if (controlCallbacks?.approvePlan) {
    return (
      <PlanApprovalCard
        requestId={toolUse.toolUseId ?? ''}
        planData={toolUse.inputData}
        onApprove={(requestId, approved, feedback) =>
          controlCallbacks.approvePlan(requestId, approved, feedback)
        }
      />
    )
  }
  // Fallback: show plan content as markdown block (display-only)
}
```

**Step 5: Type-check**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

**Step 6: Commit**

```bash
git add apps/web/src/components/live/RichPane.tsx apps/web/src/components/live/PairedToolCard.tsx
git commit -m "feat(chat): wire interactive cards into RichPane with ControlCallbacks"
```

---

## Task 18: Wire ChatInputBar into MonitorPane

**Files:**

- Modify: `apps/web/src/components/live/MonitorPane.tsx`
- Modify: Parent component that renders `<MonitorPane>` (find via `grep -r "MonitorPane" apps/web/src --include="*.tsx" -l`)

> **Architectural note:** `MonitorPane` must NOT import `useControlSession` or carry mode/model
> state. It receives pre-composed `ReactNode` slots from its parent. The parent calls
> `useControlSession`, builds `<ChatInputBar>` and `<PermissionCard>`, and passes them in.
> This keeps MonitorPane as a pure layout container.

**Step 1: Add slot props to `MonitorPaneProps`**

Add two `ReactNode` slots to `MonitorPane.tsx`:

```tsx
// In MonitorPaneProps interface — add after children:
chatInput?: React.ReactNode      // ChatInputBar — placed between content area and footer
permissionCard?: React.ReactNode // PermissionCard — placed above chatInput
```

Destructure in the function signature:

```tsx
export function MonitorPane({
  // ...existing props...
  chatInput,
  permissionCard,
  children,
}: MonitorPaneProps) {
```

**Step 2: Place slots in JSX**

Between the content `<div>` and `<Footer>`:

```tsx
      {/* Content area */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {children ?? (
          <div className="flex items-center justify-center h-full min-h-[120px] text-sm text-gray-400 dark:text-[#8B949E]">
            <Loader2 className="w-4 h-4 mr-2 animate-spin text-blue-500 dark:text-[#79C0FF]" />
            Connecting...
          </div>
        )}
      </div>

      {/* Permission card (above input bar when agent is waiting for approval) */}
      {permissionCard}

      {/* Chat input bar */}
      {chatInput}

      {/* Footer */}
      <Footer session={session} onExpand={onExpand} />
```

**Step 3: Wire slots in the parent component**

> **Audit fix B3 (Rules of Hooks):** `MonitorView.tsx` renders `MonitorPane` inside
> `visibleSessions.map()`. Hooks (`useControlSession`) cannot be called inside `.map()`.
> Solution: extract a per-session wrapper component `MonitorPaneWithControl` that calls
> `useControlSession` at component level. Each wrapper renders for one session.
>
> **Audit fix B4:** `controlStatusToInputState` returned `'idle'` and `'disabled'` which are NOT
> valid `InputBarState` values. Fixed to use `'active'`, `'dormant'`, `'waiting_permission'`, etc.
>
> **Audit fix B5:** `controlCallbacks` was `{ sendMessage, respondPermission }` which does NOT
> match the `ControlCallbacks` interface shape (`answerQuestion`, `respondPermission`,
> `approvePlan`, `submitElicitation`). Fixed to use `useControlCallbacks()` from Task 15.
>
> **Audit fix B8 (RESOLVED):** `contextUsage` is 0-100 percent, NOT 0-1 ratio. Evidence:
> `ChatStatusBar.tsx` line 2 documents `// 0-100 percentage`, `DashboardChat.tsx` line 70 passes
> it directly without multiplication. All `* 100` multiplications have been removed.

```tsx
import { useControlSession, type ControlStatus } from '../../hooks/use-control-session'
import { useControlCallbacks } from '../../hooks/use-control-callbacks'
import type { InputBarState } from '../chat/ChatInputBar'

// Helper — add near component (outside render):
function controlStatusToInputState(status: ControlStatus | undefined): InputBarState {
  switch (status) {
    case 'active':
    case 'waiting_input':       return 'active'
    case 'waiting_permission':  return 'waiting_permission'
    case 'connecting':          return 'connecting'
    case 'reconnecting':        return 'reconnecting'
    case 'completed':           return 'completed'
    case 'error':
    case 'disconnected':
    default:                    return 'dormant'
  }
}

// AUDIT FIX B3: Extract per-session wrapper to avoid calling hooks inside .map():
function MonitorPaneWithControl({ session, ...paneProps }: MonitorPaneProps & { session: LiveSession }) {
  // useControlSession accepts string | null; returns initialState + no-ops when null
  const controlSession = useControlSession(session.controlId ?? null)
  const controlCallbacks = useControlCallbacks(controlSession.sendRaw, controlSession.respondPermission)
  const inputState = controlStatusToInputState(controlSession.status)

  // B8 RESOLVED: contextUsage is 0-100 percent (confirmed ChatStatusBar.tsx:2, DashboardChat.tsx:70)
  const chatInputSlot = session.controlId ? (
    <ChatInputBar
      onSend={controlSession.sendMessage}
      state={inputState}
      contextPercent={Math.round(controlSession.contextUsage)}
    />
  ) : undefined

  const permissionCardSlot =
    controlSession.status === 'waiting_permission' && controlSession.permissionRequest ? (
      <PermissionCard
        permission={controlSession.permissionRequest}
        onRespond={controlSession.respondPermission}
      />
    ) : undefined

  return (
    <MonitorPane
      {...paneProps}
      chatInput={chatInputSlot}
      permissionCard={permissionCardSlot}
    >
      {/* AUDIT FIX B13: MonitorView renders <RichTerminalPane>, NOT <RichPane>.
          RichTerminalPane manages its own WS subscription by sessionId.
          RichPane requires pre-loaded messages[] — unavailable in this context.
          controlCallbacks is threaded through to RichPane inside RichTerminalPane
          (see Step 3a below for the RichTerminalPane interface extension). */}
      <RichTerminalPane
        sessionId={session.id}
        isVisible={isPaneVisible}
        verboseMode={verboseMode}
        controlCallbacks={controlCallbacks}
      />
    </MonitorPane>
  )
}

// In MonitorView's visibleSessions.map(), replace <MonitorPane> with:
// <MonitorPaneWithControl session={session} {...otherProps} />
```

> **Field reference:** `controlSession.permissionRequest` is the correct field name (confirmed in
> `use-control-session.ts` line 25). NOT `pendingPermission`.

**Step 3a: Extend `RichTerminalPane` to pass through `controlCallbacks`**

`RichTerminalPane` (48 lines, at `apps/web/src/components/live/RichTerminalPane.tsx`) wraps
`RichPane` and manages the WS subscription. Currently its props interface is:

```ts
// CURRENT (apps/web/src/components/live/RichTerminalPane.tsx lines 5-9):
interface RichTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  verboseMode: boolean
}
```

Add an optional `controlCallbacks` prop and pass it through to `RichPane`:

```ts
import type { ControlCallbacks } from './types/control'  // defined in Task 15

interface RichTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  verboseMode: boolean
  controlCallbacks?: ControlCallbacks   // ← NEW: optional, undefined = read-only
}

export function RichTerminalPane({ sessionId, isVisible, verboseMode, controlCallbacks }: RichTerminalPaneProps) {
  // ... existing useState + useTerminalSocket logic unchanged ...

  return (
    <RichPane
      messages={messages}
      isVisible={isVisible}
      verboseMode={verboseMode}
      bufferDone={bufferDone}
      controlCallbacks={controlCallbacks}  // ← NEW: pass through
    />
  )
}
```

> **Prerequisite:** `RichPane` must also accept an optional `controlCallbacks?: ControlCallbacks` prop.
> This is added in **Task 17 Step 3** when `RichPaneProps` is extended. Both changes (RichPane +
> RichTerminalPane) must land together — the import resolves because Task 15 defines `ControlCallbacks`
> and Task 17 extends `RichPaneProps` before Task 18 runs.

**Step 4: Type-check and build**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Run: `bun run build 2>&1 | tail -15`
Expected: 0 errors, successful build

**Step 5: Commit**

```bash
git add apps/web/src/components/live/MonitorPane.tsx
git add apps/web/src/components/live/MonitorView.tsx
git commit -m "feat(chat): wire ChatInputBar + PermissionCard into MonitorPane via slots"
```

---

## Task 19: Wire ChatInputBar into SessionDetailPanel terminal tab

**Files:**

- Modify: `apps/web/src/components/live/SessionDetailPanel.tsx`

> **Fix vs B6/Task 16.5:** Accept `controlId?: string` as an explicit prop rather than reading
> `session?.controlId` inline. `SessionDetailPanel` is used from both live monitoring and
> the history view — the `controlId` may come from different sources (SSE payload vs
> `/api/control/resume`). Accepting it as a prop supports both call sites without branching.

**Step 1: Add `controlId` prop to `SessionDetailPanel`**

```tsx
// Add to SessionDetailPanelProps (or equivalent):
controlId?: string
```

**Step 2: Call `useControlSession` unconditionally at the top of the component**

```tsx
// ALWAYS call at top level — hooks must NOT be conditional (Rules of Hooks):
// useControlSession accepts string | null; returns initialState + no-ops when null.
const controlSession = useControlSession(controlId ?? null)
const controlCallbacks = useControlCallbacks(controlSession.sendRaw, controlSession.respondPermission)
const inputState = controlStatusToInputState(controlSession.status)
// Reuse controlStatusToInputState helper from Task 18 Step 3 (or import from shared module).
```

**Step 3: Add ChatInputBar + PermissionCard in the terminal tab**

> **Audit fix B9:** The terminal tab is a bare `<RichPane>` with no wrapping div. Adding
> ChatInputBar and PermissionCard as siblings requires a flex column container. `RichPane` has no
> `className` prop, so wrap the entire tab content:

```tsx
// In the terminal tab content — WRAP in flex column container:
<div className="flex flex-col h-full">
  <div className="flex-1 min-h-0 overflow-hidden">
    <RichPane
      messages={messages}
      isVisible={isVisible}
      verboseMode={verboseMode}
      bufferDone={bufferDone}
      controlCallbacks={controlCallbacks}
    />
  </div>
  {controlSession.status === 'waiting_permission' && controlSession.permissionRequest && (
    <PermissionCard
      permission={controlSession.permissionRequest}
      onRespond={controlSession.respondPermission}
    />
  )}
  {controlId && (
    <ChatInputBar
    onSend={controlSession.sendMessage}
    state={inputState}
    contextPercent={Math.round(controlSession.contextUsage)}
  />
)}
</div>
```

> Note: `isStreaming` and `disabled` are NOT valid `ChatInputBar` props — the `state` machine
> (Task 8) handles enabled/disabled and streaming display. `contextPercent` receives
> `contextUsage` directly (already 0-100 percent — confirmed ChatStatusBar.tsx:2).

**Step 4: Commit**

```bash
git add apps/web/src/components/live/SessionDetailPanel.tsx
git commit -m "feat(chat): wire ChatInputBar into SessionDetailPanel terminal tab"
```

---

## Task 20: Wire ChatInputBar into ConversationView (history resume)

**Files:**

- Modify: `apps/web/src/components/ConversationView.tsx`

> **Layout prerequisite:** The left column in `ConversationView` is currently
> `<div className="flex-1 min-w-0">` (confirmed at line ~696). Add `flex flex-col` so
> ChatInputBar anchors at the bottom:
> ```tsx
> <div className="flex-1 min-w-0 flex flex-col">
> ```
> The Virtuoso list inside must expand to fill the remaining height — add `style={{ flex: 1 }}`
> to the `<Virtuoso>` component.

**Step 1: State for resume flow (correct hook patterns)**

```tsx
// State — controlId starts null (not yet resumed):
const [controlId, setControlId] = useState<string | null>(null)
const pendingMessageRef = useRef<string | null>(null)

// ALWAYS call useControlSession unconditionally — hooks must NOT be conditional.
// useControlSession accepts string | null and returns initialState + no-ops when null
// (confirmed: useEffect returns early on !controlId — see use-control-session.ts line 54).
const controlSession = useControlSession(controlId)
```

**Step 2: Send handler with resume-first and drain effect**

```tsx
const handleSend = useCallback(
  async (message: string) => {
    if (!controlId) {
      // Store message, trigger resume — WS isn't open yet so we can't send immediately
      pendingMessageRef.current = message
      // AUDIT FIX W9: Wrap in try/catch — existing handleResume (line ~221) demonstrates the
      // correct pattern with try/catch + showToast. Without error handling, a failed resume
      // silently strands the pending message and gives the user no feedback.
      try {
        const res = await fetch('/api/control/resume', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sessionId }),
        })
        if (!res.ok) throw new Error(`Resume failed: ${res.status}`)
        const data = await res.json()
        setControlId(data.controlId)
        // Pending message is sent by the drain effect below once WS reaches waiting_input
      } catch {
        pendingMessageRef.current = null
        showToast('Failed to resume session', 3000)
      }
    } else {
      controlSession.sendMessage(message)
    }
  },
  [controlId, controlSession.sendMessage, sessionId],
)

// Drain pending message once WS connection is ready to accept input:
// Audit fix W4: controlSession is a new object every render (spread { ...state, sendMessage, ... }).
// Using it as a dep fires the effect on every render, violating CLAUDE.md's
// "useEffect deps: Never raw parsed objects" rule.
// Fixed: use only primitive/stable deps.
useEffect(() => {
  if (
    controlId &&
    pendingMessageRef.current &&
    controlSession.status === 'waiting_input'
  ) {
    const msg = pendingMessageRef.current
    pendingMessageRef.current = null
    controlSession.sendMessage(msg)
  }
}, [controlId, controlSession.status, controlSession.sendMessage])
```

> **Why the drain effect?** After `setControlId(data.controlId)`, the WS is still opening.
> The server sends a `session_status` message that transitions state to `waiting_input` once
> the sidecar is ready. The effect fires on that transition — the correct send window.

**Step 3: Render ChatInputBar at bottom of left column**

```tsx
// Left column — changed from:
//   <div className="flex-1 min-w-0">
// to:
<div className="flex-1 min-w-0 flex flex-col">
  <Virtuoso ... style={{ flex: 1 }} />
  <ChatInputBar
    onSend={handleSend}
    state={controlStatusToInputState(controlSession.status)}
    contextPercent={Math.round(controlSession.contextUsage)}
    placeholder={controlId ? 'Send a message...' : 'Resume this session...'}
  />
</div>
```

**Step 4: Commit**

```bash
git add apps/web/src/components/ConversationView.tsx
git commit -m "feat(chat): add resume-and-send to ConversationView"
```

---

## Task 21: New session spawn UI

> **Audit fix B10:** `POST /api/control/start` does NOT exist in the Rust router. The router has
> `/control/estimate`, `/control/resume`, `/control/send`, `/control/sessions` — no `/control/start`.
> **Prerequisite:** Add a `start_session` handler in `crates/server/src/routes/control.rs` that
> proxies to the sidecar's session spawn API. This is a backend change tracked separately.
>
> **Audit fix W7:** The "+" button only works in dockview (custom layout) mode. In auto-grid mode
> (default), there is no panel concept. Constrain the "+" button to custom layout mode, or use
> a modal/overlay for auto-grid mode.

**Files:**

- Create: `apps/web/src/components/chat/NewSessionInput.tsx`
- Modify: `apps/web/src/components/live/MonitorView.tsx` (add "+" button)
- **Backend prerequisite:** `crates/server/src/routes/control.rs` (add `/control/start` endpoint)

**Step 1: Create NewSessionInput wrapper**

Thin wrapper around ChatInputBar for the "new session" context:

```tsx
// NewSessionInput.tsx
// Audit fix M3: missing React import for useCallback
import { useCallback } from 'react'
import { ChatInputBar } from './ChatInputBar'

export function NewSessionInput({ onSessionCreated }: { onSessionCreated: (sessionId: string) => void }) {
  const handleSend = useCallback(async (message: string) => {
    const res = await fetch('/api/control/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message }),
    })
    const data = await res.json()
    onSessionCreated(data.sessionId)
  }, [onSessionCreated])

  return (
    <ChatInputBar
      onSend={handleSend}
      placeholder="What do you want to build?"
      mode="code"
    />
  )
}
```

**Step 2: Add "+" button to MonitorView toolbar**

Add a button in MonitorView's header that opens a new empty dockview panel containing `NewSessionInput`. When the user sends a message, the panel transitions to a live session panel.

**Step 3: Commit**

```bash
git add apps/web/src/components/chat/NewSessionInput.tsx apps/web/src/components/live/MonitorView.tsx
git commit -m "feat(chat): add new session spawn via ChatInputBar"
```

---

## Task 22: Delete DashboardChat + ControlPage

> **Audit fix W2:** `HistoryView.tsx:605` navigates to `/control/${controlId}?sessionId=...`.
> Converting ControlPage to a redirect breaks the "Resume" button in HistoryView — users
> silently redirect to `/`. Must also update HistoryView's navigation target.
>
> **Audit fix M4:** ControlPage replacement must be whole-file, not just component body — existing
> imports reference `DashboardChat` and other deleted modules. Replace entire file contents.

**Files:**

- Delete: `apps/web/src/components/live/DashboardChat.tsx`
- Modify: `apps/web/src/pages/ControlPage.tsx` (replace entire file with redirect)
- Modify: `apps/web/src/router.tsx` (remove `/control/:controlId` route or redirect)
- Modify: `apps/web/src/components/HistoryView.tsx` (update resume navigation target)

**Step 1: Delete DashboardChat**

Remove `apps/web/src/components/live/DashboardChat.tsx` (261 lines of duplicate renderers).

**Step 2: Update ControlPage to redirect**

Replace the **entire file contents** (not just the component body — existing imports reference
deleted modules and will cause tsc errors):

```tsx
// ControlPage.tsx — redirect to monitor
import { Navigate } from 'react-router-dom'

export function ControlPage() {
  return <Navigate to="/" replace />
}
```

**Step 3: Update HistoryView resume navigation + add `?focus` consumer**

Find the "Resume" button/link in `HistoryView.tsx` that navigates to `/control/${controlId}`.
Update the target to navigate to the monitor view with a session focus parameter instead:

```tsx
// Change from:
navigate(`/control/${controlId}?sessionId=${sessionId}`)
// To:
navigate(`/?focus=${sessionId}`)
```

> **AUDIT FIX W8:** The `?focus` parameter has no consumer in the current codebase. Without a handler,
> the user navigates to `/` but no session is highlighted — a UX regression vs. the old DashboardChat
> behavior. The implementer MUST add a `?focus` handler in **`LiveMonitorPage.tsx`** (NOT
> `MonitorView.tsx` — MonitorView doesn't use `useSearchParams`; LiveMonitorPage already imports it
> at line 2 and owns `searchParams` at line 52).
>
> **Confirmed available:**
> - `useMonitorStore` has `selectPane: (id: string | null) => void` (monitor-store.ts line 24)
> - `LiveMonitorPage` already has `searchParams` from `useSearchParams()` (line 52)
> - When `?focus` is set, we also need to ensure we're in monitor view mode
>
> ```tsx
> // In LiveMonitorPage, after the existing useEffect blocks (~line 202), add:
> const selectPane = useMonitorStore((s) => s.selectPane)
> const focusSessionId = searchParams.get('focus')
>
> useEffect(() => {
>   if (focusSessionId) {
>     // Switch to monitor view if not already there (focus only makes sense in monitor mode)
>     if (viewMode !== 'monitor') {
>       handleViewModeChange('monitor')
>     }
>     selectPane(focusSessionId)
>     // Clear the ?focus param so it doesn't re-trigger on filter changes
>     const params = new URLSearchParams(searchParams)
>     params.delete('focus')
>     setSearchParams(params, { replace: true })
>   }
> }, [focusSessionId])  // intentionally minimal deps — run once on mount/param change
> ```
>
> **Import already present:** `useMonitorStore` is NOT imported in `LiveMonitorPage` — add:
> ```ts
> import { useMonitorStore } from '../store/monitor-store'
> ```

**Step 4: Verify no broken imports**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors (no file imports DashboardChat)

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: delete DashboardChat, redirect ControlPage, update HistoryView resume"
```

---

## Task 23: Build Verification + Visual Check

**Step 1: Run all web tests**

Run: `cd apps/web && bunx vitest run`
Expected: All tests pass (including commands.test.ts)

**Step 2: TypeScript type-check**

Run: `cd apps/web && bunx tsc --noEmit`
Expected: 0 errors

**Step 3: Build**

Run: `bun run build 2>&1 | tail -15`
Expected: Successful build

**Step 4: Visual verification (manual)**

Start the dev server and verify across all 4 integration points:

1. **Monitor panels:** ChatInputBar at bottom of panels with control connections
2. **Session detail panel:** ChatInputBar in terminal tab for live sessions
3. **History detail:** ChatInputBar at bottom, "Resume & Send" flow works
4. **New session:** "+" button opens empty panel, typing creates new session
5. **Interactive cards:** render inline in RichPane when interactive messages arrive
6. **Slash commands:** popover opens on `/` and navigates with arrow keys
7. **No regressions:** monitor view panels render normally

**Step 5: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(chat): address build/type issues from integration"
```

---

## Summary

| Task | Component | Est. Size | Dependencies |
| --- | --- | --- | --- |
| 1 | commands.ts + test | ~50 lines | None |
| 2 | ChatContextGauge | ~30 lines | @radix-ui/react-tooltip |
| 3 | CostPreview | ~25 lines | @radix-ui/react-tooltip |
| 4 | ModeSwitch | ~50 lines | @radix-ui/react-popover |
| 5 | ModelSelector | ~50 lines | @radix-ui/react-popover |
| 6 | AttachButton | ~60 lines | lucide-react |
| 7 | SlashCommandPopover | ~90 lines | commands.ts |
| 8 | ChatInputBar (with dormant state machine) | ~170 lines | All sub-components |
| 9 | InteractiveCardShell | ~100 lines | None |
| 10 | AskUserQuestionCard | ~100 lines | InteractiveCardShell |
| 11 | PermissionCard | ~80 lines | InteractiveCardShell, PermissionDialog pattern |
| 12 | PlanApprovalCard | ~60 lines | InteractiveCardShell |
| 13 | ElicitationCard | ~40 lines | InteractiveCardShell |
| 14 | Cards barrel export | ~6 lines | Cards |
| 15 | ControlCallbacks type + useControlCallbacks hook | ~50 lines | useControlSession |
| 16 | TakeoverConfirmDialog + session origin check | ~70 lines | @radix-ui/react-alert-dialog |
| 17 | RichPane card wiring (with controlCallbacks prop) | ~50 lines changed | Cards, ControlCallbacks |
| 18 | MonitorPane input + callbacks wiring | ~30 lines changed | ChatInputBar, useControlCallbacks |
| 19 | SessionDetailPanel terminal tab | ~20 lines changed | ChatInputBar |
| 20 | ConversationView resume + dormant input | ~50 lines changed | ChatInputBar, resume API, TakeoverConfirmDialog |
| 21 | New session spawn UI | ~30 lines + toolbar button | ChatInputBar, start API |
| 22 | Delete DashboardChat + ControlPage | ~270 lines deleted | None |
| 23 | Build verification | 0 lines | All tasks |

**Total new code:** ~1,170 lines across 20 files
**Code deleted:** ~270 lines (DashboardChat + ControlPage)
**Net change:** ~900 lines
**New dependencies:** 1 — `@radix-ui/react-alert-dialog` (Task 0). All other Radix + Lucide already installed.
**Test coverage:** commands.ts has unit tests. UI components verified via type-check + build.
**Interaction binding:** Task 15 provides the ControlCallbacks plumbing — card button clicks → hook → WebSocket → sidecar → Claude. Read-only sessions get display-only cards (no action buttons).
**Dormant input bar:** Task 8 implements 6-state machine (dormant/resuming/active/streaming/completed/controlled_elsewhere). Every session shows input bar; first send triggers resume.
**External session safety:** Task 16 adds TakeoverConfirmDialog — external sessions (CLI/VS Code) require confirmation before takeover; claude-view-spawned sessions resume directly.
**Backend prerequisite:** Tasks 20-21 need `/api/control/resume` and `/api/control/start` endpoints (already implemented in sidecar). Sidecar needs `origin` field in session info response.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| 1 | `@radix-ui/react-alert-dialog` absent from `apps/web/package.json` | Blocker | Added **Task 0** to install with `bun add` before any other task |
| 2 | `InputBarState` missing `'connecting'` and `'reconnecting'` states matching `ControlStatus` | Warning | **Task 8**: added both to `InputBarState` union and `STATE_CONFIG` entries |
| 3 | `contextUsage` from `useControlSession` — scale confirmed as 0-100 percent (NOT 0-1 ratio) | ~~Blocker~~ Resolved | **All tasks**: removed `* 100` multiplication. Evidence: `ChatStatusBar.tsx:2` documents "0-100 percentage", `DashboardChat.tsx:70` passes value directly |
| 4 | `COLORS` in `AskUserQuestionDisplay.tsx` is not exported | Warning | **Task 10**: added architecture note — define colors inline or copy from source |
| 5 | `getToolDisplay` in `PermissionDialog.tsx` is not exported (module-private) | Blocker | **Task 11**: added Step 0 to export the function before `PermissionCard` can import it |
| 6 | `PermissionCard` placed inside `RichPane` — architectural mismatch (control WS vs JSONL) | Blocker | **Task 11** + **Task 17**: `PermissionCard` is a sibling rendered via MonitorPane slot, never inside `RichPane` |
| 7 | `useControlSession` has no `sendWsMessage` / `send` method | Blocker | **Task 15**: replaced with `sendRaw` helper that calls `ws.send()` directly |
| 8 | `ControlSessionInfo` does not exist in `apps/web/src/types/control.ts` | Blocker | **Task 16**: changed from "modify existing" to **CREATE** new interface from scratch |
| 9 | `LiveSession` type has no `controlId` field | Blocker | Added **Task 16.5** to extend `LiveSession` with `controlId?: string` |
| 10 | `AskUserQuestion` interactive callback threading missed — flows via `PairedToolCard`, not direct dispatch | Warning | **Task 17**: complete rewrite with explicit PairedToolCard modification steps and `controlCallbacks` threading |
| 11 | `ExitPlanMode` tool_use not detected or filtered anywhere in `RichPane` | Warning | **Task 17**: added ExitPlanMode detection steps and `PlanApprovalCard` rendering |
| 12 | Task 18 passed entire `controlSession` hook result as a `MonitorPane` prop — wrong architecture | Blocker | **Task 18**: replaced with `chatInput?: ReactNode` + `permissionCard?: ReactNode` named slots |
| 13 | Task 18 referenced `controlSession.stop` (method does not exist in `useControlSession`) | Warning | **Task 18**: removed; slot approach means parent composes ChatInputBar without `onStop` |
| 14 | Task 18 referenced `mode`/`model` state vars not defined in `MonitorPane` | Warning | **Task 18**: state lives in parent; slot-based approach eliminates the issue |
| 15 | Task 19 read `session?.controlId` inline instead of accepting it as a prop | Warning | **Task 19**: `SessionDetailPanel` now takes explicit `controlId?: string` prop |
| 16 | Task 19 used `isStreaming`/`disabled` props that don't exist on `ChatInputBar` | Warning | **Task 19**: replaced with `state: InputBarState` per Task 8 state machine |
| 17 | Task 20 had conditional hook call `controlId ? useControlSession(controlId) : null` | Blocker | **Task 20**: always call `useControlSession(controlId)` — hook accepts `string \| null` (confirmed line 42 and 54 of `use-control-session.ts`) |
| 18 | Task 20 lost pending message after `setControlId` — WS not open yet on same render | Warning | **Task 20**: added `pendingMessageRef` + drain `useEffect` that fires on `waiting_input` status transition |
| 19 | Task 20 left column missing `flex flex-col` — ChatInputBar had no bottom anchor | Minor | **Task 20**: added layout prerequisite block with Virtuoso `style={{ flex: 1 }}` note |
| 20 | Multiple code blocks used `pendingPermission` — actual field is `permissionRequest` | Minor | **Tasks 18/19**: corrected to `permissionRequest` (confirmed `use-control-session.ts` line 25) |

### Audit Round 2 (2026-03-01)

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| B1 | `item.tool.*` field paths wrong in Task 17 — `PairedToolCardProps` has `toolUse`, not `item.tool` | Blocker | **Task 17 Step 4**: All `item.tool.*` → `toolUse.*` |
| B2 | `RichMessage` has no `toolUseId` field — `toolUse.toolUseId` always undefined | Blocker | **Task 17**: Added prerequisite note to add `toolUseId?: string` to `RichMessage` and populate from JSONL parser |
| B3 | Rules of Hooks violation — `useControlSession` called inside `visibleSessions.map()` in MonitorView | Blocker | **Task 18 Step 3**: Extracted `MonitorPaneWithControl` wrapper component |
| B4 | `controlStatusToInputState` returned `'idle'` and `'disabled'` — not valid `InputBarState` values | Blocker | **Task 18 Step 3**: Fixed mapping to `'active'`, `'dormant'`, `'waiting_permission'`, etc. |
| B5 | `controlCallbacks` passed `{ sendMessage, respondPermission }` — doesn't match `ControlCallbacks` interface | Blocker | **Task 18 Step 3**: Use `useControlCallbacks(sendRaw, respondPermission)` from Task 15 |
| B6 | `InputBarState` type not exported — Task 18 import fails | Blocker | **Task 8**: Added `export` to `type InputBarState` |
| B7 | `ContextGauge` name collision with existing `live/ContextGauge.tsx` (different API) | Blocker | **Task 2**: Renamed to `ChatContextGauge` everywhere |
| B8 | `contextUsage` scale ambiguity — 0-1 vs 0-100 unknown | ~~Blocker~~ Resolved | **All tasks**: Confirmed 0-100 percent via `ChatStatusBar.tsx:2` and `DashboardChat.tsx:70`. Removed all `* 100` multiplications. No ambiguity remains |
| B9 | Terminal tab is bare `<RichPane>` with no wrapping div — siblings need flex container | Blocker | **Task 19 Step 3**: Wrapped in `<div className="flex flex-col h-full">` |
| B10 | `POST /api/control/start` route doesn't exist in Rust router | Blocker | **Task 21**: Added backend prerequisite note |
| B11 | `AttachButton` receives unused `attachments` prop — `noUnusedParameters` will error | Blocker | **Task 6**: Removed `attachments` and `onRemove` from `AttachButtonProps` |
| B12 | `waiting_permission` → `'streaming'` UX confusion — shows "Claude is responding..." during permission wait | Blocker | **Task 8**: Added `'waiting_permission'` to `InputBarState` with distinct placeholder |
| W1 | Task 15 step ordering inverted — `useControlCallbacks` imports `sendRaw` before it exists | Warning | **Task 15**: Swapped Steps 2 and 3 |
| W2 | HistoryView navigates to `/control/${controlId}` which becomes dead redirect | Warning | **Task 22**: Added Step 3 to update HistoryView navigation target |
| W3 | `PlanApprovalCard` receives truncated `input` instead of full `inputData` | Warning | **Task 17 Step 4**: Changed `planContent={toolUse.input}` → `planData={toolUse.inputData}` |
| W4 | `useEffect` deps include `controlSession` object (new ref every render) — violates CLAUDE.md rule | Warning | **Task 20**: Changed deps to `[controlId, controlSession.status, controlSession.sendMessage]` |
| W7 | "+" button only works in dockview mode, not auto-grid | Warning | **Task 21**: Added note to constrain to custom layout mode |
| M2 | Summary says "0 new deps" but Task 0 installs one | Minor | Fixed summary line |
| M3 | `NewSessionInput.tsx` missing `useCallback` import | Minor | **Task 21**: Added import statement |
| M4 | ControlPage replacement should be whole-file | Minor | **Task 22 Step 2**: Clarified to replace entire file contents |

### Audit Round 3 — Adversarial Review (2026-03-01)

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| B13 | `MonitorPaneWithControl` renders `<RichPane>` but actual `MonitorView` uses `<RichTerminalPane>` — wrong component, wrong props | Blocker | **Task 18 Step 3**: Changed to `<RichTerminalPane>` with correct props. **Step 3a**: Added concrete `RichTerminalPane` interface extension to pass `controlCallbacks` through to `RichPane` — no TODO left |
| W8 | `/?focus=${sessionId}` navigation target has no consumer — `focus` param silently ignored | Warning | **Task 22 Step 3**: Added `?focus` handler in `LiveMonitorPage.tsx` (not MonitorView — it doesn't use `useSearchParams`). Uses confirmed `useMonitorStore.selectPane` (monitor-store.ts:24). Auto-switches to monitor view, clears param after use |
| W9 | `handleSend` in Task 20 has no error handling on `/api/control/resume` fetch — failed resume silently strands pending message | Warning | **Task 20 Step 2**: Added try/catch + `res.ok` guard + `showToast` + `pendingMessageRef` cleanup |
| B8r | `contextUsage * 100` in Tasks 18/19/20 would produce 0-10000 — `contextUsage` is already 0-100 percent (NOT 0-1 ratio) | Blocker | **Tasks 8/18/19/20**: Removed all `* 100` multiplications. Evidence: `ChatStatusBar.tsx:2` ("0-100 percentage"), `DashboardChat.tsx:70` (passes directly). Updated props comment and all 3 call sites |
| M5 | Task 18 Step 5 git add uses `<ParentComponent>.tsx` placeholder | Minor | Fixed to `MonitorView.tsx` (confirmed actual filename) |
