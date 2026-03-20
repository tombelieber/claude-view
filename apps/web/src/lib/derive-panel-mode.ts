import type { InputBarState } from '../components/chat/ChatInputBar'

export type LiveStatus = 'cc_owned' | 'cc_agent_sdk_owned' | 'inactive'
export type OwnSubState = 'active' | 'streaming' | 'waiting_permission' | 'compacting'
export type ConnectingReason = 'initial' | 'reconnecting'
export type ErrorReason = 'fatal' | 'replaced'

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
  sessionState: string,
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
      return 'controlled_elsewhere'
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
