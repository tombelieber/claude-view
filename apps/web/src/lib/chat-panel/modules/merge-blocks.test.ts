import { describe, expect, test } from 'vitest'
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { mergeBlocks } from './merge-blocks'

function userBlock(id: string, text: string): ConversationBlock {
  return { type: 'user', id, text, timestamp: 1000 } as ConversationBlock
}

function assistantBlock(id: string, text: string): ConversationBlock {
  return {
    type: 'assistant',
    id,
    segments: [{ kind: 'text', text }],
    streaming: false,
    timestamp: 1000,
  } as ConversationBlock
}

function systemBlock(id: string, variant: string): ConversationBlock {
  return { type: 'system', id, variant, data: {} } as ConversationBlock
}

describe('mergeBlocks', () => {
  test('returns incoming when existing is empty', () => {
    const incoming = [userBlock('u1', 'hello')]
    expect(mergeBlocks([], incoming)).toEqual(incoming)
  })

  test('deduplicates by ID (same source)', () => {
    const existing = [userBlock('u1', 'hello'), assistantBlock('a1', 'hi')]
    const incoming = [userBlock('u1', 'hello'), assistantBlock('a1', 'hi updated')]
    const result = mergeBlocks(existing, incoming)
    // u1 and a1 are in incoming → existing versions dropped
    expect(result).toHaveLength(2)
    expect(result[0].id).toBe('u1')
    expect(result[1].id).toBe('a1')
  })

  test('preserves non-overlapping history blocks on resume', () => {
    const existing = [
      userBlock('old-u', 'previous turn'),
      assistantBlock('old-a', 'previous answer'),
    ]
    const incoming = [userBlock('new-u', 'new question'), assistantBlock('new-a', 'new answer')]
    const result = mergeBlocks(existing, incoming)
    // All 4 blocks should be present — different content, different IDs
    expect(result).toHaveLength(4)
    expect(result.map((b) => b.id)).toEqual(['old-u', 'old-a', 'new-u', 'new-a'])
  })

  test('deduplicates user blocks by text across sources (different IDs, same content)', () => {
    // History produces UUID-based IDs, sidecar produces counter-based IDs
    const existing = [userBlock('6065760b-uuid', '1+1')]
    const incoming = [userBlock('user-1', '1+1')]
    const result = mergeBlocks(existing, incoming)
    // Same text → existing deduplicated, only incoming kept
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('user-1')
  })

  test('deduplicates assistant blocks by first segment text across sources', () => {
    // History uses msg_* ID, sidecar uses SDK UUID
    const existing = [assistantBlock('msg_01HtE1SM', '1 + 1 = **2**')]
    const incoming = [assistantBlock('deded6bf-uuid', '1 + 1 = **2**')]
    const result = mergeBlocks(existing, incoming)
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('deded6bf-uuid')
  })

  test('full cross-source merge: history + sidecar with no duplicates', () => {
    // Simulates the race: HISTORY_OK loads first, then BLOCKS_SNAPSHOT arrives
    const history = [
      systemBlock('sys-1', 'queue_operation'),
      userBlock('uuid-user', '1+1'),
      assistantBlock('msg_01Ht', '1 + 1 = **2**'),
    ]
    const sidecar = [
      userBlock('user-1', '1+1'),
      systemBlock('block-1', 'session_init'),
      assistantBlock('sdk-uuid', '1 + 1 = **2**'),
    ]
    const result = mergeBlocks(history, sidecar)
    // sys-1 preserved (no fingerprint match), uuid-user and msg_01Ht deduplicated by content
    expect(result).toHaveLength(4)
    const types = result.map((b) => b.type)
    expect(types.filter((t) => t === 'user')).toHaveLength(1)
    expect(types.filter((t) => t === 'assistant')).toHaveLength(1)
    expect(types.filter((t) => t === 'system')).toHaveLength(2)
  })

  test('does NOT fingerprint system/progress blocks (only dedup by ID)', () => {
    const existing = [systemBlock('sys-1', 'hook_event')]
    const incoming = [systemBlock('block-1', 'hook_event')]
    const result = mergeBlocks(existing, incoming)
    // Same variant but different IDs and no fingerprint → both kept
    expect(result).toHaveLength(2)
  })

  test('handles assistant blocks with tool segments (no text fingerprint)', () => {
    const toolAssistant: ConversationBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [
        {
          kind: 'tool',
          execution: {
            toolName: 'Read',
            toolInput: {},
            toolUseId: 'tu1',
            status: 'complete',
          },
        },
      ],
      streaming: false,
    } as ConversationBlock
    const existing = [toolAssistant]
    const incoming = [userBlock('u1', 'hello')]
    const result = mergeBlocks(existing, incoming)
    // Tool-only assistant has no text fingerprint → preserved by ID logic
    expect(result).toHaveLength(2)
  })
})
