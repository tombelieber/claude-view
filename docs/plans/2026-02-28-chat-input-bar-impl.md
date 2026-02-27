# ChatInputBar + Interactive Cards — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a shared chat input bar matching the Claude Code VSCode extension UX, plus 4 interactive message cards, and wire them into DashboardChat.

**Architecture:** Plain `<textarea>` with auto-grow + Radix UI Popover for slash commands + Lucide icons. Interactive cards rendered inline in the chat message stream. Zero new dependencies (Radix Popover and Lucide already installed).

**Tech Stack:** React 19, Radix UI (`react-popover`, `react-tooltip`), Lucide React, Tailwind CSS, Vitest

**Design doc:** `docs/plans/2026-02-28-chat-input-bar-design.md`

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

## Task 2: ContextGauge Sub-component

**Files:**
- Create: `apps/web/src/components/chat/ContextGauge.tsx`

**Step 1: Write the component**

```tsx
// ContextGauge.tsx
import * as Tooltip from '@radix-ui/react-tooltip'

interface ContextGaugeProps {
  percent: number // 0-100
}

export function ContextGauge({ percent }: ContextGaugeProps) {
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
git add apps/web/src/components/chat/ContextGauge.tsx
git commit -m "feat(chat): add ContextGauge sub-component"
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
  attachments: File[]
  onAttach: (files: File[]) => void
  onRemove: (index: number) => void
  disabled?: boolean
}

const ACCEPT = 'image/*,.txt,.md,.json,.ts,.tsx,.js,.jsx,.rs,.py,.css,.html'

export function AttachButton({ attachments, onAttach, onRemove, disabled }: AttachButtonProps) {
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
import { ContextGauge } from './ContextGauge'
import { CostPreview } from './CostPreview'
import { ModeSwitch } from './ModeSwitch'
import { ModelSelector } from './ModelSelector'
import { SlashCommandPopover } from './SlashCommandPopover'
import { COMMANDS } from './commands'
import type { SlashCommand } from './commands'

type Mode = 'plan' | 'code' | 'ask'

/** Dormant state machine: dormant → resuming → active → streaming → completed */
type InputBarState = 'dormant' | 'resuming' | 'active' | 'streaming' | 'completed' | 'controlled_elsewhere'

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
  contextPercent?: number
  estimatedCost?: { cached: number; uncached: number } | null
  // Commands
  commands?: SlashCommand[]
  onCommand?: (command: string) => void
}

const STATE_CONFIG: Record<InputBarState, { placeholder: string; disabled: boolean; muted: boolean }> = {
  dormant:   { placeholder: 'Resume this session...', disabled: false, muted: true },
  resuming:  { placeholder: 'Resuming session...', disabled: true, muted: true },
  active:    { placeholder: 'Send a message... (or type / for commands)', disabled: false, muted: false },
  streaming: { placeholder: 'Claude is responding...', disabled: true, muted: false },
  completed:              { placeholder: 'Session ended', disabled: true, muted: true },
  controlled_elsewhere:   { placeholder: 'Controlled in another tab', disabled: true, muted: true },
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
              attachments={attachments}
              onAttach={handleAttach}
              onRemove={handleRemoveAttachment}
              disabled={disabled}
            />
            {contextPercent != null && <ContextGauge percent={contextPercent} />}
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

- Create: `apps/web/src/components/chat/cards/PermissionCard.tsx`
- Reference: `apps/web/src/components/live/PermissionDialog.tsx` (existing modal)

**Step 1: Write the component**

Inline card version of `PermissionDialog`. Uses `InteractiveCardShell` with `variant="permission"`. Reuses `getToolDisplay()` pattern from the modal.

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

**Step 2: Create hook that builds ControlCallbacks from useControlSession**

```ts
// use-control-callbacks.ts
import { useMemo } from 'react'
import type { ControlCallbacks } from '../types/control-callbacks'

/**
 * Builds a ControlCallbacks object from a control session's WebSocket send.
 * Returns undefined when no control session exists (read-only mode).
 */
export function useControlCallbacks(
  sendWsMessage: ((msg: Record<string, unknown>) => void) | undefined,
  respondPermission: ((requestId: string, allowed: boolean) => void) | undefined,
): ControlCallbacks | undefined {
  return useMemo(() => {
    if (!sendWsMessage) return undefined
    return {
      answerQuestion: (requestId, answers) =>
        sendWsMessage({ type: 'question_response', requestId, answers }),
      respondPermission: (requestId, allowed) =>
        respondPermission?.(requestId, allowed),
      approvePlan: (requestId, approved, feedback) =>
        sendWsMessage({ type: 'plan_response', requestId, approved, feedback }),
      submitElicitation: (requestId, response) =>
        sendWsMessage({ type: 'elicitation_response', requestId, response }),
    }
  }, [sendWsMessage, respondPermission])
}
```

**Step 3: Extend useControlSession to expose raw sendWsMessage**

In `apps/web/src/hooks/use-control-session.ts`, expose a `send` method that sends arbitrary JSON over the WebSocket. This is needed for the new card response types.

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

**Step 1: Add `origin` field to control types**

```ts
// In apps/web/src/types/control.ts — add to existing ControlSessionInfo or ServerMessage types:
interface ControlSessionInfo {
  sessionId: string
  controlId: string
  status: 'idle' | 'running' | 'completed'
  origin: 'claude-view' | 'external'  // NEW
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

## Task 17: Wire interactive cards into RichPane (with controlCallbacks prop)

**Files:**

- Modify: `apps/web/src/components/live/RichPane.tsx`

**Step 1: Add `controlCallbacks` prop to RichPane**

```ts
// Add to RichPane's props interface
controlCallbacks?: ControlCallbacks  // undefined = read-only (display-only cards)
```

**Step 2: Add interactive card rendering to message dispatch**

RichPane already special-cases `AskUserQuestion` at lines 905-909 (compact mode detection). Extend the render dispatch:

- `tool_use` with `toolName === 'AskUserQuestion'` → `<AskUserQuestionCard onAnswer={controlCallbacks?.answerQuestion} />`
- `tool_use` with `toolName === 'ExitPlanMode'` → `<PlanApprovalCard onApprove={controlCallbacks?.approvePlan} />`
- Messages with `metadata.interactive === 'permission'` → `<PermissionCard onRespond={controlCallbacks?.respondPermission} />`
- Messages with `metadata.interactive === 'elicitation'` → `<ElicitationCard onSubmit={controlCallbacks?.submitElicitation} />`

When `controlCallbacks` is undefined (read-only), cards render without action buttons (display-only, showing the question/permission but no way to respond).

**Step 3: Type-check**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

**Step 4: Commit**

```bash
git add apps/web/src/components/live/RichPane.tsx
git commit -m "feat(chat): wire interactive cards into RichPane with ControlCallbacks"
```

---

## Task 18: Wire ChatInputBar into MonitorPane

**Files:**

- Modify: `apps/web/src/components/live/MonitorPane.tsx`

**Step 1: Add ChatInputBar to MonitorPane**

ChatInputBar appears at the bottom of MonitorPane, between the content area and footer, when the session has a control connection.

```tsx
// In MonitorPane, after the content area, before Footer:
{controlSession && (
  <ChatInputBar
    onSend={controlSession.sendMessage}
    onStop={controlSession.stop}
    isStreaming={!!controlSession.streamingContent}
    disabled={controlSession.status === 'completed' || controlSession.status === 'error'}
    mode={mode}
    onModeChange={setMode}
    model={model}
    onModelChange={setModel}
    contextPercent={session.contextWindowTokens ? contextPercent(session) : undefined}
  />
)}
```

MonitorPane needs a new optional `controlSession` prop (or accesses it via hook/context based on `session.controlId`).

**Step 2: Type-check and build**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Run: `bun run build 2>&1 | tail -15`
Expected: 0 errors, successful build

**Step 3: Commit**

```bash
git add apps/web/src/components/live/MonitorPane.tsx
git commit -m "feat(chat): wire ChatInputBar into MonitorPane"
```

---

## Task 19: Wire ChatInputBar into SessionDetailPanel terminal tab

**Files:**

- Modify: `apps/web/src/components/live/SessionDetailPanel.tsx`

**Step 1: Add ChatInputBar to the terminal tab**

`SessionDetailPanel` has tabs: overview, terminal, log, sub-agents, cost. The terminal tab renders `RichPane`. Add ChatInputBar below the RichPane when the session is live and controllable.

```tsx
// In the terminal tab content, after RichPane:
{session?.controlId && (
  <ChatInputBar
    onSend={controlSession.sendMessage}
    isStreaming={!!controlSession?.streamingContent}
    disabled={!controlSession || controlSession.status === 'completed'}
    contextPercent={contextPercent(session)}
  />
)}
```

Same wiring as MonitorPane — session is already live, `controlId` available.

**Step 2: Commit**

```bash
git add apps/web/src/components/live/SessionDetailPanel.tsx
git commit -m "feat(chat): wire ChatInputBar into SessionDetailPanel terminal tab"
```

---

## Task 20: Wire ChatInputBar into ConversationView (history resume)

**Files:**

- Modify: `apps/web/src/components/ConversationView.tsx`

**Step 1: Add ChatInputBar with resume-on-first-send**

ConversationView renders session history at `/sessions/:sessionId`. Add ChatInputBar at the bottom. On first send:

1. Call `/api/control/resume` with `sessionId` → get `controlId`
2. Transition to live mode (establish `useControlSession`)
3. Send the message via the new control connection

```tsx
// State for resume flow
const [controlId, setControlId] = useState<string | null>(null)
const controlSession = controlId ? useControlSession(controlId) : null

const handleSend = useCallback(async (message: string) => {
  if (!controlId) {
    // Resume first, then send
    const res = await fetch(`/api/control/resume`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sessionId }),
    })
    const data = await res.json()
    setControlId(data.controlId)
    // sendMessage will be available on next render via controlSession
  } else {
    controlSession?.sendMessage(message)
  }
}, [controlId, controlSession, sessionId])
```

ChatInputBar shows placeholder: "Resume this session..." when no controlId. Shows normal "Send a message..." after resumed.

**Step 2: Commit**

```bash
git add apps/web/src/components/ConversationView.tsx
git commit -m "feat(chat): add resume-and-send to ConversationView"
```

---

## Task 21: New session spawn UI

**Files:**

- Create: `apps/web/src/components/chat/NewSessionInput.tsx`
- Modify: `apps/web/src/components/live/MonitorView.tsx` (add "+" button)

**Step 1: Create NewSessionInput wrapper**

Thin wrapper around ChatInputBar for the "new session" context:

```tsx
// NewSessionInput.tsx
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

**Files:**

- Delete: `apps/web/src/components/live/DashboardChat.tsx`
- Modify: `apps/web/src/pages/ControlPage.tsx` (redirect to monitor or delete)
- Modify: `apps/web/src/router.tsx` (remove `/control/:controlId` route or redirect)

**Step 1: Delete DashboardChat**

Remove `apps/web/src/components/live/DashboardChat.tsx` (261 lines of duplicate renderers).

**Step 2: Update ControlPage to redirect**

Replace the component body with a redirect to the monitor view:

```tsx
// ControlPage.tsx — redirect to monitor
import { Navigate } from 'react-router-dom'

export function ControlPage() {
  return <Navigate to="/" replace />
}
```

**Step 3: Verify no broken imports**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors (no file imports DashboardChat)

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: delete DashboardChat, redirect ControlPage to monitor"
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
| 2 | ContextGauge | ~30 lines | @radix-ui/react-tooltip |
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
**New dependencies:** 0 (all Radix + Lucide already installed)
**Test coverage:** commands.ts has unit tests. UI components verified via type-check + build.
**Interaction binding:** Task 15 provides the ControlCallbacks plumbing — card button clicks → hook → WebSocket → sidecar → Claude. Read-only sessions get display-only cards (no action buttons).
**Dormant input bar:** Task 8 implements 6-state machine (dormant/resuming/active/streaming/completed/controlled_elsewhere). Every session shows input bar; first send triggers resume.
**External session safety:** Task 16 adds TakeoverConfirmDialog — external sessions (CLI/VS Code) require confirmation before takeover; claude-view-spawned sessions resume directly.
**Backend prerequisite:** Tasks 20-21 need `/api/control/resume` and `/api/control/start` endpoints (already implemented in sidecar). Sidecar needs `origin` field in session info response.
