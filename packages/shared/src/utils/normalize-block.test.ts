import { describe, expect, it, vi } from 'vitest'
import type { ConversationBlock } from '../types/blocks'
import { normalizeBlock, normalizeBlocks } from './normalize-block'

// Helper: cast to access normalized fields without triggering noExplicitAny
const raw = (b: ConversationBlock) => b as Record<string, unknown>

describe('normalizeBlock', () => {
  it('defaults missing segments on assistant block to empty array', () => {
    const block = { type: 'assistant', id: 'a1' } as unknown as ConversationBlock
    normalizeBlock(block)
    expect(raw(block).segments).toEqual([])
    expect(raw(block).streaming).toBe(false)
  })

  it('preserves existing segments on assistant block', () => {
    const segs = [{ kind: 'text', text: 'hello' }]
    const block = {
      type: 'assistant',
      id: 'a1',
      segments: segs,
      streaming: true,
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect(raw(block).segments).toBe(segs)
    expect(raw(block).streaming).toBe(true)
  })

  it('defaults missing requestId to empty string (historical blocks are resolved)', () => {
    const block = {
      type: 'interaction',
      id: 'i1',
      variant: 'permission',
    } as unknown as ConversationBlock
    normalizeBlock(block)
    // '' is truthful: no active request exists for historical blocks
    expect(raw(block).requestId).toBe('')
    expect(raw(block).resolved).toBe(false)
  })

  it('preserves existing requestId on interaction block', () => {
    const block = {
      type: 'interaction',
      id: 'i1',
      variant: 'permission',
      requestId: 'req-abc',
      resolved: true,
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect(raw(block).requestId).toBe('req-abc')
    expect(raw(block).resolved).toBe(true)
  })

  it('defaults missing arrays on turn_boundary block', () => {
    const block = {
      type: 'turn_boundary',
      id: 'tb1',
      success: true,
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect(raw(block).usage).toEqual({})
    expect(raw(block).modelUsage).toEqual({})
    expect(raw(block).permissionDenials).toEqual([])
  })

  it('normalizes turn_boundary error.messages to array', () => {
    const block = {
      type: 'turn_boundary',
      id: 'tb1',
      success: false,
      error: { subtype: 'error_during_execution' },
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).error as Record<string, unknown>).messages).toEqual([])
  })

  it('defaults missing tools/agents on session_init system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'session_init',
      data: { model: 'claude-3' },
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).tools).toEqual([])
    expect((raw(block).data as Record<string, unknown>).agents).toEqual([])
  })

  it('defaults missing taskId on task_started system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'task_started',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).taskId).toBe('')
  })

  it('defaults missing messageId on stream_delta system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'stream_delta',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).messageId).toBe('')
  })

  it('defaults missing output on auth_status notice block', () => {
    const block = {
      type: 'notice',
      id: 'n1',
      variant: 'auth_status',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).output).toEqual([])
  })

  it('defaults missing suggestion on prompt_suggestion notice block', () => {
    const block = {
      type: 'notice',
      id: 'n1',
      variant: 'prompt_suggestion',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).suggestion).toBe('')
  })

  it('defaults missing speakers/entries on team_transcript block', () => {
    const block = {
      type: 'team_transcript',
      id: 'tt1',
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect(raw(block).speakers).toEqual([])
    expect(raw(block).entries).toEqual([])
  })
})

describe('normalizeBlocks', () => {
  it('filters out non-object entries', () => {
    const result = normalizeBlocks([
      null,
      undefined,
      42,
      'string',
      { type: 'user', id: 'u1', text: 'hi' },
    ] as unknown[])
    expect(result).toHaveLength(1)
    expect(result[0].type).toBe('user')
  })

  it('filters out objects without type field', () => {
    const result = normalizeBlocks([
      { id: 'x', foo: 'bar' },
      { type: 'assistant', id: 'a1' },
    ] as unknown[])
    expect(result).toHaveLength(1)
    expect(result[0].type).toBe('assistant')
  })

  it('drops blocks with missing id and logs error', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const result = normalizeBlocks([{ type: 'user' }, { type: 'assistant', id: 'a1' }] as unknown[])
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('a1')
    expect(errorSpy).toHaveBeenCalledWith(
      '[normalizeBlocks] dropped block with missing id — upstream bug:',
      expect.objectContaining({ type: 'user' }),
    )
    errorSpy.mockRestore()
  })

  it('normalizes all blocks in array', () => {
    const blocks = [
      { type: 'assistant', id: 'a1' },
      { type: 'interaction', id: 'i1', variant: 'question' },
    ]
    const result = normalizeBlocks(blocks as unknown[])
    expect(raw(result[0]).segments).toEqual([])
    expect(raw(result[1]).requestId).toBe('')
  })

  it('handles undefined input', () => {
    const result = normalizeBlocks(undefined as unknown[])
    expect(result).toEqual([])
  })

  it('defaults files_saved arrays', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'files_saved',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((raw(block).data as Record<string, unknown>).files).toEqual([])
    expect((raw(block).data as Record<string, unknown>).failed).toEqual([])
  })
})
