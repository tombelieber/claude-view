import type { InputBarState } from '../components/chat/ChatInputBar'
import type { ConnectionHealth } from '../types/control'
import type { LiveStatus } from './live-status'

// Re-export from canonical location for backwards compatibility
export { deriveLiveStatus, type LiveStatus } from './live-status'

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
  liveStatus: LiveStatus,
  sessionState: SessionState,
): PanelMode {
  if (!sessionId) return { mode: 'blank' }

  if (liveStatus === 'cc_owned') return { mode: 'watching' }

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
