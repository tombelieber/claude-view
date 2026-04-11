import type { SessionOwnership } from '@claude-view/shared/types/generated/SessionOwnership'
import type { InputBarState } from '../components/chat/ChatInputBar'
import type { ConnectionHealth } from '../types/control'

/** Ownership tier extracted from backend SSE. null = no live data / inactive. */
export type OwnershipTier = SessionOwnership['tier'] | null

/**
 * True when session is CLI/terminal-owned (watching mode, not interactive).
 * Both tmux and observed open terminal WS for read-only block streaming.
 * Distinction: tmux sessions can be shut down (we launched them);
 * observed sessions are read-only (someone else's CLI).
 */
export function isWatchable(tier: OwnershipTier): boolean {
  return tier === 'tmux' || tier === 'observed'
}

export type OwnSubState = 'active' | 'streaming' | 'waiting_permission' | 'compacting'
export type ConnectingReason = 'initial' | 'reconnecting'
export type ErrorReason = 'fatal' | 'replaced'

export type SessionState =
  | 'idle'
  | 'initializing'
  | 'reconnecting'
  | 'waiting_input'
  | 'active'
  | 'waiting_permission'
  | 'compacting'
  | 'closed'
  | 'error'
  | 'replaced'

export type PanelMode =
  | { mode: 'blank' }
  | { mode: 'history' }
  | { mode: 'watching' }
  | { mode: 'connecting'; reason: ConnectingReason }
  | { mode: 'own'; subState: OwnSubState }
  | { mode: 'error'; reason: ErrorReason }

export function derivePanelMode(
  sessionId: string | undefined,
  ownershipTier: OwnershipTier,
  sessionState: SessionState,
): PanelMode {
  if (!sessionId) return { mode: 'blank' }

  if (isWatchable(ownershipTier)) return { mode: 'watching' }

  switch (sessionState) {
    case 'initializing':
      return { mode: 'connecting', reason: 'initial' }
    case 'reconnecting':
      return { mode: 'connecting', reason: 'reconnecting' }
    case 'waiting_input':
      return { mode: 'own', subState: 'active' }
    case 'active':
      return { mode: 'own', subState: 'streaming' }
    case 'waiting_permission':
      return { mode: 'own', subState: 'waiting_permission' }
    case 'compacting':
      return { mode: 'own', subState: 'compacting' }
    case 'error':
      return { mode: 'error', reason: 'fatal' }
    case 'replaced':
      return { mode: 'error', reason: 'replaced' }
  }

  return { mode: 'history' }
}

export function modeToInputBar(mode: PanelMode): InputBarState {
  switch (mode.mode) {
    case 'blank':
      return 'dormant'
    case 'history':
      return 'active'
    case 'watching':
      return 'active'
    case 'connecting':
      return mode.reason === 'reconnecting' ? 'reconnecting' : 'connecting'
    case 'own': {
      switch (mode.subState) {
        case 'active':
          return 'active'
        case 'streaming':
          return 'streaming'
        case 'waiting_permission':
          return 'waiting_permission'
        case 'compacting':
          return 'streaming'
        default:
          return mode.subState satisfies never
      }
    }
    case 'error':
      return 'completed'
    default:
      return (mode as { mode: never }).mode satisfies never
  }
}

export function modeToConnectionHealth(mode: PanelMode): ConnectionHealth {
  if (mode.mode === 'connecting' && mode.reason === 'reconnecting') return 'degraded'
  if (mode.mode === 'error') return 'lost'
  return 'ok'
}
