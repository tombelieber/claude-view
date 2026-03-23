import { describe, expect, test } from 'vitest'
import type { NobodySub } from '../types'
import { type NobodyEvent, nobodyTransition } from './nobody'

const mockBlocks = [{ type: 'user', id: '1', text: 'hello', timestamp: 1 }] as any

describe('nobodyTransition', () => {
  const cases: [string, NobodySub, NobodyEvent, NobodySub][] = [
    [
      'loading + HISTORY_OK → ready',
      { sub: 'loading' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { sub: 'ready', blocks: mockBlocks },
    ],
    [
      'loading + HISTORY_FAILED → ready(empty)',
      { sub: 'loading' },
      { type: 'HISTORY_FAILED' },
      { sub: 'ready', blocks: [] },
    ],
    [
      'ready + HISTORY_OK → ready(updated)',
      { sub: 'ready', blocks: [] },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { sub: 'ready', blocks: mockBlocks },
    ],
    [
      'ready + unrelated event → no change',
      { sub: 'ready', blocks: mockBlocks },
      { type: 'UNKNOWN' } as any,
      { sub: 'ready', blocks: mockBlocks },
    ],
  ]

  test.each(cases)('%s', (_, state, event, expected) => {
    expect(nobodyTransition(state, event)).toEqual(expected)
  })
})
