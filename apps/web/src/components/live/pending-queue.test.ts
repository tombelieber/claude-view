import { describe, expect, it } from 'vitest'
import type { RichMessage } from './RichPane'
import type { ActionCategory } from './action-log/types'
import { findActiveUserEnqueues, isUserQueueContent } from './pending-queue'

function makeSystem(metadata: Record<string, unknown>): RichMessage {
  return {
    type: 'system',
    content: '',
    category: 'queue' as ActionCategory,
    metadata,
  }
}

describe('isUserQueueContent', () => {
  it('returns true for plain user text', () => {
    expect(isUserQueueContent('fix the bug')).toBe(true)
  })

  it('returns false for task dispatch JSON', () => {
    expect(isUserQueueContent('{"task_id":"abc","task_type":"local_bash"}')).toBe(false)
  })

  it('returns false for task notification XML', () => {
    expect(
      isUserQueueContent('<task-notification>\n<task-id>abc</task-id></task-notification>'),
    ).toBe(false)
  })

  it('returns false for empty/undefined', () => {
    expect(isUserQueueContent('')).toBe(false)
    expect(isUserQueueContent(undefined)).toBe(false)
  })
})

describe('findActiveUserEnqueues', () => {
  it('returns empty set when no queue messages', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hello' },
      { type: 'assistant', content: 'hi' },
    ]
    expect(findActiveUserEnqueues(messages).size).toBe(0)
  })

  it('marks un-dequeued user enqueue as active', () => {
    const messages: RichMessage[] = [
      { type: 'assistant', content: 'working...' },
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'fix the bug' }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(1)
    expect(active.has(1)).toBe(true)
  })

  it('dequeue removes the pending enqueue', () => {
    const messages: RichMessage[] = [
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'fix the bug' }),
      makeSystem({ type: 'queue-operation', operation: 'dequeue' }),
      { type: 'user', content: 'fix the bug' },
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(0)
  })

  it('ignores task dispatch enqueues', () => {
    const messages: RichMessage[] = [
      makeSystem({
        type: 'queue-operation',
        operation: 'enqueue',
        content: '{"task_id":"abc","task_type":"local_bash"}',
      }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(0)
  })

  it('handles FIFO: dequeue removes oldest enqueue', () => {
    const messages: RichMessage[] = [
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'first' }),
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'second' }),
      makeSystem({ type: 'queue-operation', operation: 'dequeue' }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(1)
    expect(active.has(1)).toBe(true) // "second" still pending
  })

  it('popAll clears all pending', () => {
    const messages: RichMessage[] = [
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'first' }),
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'second' }),
      makeSystem({ type: 'queue-operation', operation: 'popAll' }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(0)
  })

  it('interleaved task and user enqueues with dequeue', () => {
    const messages: RichMessage[] = [
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'user msg' }),
      makeSystem({
        type: 'queue-operation',
        operation: 'enqueue',
        content: '{"task_id":"abc","task_type":"local_bash"}',
      }),
      makeSystem({ type: 'queue-operation', operation: 'dequeue' }),
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'second user msg' }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(1)
    expect(active.has(3)).toBe(true)
  })

  it('task-dispatch before user: dequeue consumes task FIFO slot, not user slot', () => {
    const messages: RichMessage[] = [
      makeSystem({
        type: 'queue-operation',
        operation: 'enqueue',
        content: '{"task_id":"abc","task_type":"local_bash"}',
      }),
      makeSystem({ type: 'queue-operation', operation: 'enqueue', content: 'user msg' }),
      makeSystem({ type: 'queue-operation', operation: 'dequeue' }),
    ]
    const active = findActiveUserEnqueues(messages)
    expect(active.size).toBe(1)
    expect(active.has(1)).toBe(true)
  })
})
