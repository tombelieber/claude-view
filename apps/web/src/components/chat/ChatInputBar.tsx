import { ArrowUp, Square } from 'lucide-react'
import { useMemo } from 'react'
import type { ModelOption } from '../../hooks/use-models'
import type { SessionCapabilities } from '../../hooks/use-session-capabilities'
import { FEATURES } from '../../lib/feature-flags'
import { cn } from '../../lib/utils'
import type { PermissionMode } from '../../types/control'
import { AttachButton, AttachmentChips } from './AttachButton'
import { ChatContextGauge } from './ChatContextGauge'
import { ChatPalette } from './ChatPalette'
import { ModeSwitch } from './ModeSwitch'
import { ModelSelector } from './ModelSelector'
import { SlashCommandPopover } from './SlashCommandPopover'
import { ThinkingBudgetControl } from './ThinkingBudgetControl'
import type { SlashCommand } from './commands'
import { STATE_CONFIG } from './input/input-bar-state'
import { useChatInput } from './input/useChatInput'
import { buildPaletteSections } from './palette-items'

// Input-bar state machine + STATE_CONFIG live in ./input/input-bar-state.
// Re-export the type so existing consumers/tests keep importing it from here.
export type { InputBarState } from './input/input-bar-state'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface ChatInputBarProps {
  onSend: (message: string) => void
  onStop?: () => void
  state?: import('./input/input-bar-state').InputBarState
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
  capabilities?: SessionCapabilities
  modelOptions?: ModelOption[]
  onModelSwitch?: (model: string) => void
  onPaletteModeChange?: (mode: PermissionMode) => void
  onAgent?: (agent: string) => void
  onPaletteOpen?: () => void
  effortValue?: number | null
  onEffortChange?: (tokens: number | null) => void
}

// ---------------------------------------------------------------------------
// Component (thin view — input state/handlers live in useChatInput)
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

  const {
    textareaRef,
    input,
    slashOpen,
    setSlashOpen,
    attachments,
    send,
    canSend,
    handleSlashSelect,
    handleInputChange,
    handleKeyDown,
    handlePaste,
    handleAttach,
    handleRemoveAttachment,
    handlePaletteClose,
  } = useChatInput({
    onSend,
    onStop,
    isDisabled,
    isStreaming,
    mode,
    onModeChange,
    onCommand,
    onPaletteOpen,
  })

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

          {/* Send / Stop button. During streaming: Send if text typed (queues), else Stop. */}
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
