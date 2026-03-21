import type { AcquiringStep } from '../types'

// E-B2: Only 4 events — WS_OPEN/WS_FAILED handled by coordinator
export type AcquiringEvent =
  | { type: 'ACQUIRE_OK'; controlId: string; sessionId?: string }
  | { type: 'ACQUIRE_FAILED'; error: string }
  | { type: 'SESSION_INIT' }
  | { type: 'INIT_TIMEOUT' }

export type AcquiringResult =
  | { stay: true; state: AcquiringStep }
  | { stay: false; exit: 'active'; controlId: string; sessionId?: string }
  | { stay: false; exit: 'action_failed'; error: string }
  | { stay: false; exit: 'ws_fatal'; error: string }

export function acquiringTransition(s: AcquiringStep, e: AcquiringEvent): AcquiringResult {
  switch (s.step) {
    case 'posting':
      if (e.type === 'ACQUIRE_OK')
        return { stay: true, state: { step: 'ws_connecting', controlId: e.controlId } }
      if (e.type === 'ACQUIRE_FAILED')
        return { stay: false, exit: 'action_failed', error: e.error }
      return { stay: true, state: s }

    case 'ws_connecting':
      if (e.type === 'ACQUIRE_FAILED')
        return { stay: false, exit: 'action_failed', error: e.error }
      if (e.type === 'SESSION_INIT')
        return { stay: false, exit: 'active', controlId: s.controlId }
      return { stay: true, state: s }

    case 'ws_initializing':
      if (e.type === 'SESSION_INIT')
        return { stay: false, exit: 'active', controlId: s.controlId }
      if (e.type === 'INIT_TIMEOUT')
        return { stay: false, exit: 'ws_fatal', error: 'Session init timed out' }
      return { stay: true, state: s }
  }
}
