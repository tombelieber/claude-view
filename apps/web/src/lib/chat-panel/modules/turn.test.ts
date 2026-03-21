import { describe, expect, it } from 'vitest'
import type { TurnState } from '../types'
import { type TurnEvent, turnTransition } from './turn'

const idle: TurnState = { turn: 'idle' }
const streaming: TurnState = { turn: 'streaming' }
const awaiting: TurnState = { turn: 'awaiting', kind: 'permission', requestId: 'r1' }
const compacting: TurnState = { turn: 'compacting' }

describe('turnTransition', () => {
  const cases: [string, TurnState, TurnEvent, TurnState][] = [
    // idle transitions
    ['idle + STREAM_DELTA → streaming', idle, { type: 'STREAM_DELTA' }, { turn: 'streaming' }],
    ['idle + BLOCKS_UPDATE → streaming', idle, { type: 'BLOCKS_UPDATE' }, { turn: 'streaming' }],
    [
      'idle + PERMISSION_REQUEST → awaiting',
      idle,
      { type: 'PERMISSION_REQUEST', kind: 'question', requestId: 'q1' },
      { turn: 'awaiting', kind: 'question', requestId: 'q1' },
    ],
    [
      'idle + SESSION_COMPACTING → compacting',
      idle,
      { type: 'SESSION_COMPACTING' },
      { turn: 'compacting' },
    ],

    // streaming transitions
    ['streaming + TURN_COMPLETE → idle', streaming, { type: 'TURN_COMPLETE' }, { turn: 'idle' }],
    ['streaming + TURN_ERROR → idle', streaming, { type: 'TURN_ERROR' }, { turn: 'idle' }],
    [
      'streaming + PERMISSION_REQUEST → awaiting',
      streaming,
      { type: 'PERMISSION_REQUEST', kind: 'plan', requestId: 'p1' },
      { turn: 'awaiting', kind: 'plan', requestId: 'p1' },
    ],

    // awaiting transitions
    ['awaiting + TURN_COMPLETE → idle', awaiting, { type: 'TURN_COMPLETE' }, { turn: 'idle' }],
    ['awaiting + TURN_ERROR → idle', awaiting, { type: 'TURN_ERROR' }, { turn: 'idle' }],
    [
      'awaiting + STREAM_DELTA → streaming',
      awaiting,
      { type: 'STREAM_DELTA' },
      { turn: 'streaming' },
    ],

    // compacting transitions
    ['compacting + COMPACT_DONE → idle', compacting, { type: 'COMPACT_DONE' }, { turn: 'idle' }],

    // no-op: unrelated events
    ['idle + TURN_COMPLETE → idle (no-op)', idle, { type: 'TURN_COMPLETE' }, idle],
    [
      'streaming + SESSION_COMPACTING → streaming (no-op)',
      streaming,
      { type: 'SESSION_COMPACTING' },
      streaming,
    ],
    [
      'compacting + STREAM_DELTA → compacting (no-op)',
      compacting,
      { type: 'STREAM_DELTA' },
      compacting,
    ],
  ]

  it.each(cases)('%s', (_label, state, event, expected) => {
    expect(turnTransition(state, event)).toEqual(expected)
  })
})
