import { ArrowUp, Square } from 'lucide-react'
import { useCallback, useMemo, useRef, useState } from 'react'
import type { ModelOption } from '../../hooks/use-models'
import type { SessionCapabilities } from '../../hooks/use-session-capabilities'
import { FEATURES } from '../../lib/feature-flags'
import { cn } from '../../lib/utils'
import type { PermissionMode } from '../../types/control'
import { AttachButton, AttachmentChips } from './AttachButton'
import { ChatContextGauge } from './ChatContextGauge'
import { ChatPalette } from './ChatPalette'
import { ModeSwitch, cycleMode } from './ModeSwitch'
import { ModelSelector } from './ModelSelector'
import { SlashCommandPopover } from './SlashCommandPopover'
import { ThinkingBudgetControl } from './ThinkingBudgetControl'
import type { SlashCommand } from './commands'
import { buildPaletteSections } from './palette-items'

// ---------------------------------------------------------------------------
// Dormant state machine
// ---------------------------------------------------------------------------

export type InputBarState =
  | 'dormant'
  | 'active'
  | 'streaming'
  | 'waiting_permission'
  | 'completed'
  | 'controlled_elsewhere'
  | 'connecting'
  | 'reconnecting'

interface StateConfig {
  placeholder: string
  disabled: boolean
  muted: boolean
}

const STATE_CONFIG: Record<InputBarState, StateConfig> = {
  dormant: { placeholder: 'Send a message...', disabled: false, muted: true },
  connecting: { placeholder: 'Connecting...', disabled: true, muted: true },
  reconnecting: { placeholder: 'Reconnecting...', disabled: true, muted: true },
  active: {
    placeholder: 'Send a message... (or type / for commands)',
    disabled: false,
    muted: false,
  },
  streaming: { placeholder: 'Type to queue a follow-up...', disabled: false, muted: false },
  waiting_permission: {
    placeholder: 'Waiting for permission response...',
    disabled: true,
    muted: false,
  },
  completed: { placeholder: 'Session ended', disabled: true, muted: true },
  controlled_elsewhere: {
    placeholder: 'This session is running in another process',
    disabled: true,
    muted: true,
  },
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface ChatInputBarProps {
  onSend: (message: string) => void
  onStop?: () => void
  state?: InputBarState
  placeholder?: string
  mode?: PermissionMode
  onModeChange?: (mode: PermissionMode) => void
  model?: string
  onModelChange?: (model: string) => void
  contextPercent?: number
  contextInfo?: {
    tokens: number
    limit: number
    source: 'statusline' | 'sidecar' | 'history'
  } | null
  commands?: SlashCommand[]
  onCommand?: (command: string) => void
  // NEW: Session capabilities for command palette
  capabilities?: SessionCapabilities
  // NEW: Model options for palette submenu
  modelOptions?: ModelOption[]
  // NEW: Palette-specific model switch (triggers resume)
  onModelSwitch?: (model: string) => void
  // NEW: Palette-specific mode change (triggers resume)
  onPaletteModeChange?: (mode: PermissionMode) => void
  // NEW: Palette agent invocation (sends @agent-name)
  onAgent?: (agent: string) => void
  // NEW: Called when palette opens — refresh commands/agents via WS
  onPaletteOpen?: () => void
  // Effort slider (thinking budget)
  effortValue?: number | null
  onEffortChange?: (tokens: number | null) => void
}

// ---------------------------------------------------------------------------
// Mode commands that map directly to onModeChange
// ---------------------------------------------------------------------------
const MODE_COMMANDS = new Set(['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions'])

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ChatInputBar({
  onSend,
  onStop,
  state = 'active',
  placeholder: placeholderProp,
  mode = 'default',
  onModeChange,
  model = 'sonnet',
  onModelChange,
  contextPercent,
  contextInfo,
  commands: commandsProp,
  onCommand,
  capabilities,
  modelOptions,
  onModelSwitch,
  onPaletteModeChange,
  onAgent,
  onPaletteOpen,
  effortValue,
  onEffortChange,
}: ChatInputBarProps) {
  const config = STATE_CONFIG[state]
  const resolvedPlaceholder = placeholderProp ?? config.placeholder
  const isDisabled = config.disabled
  const isMuted = config.muted
  const isStreaming = state === 'streaming'

  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [input, setInput] = useState('')
  const [slashOpen, setSlashOpen] = useState(false)
  const [attachments, setAttachments] = useState<File[]>([])

  // ---- Auto-grow textarea ----
  const autoGrow = useCallback(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`
  }, [])

  // ---- Send logic ----
  const send = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed || isDisabled) return
    onSend(trimmed)
    setInput('')
    setAttachments([])
    // Reset textarea height on next frame
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (el) {
        el.style.height = 'auto'
      }
    })
  }, [input, isDisabled, onSend])

  // ---- Slash command handling ----
  const handleSlashSelect = useCallback(
    (cmd: SlashCommand) => {
      if (MODE_COMMANDS.has(cmd.name) && onModeChange) {
        onModeChange(cmd.name as PermissionMode)
      } else if (onCommand) {
        onCommand(cmd.name)
      }
      setInput('')
      setSlashOpen(false)
      requestAnimationFrame(() => {
        const el = textareaRef.current
        if (el) {
          el.style.height = 'auto'
          el.focus()
        }
      })
    },
    [onModeChange, onCommand],
  )

  // ---- Input change ----
  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const value = e.target.value
      setInput(value)

      // Open slash popover when input starts with "/" (single-line, at start)
      const trimmed = value.trimStart()
      if (trimmed.startsWith('/') && !trimmed.includes('\n')) {
        if (!slashOpen) onPaletteOpen?.()
        setSlashOpen(true)
      } else {
        setSlashOpen(false)
      }

      requestAnimationFrame(autoGrow)
    },
    [autoGrow, slashOpen, onPaletteOpen],
  )

  // ---- Keyboard: Enter=send, Shift+Enter=newline, Escape=stop ----
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Don't intercept keys when slash popover is open
      // (the popover handles its own keyboard events)
      if (slashOpen) return

      if (e.key === 'Tab' && e.shiftKey && onModeChange) {
        e.preventDefault()
        onModeChange(cycleMode(mode))
      } else if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        send()
      } else if (e.key === 'Escape' && isStreaming && onStop) {
        e.preventDefault()
        onStop()
      }
    },
    [slashOpen, send, isStreaming, onStop, mode, onModeChange],
  )

  // ---- Image paste handler ----
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (isDisabled) return
      const items = e.clipboardData?.items
      if (!items) return

      const imageFiles: File[] = []
      for (let i = 0; i < items.length; i++) {
        const item = items[i]
        if (item.type.startsWith('image/')) {
          const file = item.getAsFile()
          if (file) imageFiles.push(file)
        }
      }

      if (imageFiles.length > 0) {
        setAttachments((prev) => [...prev, ...imageFiles])
      }
    },
    [isDisabled],
  )

  // ---- Attachments ----
  const handleAttach = useCallback((files: File[]) => {
    setAttachments((prev) => [...prev, ...files])
  }, [])

  const handleRemoveAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index))
  }, [])

  // ---- Command palette sections ----
  const paletteCallbacks = useMemo(
    () => ({
      onModelSwitch: onModelSwitch ?? (() => {}),
      onPaletteModeChange: onPaletteModeChange ?? (() => {}),
      onCommand: onCommand ?? (() => {}),
      onAgent: onAgent ?? (() => {}),
      onClear: () => onCommand?.('clear'),
      onCompact: () => onCommand?.('compact'),
    }),
    [onModelSwitch, onPaletteModeChange, onCommand, onAgent],
  )

  const paletteSections = useMemo(() => {
    if (!capabilities || !modelOptions) return null
    return buildPaletteSections(capabilities, modelOptions, paletteCallbacks, {
      sessionActive: state !== 'dormant' && state !== 'completed',
      isStreaming,
    })
  }, [capabilities, modelOptions, paletteCallbacks, state, isStreaming])

  const handlePaletteClose = useCallback(() => {
    setSlashOpen(false)
    setInput('')
    requestAnimationFrame(() => {
      const el = textareaRef.current
      if (el) {
        el.style.height = 'auto'
        el.focus()
      }
    })
  }, [])

  // ---- Send/Stop button ----
  const canSend = input.trim().length > 0 && !isDisabled

  if (!FEATURES.chat) return null

  return (
    <div
      className={cn(
        'relative border-t border-gray-200 dark:border-gray-800 px-4 py-3 transition-colors duration-200',
        isMuted && 'bg-gray-50 dark:bg-gray-900/50',
      )}
    >
      {/* Command palette or legacy slash popover — positioned above */}
      {slashOpen && paletteSections ? (
        <div className="absolute bottom-full left-0 right-0 mb-1 z-50">
          <ChatPalette
            sections={paletteSections}
            filter={input.replace(/^\//, '')}
            onClose={handlePaletteClose}
          />
        </div>
      ) : (
        <SlashCommandPopover
          input={input}
          open={slashOpen}
          onSelect={handleSlashSelect}
          onClose={() => setSlashOpen(false)}
          commands={commandsProp}
          anchorRef={textareaRef}
        />
      )}

      {/* Input chrome */}
      <div
        className={cn(
          'rounded-xl border transition-colors duration-200',
          isMuted
            ? 'border-gray-200 dark:border-gray-800 bg-gray-100 dark:bg-gray-900'
            : 'border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900',
          !isDisabled &&
            'focus-within:border-blue-400 dark:focus-within:border-blue-500 focus-within:ring-1 focus-within:ring-blue-400/30',
        )}
      >
        {/* Top bar: ModeSwitch (left) + Effort + ModelSelector (right) */}
        <div className="flex items-center justify-between px-3 pt-2 pb-1">
          <ModeSwitch
            mode={mode}
            onModeChange={onModeChange ?? (() => {})}
            disabled={isDisabled || !onModeChange}
          />
          <div className="flex items-center gap-3">
            {onEffortChange && (
              <ThinkingBudgetControl
                value={effortValue ?? null}
                onChange={onEffortChange}
                disabled={isDisabled}
              />
            )}
            <ModelSelector
              model={model}
              onModelChange={onModelChange ?? (() => {})}
              disabled={isDisabled || !onModelChange}
              isLive={state === 'active' || state === 'streaming' || state === 'waiting_permission'}
              onSetModel={onModelSwitch}
            />
          </div>
        </div>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={resolvedPlaceholder}
          disabled={isDisabled}
          rows={1}
          className={cn(
            'w-full resize-none bg-transparent px-3 py-2 text-sm leading-relaxed',
            'text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500',
            'focus:outline-none',
            'disabled:cursor-not-allowed disabled:opacity-60',
          )}
          style={{ minHeight: '36px', maxHeight: '200px' }}
          aria-label="Message input"
          data-testid="chat-input"
        />

        {/* Attachment chips */}
        <AttachmentChips attachments={attachments} onRemove={handleRemoveAttachment} />

        {/* Bottom bar: AttachButton, context gauge, cost, send button */}
        <div className="flex items-center justify-between px-2 pb-2">
          <div className="flex items-center gap-2">
            <AttachButton onAttach={handleAttach} disabled={isDisabled} />

            {contextPercent != null && (
              <ChatContextGauge
                percent={contextPercent}
                tokens={contextInfo?.tokens}
                limit={contextInfo?.limit}
                source={contextInfo?.source}
              />
            )}
          </div>

          {/* Send / Stop button */}
          {/* During streaming: show Send if user typed text (queues message), Stop otherwise */}
          {isStreaming && !canSend ? (
            <button
              type="button"
              onClick={onStop}
              disabled={!onStop}
              className={cn(
                'p-1.5 rounded-lg transition-colors duration-150',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                'bg-red-500 hover:bg-red-600 text-white disabled:opacity-50',
              )}
              aria-label="Stop generation"
            >
              <Square className="w-4 h-4" aria-hidden="true" />
            </button>
          ) : (
            <button
              type="button"
              onClick={send}
              disabled={!canSend}
              className={cn(
                'p-1.5 rounded-lg transition-colors duration-150',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                canSend
                  ? 'bg-blue-500 hover:bg-blue-600 text-white cursor-pointer'
                  : 'bg-gray-200 dark:bg-gray-700 text-gray-400 dark:text-gray-500 cursor-not-allowed',
              )}
              aria-label="Send message"
            >
              <ArrowUp className="w-4 h-4" aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
