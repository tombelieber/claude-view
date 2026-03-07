// Pure state machine for WebSocket connection lifecycle.
// No side effects — all WS operations happen in the hook that wraps this.

import { NON_RECOVERABLE_CODES } from '../types/control'
import type { ServerMessage } from '../types/control'

const MAX_RECONNECT_ATTEMPTS = 10

export type ConnectionState =
  | { phase: 'idle' }
  | { phase: 'connecting'; sessionId: string }
  | { phase: 'active'; sessionId: string; lastSeq: number }
  | { phase: 'reconnecting'; sessionId: string; attempt: number; lastSeq: number }
  | { phase: 'fatal'; sessionId: string; reason: string; code?: number }
  | { phase: 'failed'; sessionId: string; reason: string }
  | { phase: 'completed'; sessionId: string }

export type ConnectionAction =
  | { type: 'connect'; sessionId: string }
  | { type: 'ws_open' }
  | { type: 'ws_message'; msg: ServerMessage; seq: number }
  | { type: 'ws_close'; code: number; reason: string }
  | { type: 'ws_error'; error: string }
  | { type: 'reset' }

export function connectionReducer(
  state: ConnectionState,
  action: ConnectionAction,
): ConnectionState {
  // Terminal states — no outgoing transitions except explicit user actions
  if (state.phase === 'fatal') {
    if (action.type === 'reset') return { phase: 'idle' }
    return state // absorb everything
  }

  switch (action.type) {
    case 'connect':
      if (state.phase === 'idle' || state.phase === 'failed' || state.phase === 'completed') {
        return { phase: 'connecting', sessionId: action.sessionId }
      }
      return state

    case 'ws_open':
      if (state.phase === 'connecting') {
        return { phase: 'active', sessionId: state.sessionId, lastSeq: -1 }
      }
      if (state.phase === 'reconnecting') {
        return { phase: 'active', sessionId: state.sessionId, lastSeq: state.lastSeq }
      }
      return state

    case 'ws_message': {
      if (state.phase !== 'active') return state
      const msg = action.msg

      // Fatal error — terminal
      if (msg.type === 'error' && msg.fatal) {
        return { phase: 'fatal', sessionId: state.sessionId, reason: msg.message }
      }

      // Session completed
      if (msg.type === 'session_status' && msg.status === 'completed') {
        return { phase: 'completed', sessionId: state.sessionId }
      }

      // Update lastSeq
      return { ...state, lastSeq: action.seq }
    }

    case 'ws_close': {
      // Non-recoverable close code → fatal
      if ((NON_RECOVERABLE_CODES as ReadonlySet<number>).has(action.code)) {
        if (
          state.phase === 'connecting' ||
          state.phase === 'active' ||
          state.phase === 'reconnecting'
        ) {
          return {
            phase: 'fatal',
            sessionId: state.sessionId,
            reason: action.reason || `Connection closed (${action.code})`,
            code: action.code,
          }
        }
      }

      // First-connect failure → failed (no retry)
      if (state.phase === 'connecting') {
        return {
          phase: 'failed',
          sessionId: state.sessionId,
          reason: action.reason || 'Connection failed',
        }
      }

      // Active → reconnecting
      if (state.phase === 'active') {
        return {
          phase: 'reconnecting',
          sessionId: state.sessionId,
          attempt: 1,
          lastSeq: state.lastSeq,
        }
      }

      // Reconnecting → increment or fail
      if (state.phase === 'reconnecting') {
        if (state.attempt >= MAX_RECONNECT_ATTEMPTS) {
          return {
            phase: 'failed',
            sessionId: state.sessionId,
            reason: 'Max reconnect attempts reached',
          }
        }
        return { ...state, attempt: state.attempt + 1 }
      }

      return state
    }

    case 'ws_error':
      if (state.phase === 'connecting') {
        return { phase: 'failed', sessionId: state.sessionId, reason: action.error }
      }
      return state

    case 'reset':
      return { phase: 'idle' }

    default:
      return state
  }
}
