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

interface ChatInputBarProps {
  onSend: (message: string) => void
  onStop?: () => void
  isStreaming?: boolean
  disabled?: boolean
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

export function ChatInputBar({
  onSend,
  onStop,
  isStreaming = false,
  disabled = false,
  placeholder = 'Send a message... (or type / for commands)',
  mode = 'code',
  onModeChange,
  model = 'claude-sonnet-4-6',
  onModelChange,
  contextPercent,
  estimatedCost,
  commands,
  onCommand,
}: ChatInputBarProps) {
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
    <div className="relative border-t border-gray-800 bg-gray-950 px-4 py-3">
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

## Task 9: PermissionCard (inline version)

**Files:**
- Create: `apps/web/src/components/chat/cards/PermissionCard.tsx`

Refactor of `PermissionDialog.tsx` logic into an inline card. Reuses `getToolDisplay()` pattern. Keeps the existing modal `PermissionDialog` untouched — this is an alternative inline rendering.

**Step 1: Write the component**

Extract the tool display logic, countdown timer, and Allow/Deny buttons into a card (not modal). Use the same timer pattern from `apps/web/src/components/live/PermissionDialog.tsx:21-41`.

Key differences from the modal:
- No Radix Dialog overlay — rendered inline in the message stream
- Same countdown logic (useEffect + setInterval)
- Same `getToolDisplay()` function
- Shows "Allowed"/"Denied" badge after decision

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/PermissionCard.tsx
git commit -m "feat(chat): add inline PermissionCard for message stream"
```

---

## Task 10: AskUserQuestionCard

**Files:**
- Create: `apps/web/src/components/chat/cards/AskUserQuestionCard.tsx`

**Step 1: Write the component**

Renders a question with radio/checkbox options and optional markdown preview panel. Shows "Other" free-text input. After submission, grays out and shows the selected answer.

Key features:
- Single-select: radio buttons (controlled via `useState`)
- Multi-select: checkboxes (controlled via `useState<Set<number>>`)
- Optional markdown preview: shown to the right when any option has `markdown` field
- "Other" option: text input that appears at bottom of option list
- Submit button: calls `onAnswer` with the selected option label(s)
- Answered state: entire card becomes `opacity-60`, shows selected answer badge, non-interactive

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/AskUserQuestionCard.tsx
git commit -m "feat(chat): add AskUserQuestionCard with single/multi-select and markdown preview"
```

---

## Task 11: PlanApprovalCard

**Files:**
- Create: `apps/web/src/components/chat/cards/PlanApprovalCard.tsx`

**Step 1: Write the component**

Renders plan markdown content with Approve/Reject buttons. "Request Changes" opens a textarea for feedback.

Key features:
- Plan content rendered as `<pre>` with `whitespace-pre-wrap` (or use existing markdown renderer if available)
- Two buttons: "Approve Plan" (green) and "Request Changes" (gray)
- "Request Changes" expands a textarea for feedback text
- After decision: shows "Approved" or "Changes Requested" badge, non-interactive

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/PlanApprovalCard.tsx
git commit -m "feat(chat): add PlanApprovalCard with approve/reject and feedback"
```

---

## Task 12: ElicitationCard

**Files:**
- Create: `apps/web/src/components/chat/cards/ElicitationCard.tsx`

**Step 1: Write the component**

Simple: prompt text + text input + submit button. After submission, shows the submitted text grayed out.

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/ElicitationCard.tsx
git commit -m "feat(chat): add ElicitationCard for generic dialog prompts"
```

---

## Task 13: Cards barrel export

**Files:**
- Create: `apps/web/src/components/chat/cards/index.ts`

**Step 1: Write barrel export**

```ts
export { PermissionCard } from './PermissionCard'
export { AskUserQuestionCard } from './AskUserQuestionCard'
export { PlanApprovalCard } from './PlanApprovalCard'
export { ElicitationCard } from './ElicitationCard'
```

**Step 2: Commit**

```bash
git add apps/web/src/components/chat/cards/index.ts
git commit -m "feat(chat): add cards barrel export"
```

---

## Task 14: Wire ChatInputBar into DashboardChat

**Files:**
- Modify: `apps/web/src/components/live/DashboardChat.tsx`

**Step 1: Replace textarea with ChatInputBar**

Replace lines 140-165 (the input section) with `<ChatInputBar>`. Wire up:
- `onSend={session.sendMessage}`
- `onStop` → not implemented yet (placeholder)
- `isStreaming={!!session.streamingContent}`
- `disabled={isInputDisabled}`
- `contextPercent={session.contextUsage}`
- Add `mode` and `model` as local state
- For V1, `onCommand` sends the command as a regular message (e.g., user types `/compact` → sends "/compact" as message text)

**Step 2: Add interactive card rendering**

In the message rendering section, detect `tool_use_start` messages with special tool names and render the appropriate card:
- `toolName === 'AskUserQuestion'` → `<AskUserQuestionCard>`
- `toolName === 'ExitPlanMode'` → `<PlanApprovalCard>`

For permissions, keep the existing `<PermissionDialog>` modal AND add `<PermissionCard>` inline as a dismissed log entry (shows what was approved/denied in the stream after the fact).

**Step 3: Type-check and build**

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Run: `bun run build 2>&1 | tail -15`
Expected: 0 errors, successful build

**Step 4: Commit**

```bash
git add apps/web/src/components/live/DashboardChat.tsx
git commit -m "feat(chat): wire ChatInputBar and interactive cards into DashboardChat"
```

---

## Task 15: Build Verification + Visual Check

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

The ChatInputBar cannot be fully verified without the Rust backend running (needs WebSocket + control session). However, the component should render correctly in isolation:

1. Check that the chat directory exists with all expected files
2. Verify no import errors in the build output
3. Verify the component tree is correct via type-checking

**Step 5: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(chat): address build/type issues from integration"
```

---

## Summary

| Task | Component | Est. Size | Dependencies |
|------|-----------|-----------|-------------|
| 1 | commands.ts + test | ~50 lines | None |
| 2 | ContextGauge | ~30 lines | @radix-ui/react-tooltip |
| 3 | CostPreview | ~25 lines | @radix-ui/react-tooltip |
| 4 | ModeSwitch | ~50 lines | @radix-ui/react-popover |
| 5 | ModelSelector | ~50 lines | @radix-ui/react-popover |
| 6 | AttachButton | ~60 lines | lucide-react |
| 7 | SlashCommandPopover | ~90 lines | commands.ts |
| 8 | ChatInputBar | ~150 lines | All sub-components |
| 9 | PermissionCard | ~80 lines | PermissionDialog pattern |
| 10 | AskUserQuestionCard | ~120 lines | None |
| 11 | PlanApprovalCard | ~60 lines | None |
| 12 | ElicitationCard | ~40 lines | None |
| 13 | Cards barrel export | ~5 lines | Cards |
| 14 | DashboardChat wiring | ~50 lines changed | ChatInputBar + cards |
| 15 | Build verification | 0 lines | All tasks |

**Total new code:** ~850 lines across 14 files
**New dependencies:** 0 (all Radix + Lucide already installed)
**Test coverage:** commands.ts has unit tests. UI components verified via type-check + build.
