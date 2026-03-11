import { ArrowUp, Square } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { cn } from '../../lib/utils'
import type { PermissionMode } from '../../types/control'
import { AttachButton, AttachmentChips } from './AttachButton'
import { ChatContextGauge } from './ChatContextGauge'
import { ModeSwitch } from './ModeSwitch'
import { ModelSelector } from './ModelSelector'
import { SlashCommandPopover } from './SlashCommandPopover'
import type { SlashCommand } from './commands'

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
  dormant: { placeholder: 'Resume this session...', disabled: false, muted: true },
  connecting: { placeholder: 'Connecting...', disabled: true, muted: true },
  reconnecting: { placeholder: 'Reconnecting...', disabled: true, muted: true },
  active: {
    placeholder: 'Send a message... (or type / for commands)',
    disabled: false,
    muted: false,
  },
  streaming: { placeholder: 'Claude is responding...', disabled: true, muted: false },
  waiting_permission: {
    placeholder: 'Waiting for permission response...',
    disabled: true,
    muted: false,
  },
  completed: { placeholder: 'Session ended', disabled: true, muted: true },
  controlled_elsewhere: {
    placeholder: 'Controlled in another tab',
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
  commands?: SlashCommand[]
  onCommand?: (command: string) => void
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
  model = 'claude-sonnet-4-6',
  onModelChange,
  contextPercent,
  commands: commandsProp,
  onCommand,
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
        setSlashOpen(true)
      } else {
        setSlashOpen(false)
      }

      requestAnimationFrame(autoGrow)
    },
    [autoGrow],
  )

  // ---- Keyboard: Enter=send, Shift+Enter=newline, Escape=stop ----
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Don't intercept keys when slash popover is open
      // (the popover handles its own keyboard events)
      if (slashOpen) return

      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        send()
      } else if (e.key === 'Escape' && isStreaming && onStop) {
        e.preventDefault()
        onStop()
      }
    },
    [slashOpen, send, isStreaming, onStop],
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

  // ---- Send/Stop button ----
  const canSend = input.trim().length > 0 && !isDisabled

  return (
    <div
      className={cn(
        'relative border-t border-gray-200 dark:border-gray-800 px-4 py-3 transition-colors duration-200',
        isMuted && 'bg-gray-50 dark:bg-gray-900/50',
      )}
    >
      {/* Slash command popover — positioned above */}
      <SlashCommandPopover
        input={input}
        open={slashOpen}
        onSelect={handleSlashSelect}
        onClose={() => setSlashOpen(false)}
        commands={commandsProp}
        anchorRef={textareaRef}
      />

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
        {/* Top bar: ModeSwitch (left) + ModelSelector (right) */}
        <div className="flex items-center justify-between px-3 pt-2 pb-1">
          <ModeSwitch
            mode={mode}
            onModeChange={onModeChange ?? (() => {})}
            disabled={isDisabled || !onModeChange}
          />
          <ModelSelector
            model={model}
            onModelChange={onModelChange ?? (() => {})}
            disabled={isDisabled || !onModelChange}
          />
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
        />

        {/* Attachment chips */}
        <AttachmentChips attachments={attachments} onRemove={handleRemoveAttachment} />

        {/* Bottom bar: AttachButton, context gauge, cost, send button */}
        <div className="flex items-center justify-between px-2 pb-2">
          <div className="flex items-center gap-2">
            <AttachButton onAttach={handleAttach} disabled={isDisabled} />

            {contextPercent != null && <ChatContextGauge percent={contextPercent} />}
          </div>

          {/* Send / Stop button */}
          <button
            type="button"
            onClick={isStreaming ? onStop : send}
            disabled={isStreaming ? !onStop : !canSend}
            className={cn(
              'p-1.5 rounded-lg transition-colors duration-150',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
              isStreaming
                ? 'bg-red-500 hover:bg-red-600 text-white disabled:opacity-50'
                : canSend
                  ? 'bg-blue-500 hover:bg-blue-600 text-white cursor-pointer'
                  : 'bg-gray-200 dark:bg-gray-700 text-gray-400 dark:text-gray-500 cursor-not-allowed',
            )}
            aria-label={isStreaming ? 'Stop generation' : 'Send message'}
          >
            {isStreaming ? (
              <Square className="w-4 h-4" aria-hidden="true" />
            ) : (
              <ArrowUp className="w-4 h-4" aria-hidden="true" />
            )}
          </button>
        </div>
      </div>
    </div>
  )
}
