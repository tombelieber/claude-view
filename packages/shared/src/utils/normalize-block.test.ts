import { describe, expect, it } from 'vitest'
import type { ConversationBlock } from '../types/blocks'
import { normalizeBlock, normalizeBlocks } from './normalize-block'

describe('normalizeBlock', () => {
  it('defaults missing segments on assistant block to empty array', () => {
    const block = { type: 'assistant', id: 'a1' } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).segments).toEqual([])
    expect((block as any).streaming).toBe(false)
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
    expect((block as any).segments).toBe(segs)
    expect((block as any).streaming).toBe(true)
  })

  it('defaults missing requestId on interaction block with unique fallback', () => {
    const block1 = {
      type: 'interaction',
      id: 'i1',
      variant: 'permission',
    } as unknown as ConversationBlock
    const block2 = {
      type: 'interaction',
      id: 'i2',
      variant: 'question',
    } as unknown as ConversationBlock
    normalizeBlock(block1)
    normalizeBlock(block2)
    // Each gets a unique requestId, not shared ''
    expect((block1 as any).requestId).toMatch(/^req-/)
    expect((block2 as any).requestId).toMatch(/^req-/)
    expect((block1 as any).requestId).not.toBe((block2 as any).requestId)
    expect((block1 as any).resolved).toBe(false)
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
    expect((block as any).requestId).toBe('req-abc')
    expect((block as any).resolved).toBe(true)
  })

  it('defaults missing arrays on turn_boundary block', () => {
    const block = {
      type: 'turn_boundary',
      id: 'tb1',
      success: true,
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).usage).toEqual({})
    expect((block as any).modelUsage).toEqual({})
    expect((block as any).permissionDenials).toEqual([])
  })

  it('normalizes turn_boundary error.messages to array', () => {
    const block = {
      type: 'turn_boundary',
      id: 'tb1',
      success: false,
      error: { subtype: 'error_during_execution' },
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).error.messages).toEqual([])
  })

  it('defaults missing tools/agents on session_init system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'session_init',
      data: { model: 'claude-3' },
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).data.tools).toEqual([])
    expect((block as any).data.agents).toEqual([])
  })

  it('defaults missing taskId on task_started system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'task_started',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).data.taskId).toBe('')
  })

  it('defaults missing messageId on stream_delta system block', () => {
    const block = {
      type: 'system',
      id: 's1',
      variant: 'stream_delta',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).data.messageId).toBe('')
  })

  it('defaults missing output on auth_status notice block', () => {
    const block = {
      type: 'notice',
      id: 'n1',
      variant: 'auth_status',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).data.output).toEqual([])
  })

  it('defaults missing suggestion on prompt_suggestion notice block', () => {
    const block = {
      type: 'notice',
      id: 'n1',
      variant: 'prompt_suggestion',
      data: {},
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).data.suggestion).toBe('')
  })

  it('defaults missing speakers/entries on team_transcript block', () => {
    const block = {
      type: 'team_transcript',
      id: 'tt1',
    } as unknown as ConversationBlock
    normalizeBlock(block)
    expect((block as any).speakers).toEqual([])
    expect((block as any).entries).toEqual([])
  })

  it('generates collision-free ids for blocks missing one', () => {
    const block1 = { type: 'user' } as unknown as ConversationBlock
    const block2 = { type: 'user' } as unknown as ConversationBlock
    normalizeBlock(block1)
    normalizeBlock(block2)
    expect((block1 as any).id).toMatch(/^block-/)
    expect((block2 as any).id).toMatch(/^block-/)
    expect((block1 as any).id).not.toBe((block2 as any).id)
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
    ] as any)
    expect(result).toHaveLength(1)
    expect(result[0].type).toBe('user')
  })

  it('filters out objects without type field', () => {
    const result = normalizeBlocks([
      { id: 'x', foo: 'bar' },
      { type: 'assistant', id: 'a1' },
    ] as any)
    expect(result).toHaveLength(1)
    expect(result[0].type).toBe('assistant')
  })

  it('normalizes all blocks in array', () => {
    const blocks = [
      { type: 'assistant', id: 'a1' },
      { type: 'interaction', id: 'i1', variant: 'question' },
    ]
    const result = normalizeBlocks(blocks as any)
    expect((result[0] as any).segments).toEqual([])
    expect((result[1] as any).requestId).toMatch(/^req-/)
  })

  it('handles undefined input', () => {
    const result = normalizeBlocks(undefined as any)
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
    expect((block as any).data.files).toEqual([])
    expect((block as any).data.failed).toEqual([])
  })
})
