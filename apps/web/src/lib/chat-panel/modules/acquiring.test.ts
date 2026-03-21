import { describe, expect, test } from 'vitest'
import type { AcquiringStep } from '../types'
import {
  type AcquiringEvent,
  type AcquiringResult,
  acquiringTransition,
} from './acquiring'

describe('acquiringTransition', () => {
  const cases: [string, AcquiringStep, AcquiringEvent, AcquiringResult][] = [
    [
      'posting + ACQUIRE_OK → ws_connecting',
      { step: 'posting' },
      { type: 'ACQUIRE_OK', controlId: 'c1' },
      { stay: true, state: { step: 'ws_connecting', controlId: 'c1' } },
    ],
    [
      'posting + ACQUIRE_FAILED → exit action_failed',
      { step: 'posting' },
      { type: 'ACQUIRE_FAILED', error: 'timeout' },
      { stay: false, exit: 'action_failed', error: 'timeout' },
    ],
    [
      'ws_connecting + ACQUIRE_FAILED → exit action_failed',
      { step: 'ws_connecting', controlId: 'c1' },
      { type: 'ACQUIRE_FAILED', error: 'ws refused' },
      { stay: false, exit: 'action_failed', error: 'ws refused' },
    ],
    [
      'ws_connecting + SESSION_INIT → exit active',
      { step: 'ws_connecting', controlId: 'c1' },
      { type: 'SESSION_INIT' },
      { stay: false, exit: 'active', controlId: 'c1' },
    ],
    [
      'ws_initializing + SESSION_INIT → exit active',
      { step: 'ws_initializing', controlId: 'c1' },
      { type: 'SESSION_INIT' },
      { stay: false, exit: 'active', controlId: 'c1' },
    ],
    [
      'ws_initializing + INIT_TIMEOUT → exit ws_fatal',
      { step: 'ws_initializing', controlId: 'c1' },
      { type: 'INIT_TIMEOUT' },
      { stay: false, exit: 'ws_fatal', error: 'Session init timed out' },
    ],
    [
      'posting + unrelated → no change',
      { step: 'posting' },
      { type: 'SESSION_INIT' } as AcquiringEvent,
      { stay: true, state: { step: 'posting' } },
    ],
  ]

  test.each(cases)('%s', (_, state, event, expected) => {
    expect(acquiringTransition(state, event)).toEqual(expected)
  })
})
