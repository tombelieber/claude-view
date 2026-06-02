// Input-bar presentation state machine. Extracted from ChatInputBar so the
// view, the logic hook, and tests share one source of truth for placeholders
// and per-state flags (disabled / muted).

export type InputBarState =
  | 'dormant'
  | 'active'
  | 'streaming'
  | 'waiting_permission'
  | 'completed'
  | 'controlled_elsewhere'
  | 'connecting'
  | 'reconnecting'

export interface StateConfig {
  placeholder: string
  disabled: boolean
  muted: boolean
}

export const STATE_CONFIG: Record<InputBarState, StateConfig> = {
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

/** Slash commands that map directly to a permission-mode change. */
export const MODE_COMMANDS = new Set([
  'default',
  'acceptEdits',
  'plan',
  'dontAsk',
  'bypassPermissions',
])
