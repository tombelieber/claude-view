import { describe, expect, it } from 'vitest'
import { chatRegistry } from '../registry'
import { isChatVisibleQueueOp } from '../SystemBlock'
import type { SystemBlock } from '../../../../../types/blocks'

// ── Registry shape ────────────────────────────────────────────────────────────

describe('chatRegistry', () => {
  it('exports all required block type renderers', () => {
    expect(chatRegistry).toHaveProperty('user')
    expect(chatRegistry).toHaveProperty('assistant')
    expect(chatRegistry).toHaveProperty('interaction')
    expect(chatRegistry).toHaveProperty('turn_boundary')
    expect(chatRegistry).toHaveProperty('notice')
    expect(chatRegistry).toHaveProperty('system')
    expect(chatRegistry).toHaveProperty('progress')
    expect(chatRegistry).toHaveProperty('team_transcript')
  })

  it('exports canRender function', () => {
    expect(typeof chatRegistry.canRender).toBe('function')
  })
})

// ── canRender — queue_operation gate ─────────────────────────────────────────

describe('chatRegistry.canRender', () => {
  it('returns true for non-system blocks', () => {
    const block = { type: 'user' as const, id: 'u-1', text: 'hi', timestamp: 0 }
    expect(chatRegistry.canRender!(block)).toBe(true)
  })

  it('returns true for system blocks with non-queue_operation variant', () => {
    const block: SystemBlock = {
      type: 'system',
      id: 'sb-1',
      variant: 'session_init',
      data: {} as SystemBlock['data'],
    }
    expect(chatRegistry.canRender!(block)).toBe(true)
  })

  it('returns false for queue_operation enqueue without content', () => {
    const block: SystemBlock = {
      type: 'system',
      id: 'sb-2',
      variant: 'queue_operation',
      data: {
        type: 'queue-operation',
        operation: 'enqueue',
        timestamp: new Date().toISOString(),
        content: '',
      },
    }
    expect(chatRegistry.canRender!(block)).toBe(false)
  })

  it('returns true for queue_operation enqueue with non-empty content', () => {
    const block: SystemBlock = {
      type: 'system',
      id: 'sb-3',
      variant: 'queue_operation',
      data: {
        type: 'queue-operation',
        operation: 'enqueue',
        timestamp: new Date().toISOString(),
        content: 'Hello user',
      },
    }
    expect(chatRegistry.canRender!(block)).toBe(true)
  })

  it('returns false for queue_operation dequeue', () => {
    const block: SystemBlock = {
      type: 'system',
      id: 'sb-4',
      variant: 'queue_operation',
      data: {
        type: 'queue-operation',
        operation: 'dequeue',
        timestamp: new Date().toISOString(),
      },
    }
    expect(chatRegistry.canRender!(block)).toBe(false)
  })

  it('returns true for team_transcript blocks', () => {
    const block = {
      type: 'team_transcript' as const,
      id: 'tt-1',
      teamName: 'T',
      description: '',
      speakers: [],
      entries: [],
    }
    expect(chatRegistry.canRender!(block)).toBe(true)
  })
})

// ── isChatVisibleQueueOp ──────────────────────────────────────────────────────

describe('isChatVisibleQueueOp', () => {
  function makeQueueOp(operation: string, content?: string): SystemBlock {
    return {
      type: 'system',
      id: 'sb-q',
      variant: 'queue_operation',
      data: {
        type: 'queue-operation',
        operation: operation as 'enqueue' | 'dequeue' | 'remove' | 'popAll',
        timestamp: new Date().toISOString(),
        content,
      },
    }
  }

  it('returns false for non-queue_operation variant', () => {
    const block: SystemBlock = {
      type: 'system',
      id: 'sb-x',
      variant: 'session_init',
      data: {} as SystemBlock['data'],
    }
    expect(isChatVisibleQueueOp(block)).toBe(false)
  })

  it('returns false for enqueue without content', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('enqueue'))).toBe(false)
  })

  it('returns false for enqueue with empty content', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('enqueue', '   '))).toBe(false)
  })

  it('returns true for enqueue with non-empty content', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('enqueue', 'Hello'))).toBe(true)
  })

  it('returns false for dequeue operation', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('dequeue'))).toBe(false)
  })

  it('returns false for remove operation', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('remove'))).toBe(false)
  })

  it('returns false for popAll operation', () => {
    expect(isChatVisibleQueueOp(makeQueueOp('popAll'))).toBe(false)
  })
})
