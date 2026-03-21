import { describe, expect, it } from 'vitest'
import type { OutboxState } from '../types'
import { outboxTransition } from './outbox'

type OutboxEvent =
  | { type: 'QUEUE'; localId: string; text: string }
  | { type: 'MARK_SENT'; localId: string }
  | { type: 'MARK_FAILED'; localId: string }
  | { type: 'REMOVE'; localId: string }
  | { type: 'MARK_ALL_FAILED' }
  | { type: 'REMOVE_BY_TEXT'; text: string }

const empty: OutboxState = { messages: [] }

describe('outboxTransition', () => {
  it('QUEUE adds entry with status queued', () => {
    const result = outboxTransition(empty, { type: 'QUEUE', localId: 'a', text: 'hello' })
    expect(result.messages).toHaveLength(1)
    expect(result.messages[0]).toEqual({ localId: 'a', text: 'hello', status: 'queued' })
  })

  it('MARK_SENT sets status to sent and adds sentAt', () => {
    const state: OutboxState = { messages: [{ localId: 'a', text: 'hi', status: 'queued' }] }
    const result = outboxTransition(state, { type: 'MARK_SENT', localId: 'a' })
    expect(result.messages[0]!.status).toBe('sent')
    expect(result.messages[0]!.sentAt).toBeTypeOf('number')
  })

  it('MARK_FAILED sets status to failed', () => {
    const state: OutboxState = { messages: [{ localId: 'a', text: 'hi', status: 'queued' }] }
    const result = outboxTransition(state, { type: 'MARK_FAILED', localId: 'a' })
    expect(result.messages[0]!.status).toBe('failed')
  })

  it('REMOVE removes entry by localId', () => {
    const state: OutboxState = {
      messages: [
        { localId: 'a', text: 'hi', status: 'queued' },
        { localId: 'b', text: 'bye', status: 'queued' },
      ],
    }
    const result = outboxTransition(state, { type: 'REMOVE', localId: 'a' })
    expect(result.messages).toHaveLength(1)
    expect(result.messages[0]!.localId).toBe('b')
  })

  it('MARK_ALL_FAILED marks all queued and sent as failed', () => {
    const state: OutboxState = {
      messages: [
        { localId: 'a', text: 'hi', status: 'queued' },
        { localId: 'b', text: 'bye', status: 'sent', sentAt: 100 },
        { localId: 'c', text: 'ok', status: 'failed' },
      ],
    }
    const result = outboxTransition(state, { type: 'MARK_ALL_FAILED' })
    expect(result.messages.every((m) => m.status === 'failed')).toBe(true)
  })

  it('REMOVE_BY_TEXT removes first entry matching text', () => {
    const state: OutboxState = {
      messages: [
        { localId: 'a', text: 'hello', status: 'queued' },
        { localId: 'b', text: 'hello', status: 'queued' },
      ],
    }
    const result = outboxTransition(state, { type: 'REMOVE_BY_TEXT', text: 'hello' })
    expect(result.messages).toHaveLength(1)
    expect(result.messages[0]!.localId).toBe('b')
  })

  it('REMOVE_BY_TEXT with no match returns same state', () => {
    const state: OutboxState = { messages: [{ localId: 'a', text: 'hi', status: 'queued' }] }
    const result = outboxTransition(state, { type: 'REMOVE_BY_TEXT', text: 'nope' })
    expect(result.messages).toEqual(state.messages)
  })

  it('MARK_SENT on unknown localId returns same state', () => {
    const state: OutboxState = { messages: [{ localId: 'a', text: 'hi', status: 'queued' }] }
    const result = outboxTransition(state, { type: 'MARK_SENT', localId: 'z' })
    expect(result).toEqual(state)
  })
})
