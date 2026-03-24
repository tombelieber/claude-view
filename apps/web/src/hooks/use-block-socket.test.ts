import { describe, expect, it } from 'vitest'
import { mergeBlockById } from './use-block-socket'
import type { ConversationBlock } from '@claude-view/shared/types/blocks'

describe('mergeBlockById', () => {
  it('adds new block to empty map', () => {
    const map = new Map<string, ConversationBlock>()
    const incoming = {
      type: 'user',
      id: 'u-1',
      text: 'hello',
      timestamp: 1000,
    } as ConversationBlock
    const result = mergeBlockById(map, incoming)
    expect(result.size).toBe(1)
    expect(result.get('u-1')?.id).toBe('u-1')
  })

  it('replaces existing block with same ID', () => {
    const map = new Map<string, ConversationBlock>()
    map.set('u-1', { type: 'user', id: 'u-1', text: 'old', timestamp: 1000 } as ConversationBlock)
    const incoming = { type: 'user', id: 'u-1', text: 'new', timestamp: 1001 } as ConversationBlock
    const result = mergeBlockById(map, incoming)
    expect(result.size).toBe(1)
    expect((result.get('u-1') as any).text).toBe('new')
  })

  it('preserves insertion order when replacing (ES2015 Map spec)', () => {
    const map = new Map<string, ConversationBlock>()
    map.set('u-1', { type: 'user', id: 'u-1', text: 'a', timestamp: 1000 } as ConversationBlock)
    map.set('u-2', { type: 'user', id: 'u-2', text: 'b', timestamp: 1001 } as ConversationBlock)
    map.set('u-3', { type: 'user', id: 'u-3', text: 'c', timestamp: 1002 } as ConversationBlock)
    const incoming = {
      type: 'user',
      id: 'u-2',
      text: 'B-updated',
      timestamp: 1001,
    } as ConversationBlock
    const result = mergeBlockById(map, incoming)
    const ids = [...result.keys()]
    expect(ids).toEqual(['u-1', 'u-2', 'u-3'])
    expect((result.get('u-2') as any).text).toBe('B-updated')
  })

  it('returns new Map reference (immutable update for React state)', () => {
    const map = new Map<string, ConversationBlock>()
    const incoming = {
      type: 'user',
      id: 'u-1',
      text: 'hello',
      timestamp: 1000,
    } as ConversationBlock
    const result = mergeBlockById(map, incoming)
    expect(result).not.toBe(map)
  })

  it('handles multiple block types', () => {
    const map = new Map<string, ConversationBlock>()
    const user = { type: 'user', id: 'u-1', text: 'hi', timestamp: 1000 } as ConversationBlock
    const assistant = {
      type: 'assistant',
      id: 'a-1',
      segments: [],
      streaming: false,
      timestamp: 1001,
    } as ConversationBlock
    const system = {
      type: 'system',
      id: 's-1',
      variant: 'session_init',
      data: {},
    } as unknown as ConversationBlock
    let result = mergeBlockById(map, user)
    result = mergeBlockById(result, assistant)
    result = mergeBlockById(result, system)
    expect(result.size).toBe(3)
    expect([...result.keys()]).toEqual(['u-1', 'a-1', 's-1'])
  })
})
