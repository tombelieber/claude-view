import type { ConnHealth } from '../types'

export type ConnEvent =
  | { type: 'WS_CLOSE'; recoverable: boolean }
  | { type: 'WS_OPEN' }
  | { type: 'RECONNECT_ATTEMPT' }
  | { type: 'SESSION_CLOSED' }

export type ConnResult =
  | { stay: true; state: ConnHealth }
  | { stay: false; exit: 'ws_fatal'; error: string }
  | { stay: false; exit: 'replaced' }

const DEFAULT_MAX_RETRIES = 5

export function connTransition(
  state: ConnHealth,
  event: ConnEvent,
  maxRetries: number = DEFAULT_MAX_RETRIES,
): ConnResult {
  switch (state.health) {
    case 'ok':
      if (event.type === 'WS_CLOSE') {
        if (event.recoverable) {
          return { stay: true, state: { health: 'reconnecting', attempt: 1 } }
        }
        // Non-recoverable close → fatal immediately (don't silently swallow)
        return { stay: false, exit: 'ws_fatal', error: 'Non-recoverable WebSocket close' }
      }
      return { stay: true, state }

    case 'reconnecting':
      switch (event.type) {
        case 'WS_OPEN':
          return { stay: true, state: { health: 'ok' } }
        case 'RECONNECT_ATTEMPT':
          if (state.attempt >= maxRetries) {
            return { stay: false, exit: 'ws_fatal', error: `Max retries (${maxRetries}) exceeded` }
          }
          return { stay: true, state: { health: 'reconnecting', attempt: state.attempt + 1 } }
        case 'SESSION_CLOSED':
          return { stay: false, exit: 'replaced' }
        default:
          return { stay: true, state }
      }
  }
}
