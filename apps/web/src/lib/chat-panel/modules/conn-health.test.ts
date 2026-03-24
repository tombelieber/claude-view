import { describe, expect, it } from 'vitest'
import type { ConnHealth } from '../types'
import { type ConnEvent, type ConnResult, connTransition } from './conn-health'

const ok: ConnHealth = { health: 'ok' }
const recon1: ConnHealth = { health: 'reconnecting', attempt: 1 }
const recon4: ConnHealth = { health: 'reconnecting', attempt: 4 }
const recon5: ConnHealth = { health: 'reconnecting', attempt: 5 }

describe('connTransition', () => {
  const cases: [string, ConnHealth, ConnEvent, ConnResult, number?][] = [
    // ok → reconnecting on recoverable close
    [
      'ok + WS_CLOSE recoverable → reconnecting(1)',
      ok,
      { type: 'WS_CLOSE', recoverable: true },
      { stay: true, state: { health: 'reconnecting', attempt: 1 } },
    ],

    // ok + non-recoverable close → exit ws_fatal immediately
    [
      'ok + WS_CLOSE non-recoverable → exit ws_fatal',
      ok,
      { type: 'WS_CLOSE', recoverable: false },
      { stay: false, exit: 'ws_fatal', error: 'Non-recoverable WebSocket close' } as ConnResult,
    ],

    // reconnecting → ok on WS_OPEN
    ['reconnecting(1) + WS_OPEN → ok', recon1, { type: 'WS_OPEN' }, { stay: true, state: ok }],

    // reconnecting(n) → reconnecting(n+1) on RECONNECT_ATTEMPT within limit
    [
      'reconnecting(1) + RECONNECT_ATTEMPT → reconnecting(2)',
      recon1,
      { type: 'RECONNECT_ATTEMPT' },
      { stay: true, state: { health: 'reconnecting', attempt: 2 } },
    ],

    // reconnecting(n) at max → exit ws_fatal
    [
      'reconnecting(5) + RECONNECT_ATTEMPT → exit ws_fatal (default max=5)',
      recon5,
      { type: 'RECONNECT_ATTEMPT' },
      { stay: false, exit: 'ws_fatal', error: 'Max retries (5) exceeded' } as ConnResult,
    ],

    // custom maxRetries
    [
      'reconnecting(4) + RECONNECT_ATTEMPT → exit ws_fatal (max=4)',
      recon4,
      { type: 'RECONNECT_ATTEMPT' },
      { stay: false, exit: 'ws_fatal', error: 'Max retries (4) exceeded' } as ConnResult,
      4,
    ],

    // reconnecting → exit replaced on SESSION_CLOSED
    [
      'reconnecting(1) + SESSION_CLOSED → exit replaced',
      recon1,
      { type: 'SESSION_CLOSED' },
      { stay: false, exit: 'replaced' } as ConnResult,
    ],

    // ok + unrelated → stay ok
    [
      'ok + RECONNECT_ATTEMPT → stay ok (no-op)',
      ok,
      { type: 'RECONNECT_ATTEMPT' },
      { stay: true, state: ok },
    ],
  ]

  it.each(cases)('%s', (_label, state, event, expected, maxRetries?) => {
    expect(connTransition(state, event, maxRetries)).toEqual(expected)
  })
})
