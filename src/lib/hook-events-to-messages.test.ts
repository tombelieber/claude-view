import { describe, it, expect } from 'vitest'
import type { HookEventItem } from '../components/live/action-log/types'
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'
import {
  hookEventsToMessages,
  hookEventsToRichMessages,
  getMessageSortTs,
  mergeByTimestamp,
  suppressHookProgress,
  suppressRichHookProgress,
} from './hook-events-to-messages'

function makeHookEvent(overrides: Partial<HookEventItem> = {}): HookEventItem {
  return {
    id: 'hook-1',
    type: 'hook_event',
    timestamp: 1706400000,
    eventName: 'PreToolUse',
    label: 'Running: git status',
    group: 'autonomous',
    ...overrides,
  }
}

describe('hookEventsToMessages', () => {
  it('converts a hook event to a synthetic Message with original event in metadata', () => {
    const event = makeHookEvent()
    const msgs = hookEventsToMessages([event])

    expect(msgs).toHaveLength(1)
    expect(msgs[0].role).toBe('progress')
    expect(msgs[0].uuid).toBe('hook-event-hook-1')
    expect(msgs[0].category).toBe('hook')
    expect(msgs[0].metadata.type).toBe('hook_event')
    expect(msgs[0].metadata._hookEvent).toBe(event) // same reference
  })

  it('sets timestamp as ISO string when positive', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 1706400000 })])
    expect(msgs[0].timestamp).toBe(new Date(1706400000 * 1000).toISOString())
  })

  it('sets timestamp to null when zero', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 0 })])
    expect(msgs[0].timestamp).toBeNull()
  })

  it('carries _sortTs in metadata for fast merge', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 1706400000 })])
    expect(msgs[0].metadata._sortTs).toBe(1706400000)
  })

  it('returns empty array for empty input', () => {
    expect(hookEventsToMessages([])).toEqual([])
  })
})

describe('hookEventsToRichMessages', () => {
  it('converts a hook event to a RichMessage with original event in metadata', () => {
    const event = makeHookEvent()
    const rich = hookEventsToRichMessages([event])

    expect(rich).toHaveLength(1)
    expect(rich[0].type).toBe('progress')
    expect(rich[0].category).toBe('hook')
    expect(rich[0].ts).toBe(1706400000)
    expect(rich[0].metadata!.type).toBe('hook_event')
    expect(rich[0].metadata!._hookEvent).toBe(event) // same reference
  })

  it('sets ts to undefined when timestamp is zero', () => {
    const rich = hookEventsToRichMessages([makeHookEvent({ timestamp: 0 })])
    expect(rich[0].ts).toBeUndefined()
  })

  it('returns empty array for empty input', () => {
    expect(hookEventsToRichMessages([])).toEqual([])
  })
})

describe('getMessageSortTs', () => {
  it('returns _sortTs from metadata when available', () => {
    const msg = { metadata: { _sortTs: 1000 }, timestamp: '2026-01-01T00:00:00Z' } as any
    expect(getMessageSortTs(msg)).toBe(1000)
  })

  it('falls back to parsing ISO timestamp', () => {
    const msg = { metadata: {}, timestamp: '2026-01-28T10:00:00Z' } as any
    const expected = Date.parse('2026-01-28T10:00:00Z') / 1000
    expect(getMessageSortTs(msg)).toBe(expected)
  })

  it('returns undefined for null timestamp', () => {
    const msg = { metadata: {}, timestamp: null } as any
    expect(getMessageSortTs(msg)).toBeUndefined()
  })

  it('returns undefined for invalid timestamp string', () => {
    const msg = { metadata: {}, timestamp: 'not-a-date' } as any
    expect(getMessageSortTs(msg)).toBeUndefined()
  })
})

describe('mergeByTimestamp', () => {
  const getTs = (n: { ts?: number }) => n.ts

  it('merges two sorted arrays maintaining order', () => {
    const a = [{ ts: 1 }, { ts: 3 }, { ts: 5 }]
    const b = [{ ts: 2 }, { ts: 4 }]
    const merged = mergeByTimestamp(a, b, getTs)
    expect(merged.map(x => x.ts)).toEqual([1, 2, 3, 4, 5])
  })

  it('returns a when b is empty', () => {
    const a = [{ ts: 1 }]
    const result = mergeByTimestamp(a, [], getTs)
    expect(result).toBe(a) // Same reference, no copy
  })

  it('returns b when a is empty', () => {
    const b = [{ ts: 1 }]
    const result = mergeByTimestamp([], b, getTs)
    expect(result).toBe(b)
  })

  it('pushes items without timestamps to the end', () => {
    const a = [{ ts: 1 }, { ts: undefined }]
    const b = [{ ts: 2 }]
    const merged = mergeByTimestamp(a, b, getTs)
    expect(merged.map(x => x.ts)).toEqual([1, 2, undefined])
  })

  it('preserves order for equal timestamps (stable)', () => {
    const a = [{ ts: 1, src: 'a' }]
    const b = [{ ts: 1, src: 'b' }]
    const merged = mergeByTimestamp(a, b, (x) => x.ts)
    expect(merged[0].src).toBe('a') // a comes first when equal
  })
})

describe('suppressHookProgress', () => {
  it('filters out hook_progress messages', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
      { role: 'progress', content: '', metadata: { type: 'hook_progress' } } as any as Message,
      { role: 'progress', content: '', metadata: { type: 'bash_progress' } } as any as Message,
    ]
    const filtered = suppressHookProgress(messages)
    expect(filtered).toHaveLength(2)
    expect(filtered[0].role).toBe('user')
    expect(filtered[1].metadata?.type).toBe('bash_progress')
  })

  it('returns all messages when none are hook_progress', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
    ]
    expect(suppressHookProgress(messages)).toHaveLength(1)
  })

  it('handles messages with null metadata', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
    ]
    expect(suppressHookProgress(messages)).toHaveLength(1)
  })
})

describe('suppressRichHookProgress', () => {
  it('filters out hook_progress RichMessages', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'progress', content: '', metadata: { type: 'hook_progress' } },
      { type: 'progress', content: '', metadata: { type: 'hook_event' } },
    ]
    const filtered = suppressRichHookProgress(messages)
    expect(filtered).toHaveLength(2)
    expect(filtered[1].metadata!.type).toBe('hook_event')
  })
})
