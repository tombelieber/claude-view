import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, it } from 'vitest'
import { blocksToMessages } from './blocks-to-messages'

// 2024-01-15T10:00:00.000Z in Unix seconds (block timestamps are Unix seconds)
const TS = 1705312800

function userBlock(overrides: Partial<Extract<ConversationBlock, { type: 'user' }>> = {}) {
  return {
    type: 'user',
    id: 'u1',
    text: 'fix the bug',
    timestamp: TS,
    ...overrides,
  } as ConversationBlock
}

function assistantBlock(
  overrides: Partial<Extract<ConversationBlock, { type: 'assistant' }>> = {},
) {
  return {
    type: 'assistant',
    id: 'a1',
    segments: [],
    streaming: false,
    timestamp: TS,
    ...overrides,
  } as ConversationBlock
}

describe('blocksToMessages', () => {
  it('returns empty array for empty input', () => {
    expect(blocksToMessages([])).toEqual([])
  })

  it('converts user + assistant blocks with tool segments and thinking', () => {
    const blocks: ConversationBlock[] = [
      userBlock({
        images: [{ sourceType: 'base64', mediaType: 'image/png', data: 'abc123' }],
      }),
      assistantBlock({
        thinking: 'let me look at the file',
        segments: [
          { kind: 'text', text: 'I found the issue.' },
          {
            kind: 'tool',
            execution: {
              toolName: 'Read',
              toolInput: { file_path: '/tmp/a.ts' },
              toolUseId: 'tu1',
              status: 'complete',
            },
          },
          {
            kind: 'tool',
            execution: {
              toolName: 'Edit',
              toolInput: { file_path: '/tmp/a.ts', old_string: 'x', new_string: 'y' },
              toolUseId: 'tu2',
              status: 'complete',
            },
          },
          { kind: 'text', text: 'Fixed it.' },
        ],
      }),
    ]

    const messages = blocksToMessages(blocks)
    expect(messages).toHaveLength(2)

    const [user, assistant] = messages
    expect(user.role).toBe('user')
    expect(user.content).toBe('fix the bug')
    expect(user.timestamp).toBe('2024-01-15T10:00:00.000Z')
    expect(user.images).toEqual([{ sourceType: 'base64', mediaType: 'image/png', data: 'abc123' }])

    expect(assistant.role).toBe('assistant')
    // Text segments joined with double newline
    expect(assistant.content).toBe('I found the issue.\n\nFixed it.')
    expect(assistant.thinking).toBe('let me look at the file')
    expect(assistant.timestamp).toBe('2024-01-15T10:00:00.000Z')
    expect(assistant.tool_calls).toEqual([
      { name: 'Read', count: 1, input: { file_path: '/tmp/a.ts' } },
      {
        name: 'Edit',
        count: 1,
        input: { file_path: '/tmp/a.ts', old_string: 'x', new_string: 'y' },
      },
    ])
  })

  it('defaults user images to empty array and zero timestamp to null', () => {
    const messages = blocksToMessages([userBlock({ timestamp: 0 })])
    expect(messages).toHaveLength(1)
    expect(messages[0].images).toEqual([])
    expect(messages[0].timestamp).toBeNull()
  })

  it('converts system blocks with best-effort text fallbacks', () => {
    const blocks: ConversationBlock[] = [
      {
        type: 'system',
        id: 's1',
        variant: 'informational',
        data: { content: 'compaction complete' },
      } as ConversationBlock,
      {
        type: 'system',
        id: 's2',
        variant: 'session_status',
        data: { message: 'session resumed' },
      } as ConversationBlock,
      // No content/message → falls back to the variant name
      { type: 'system', id: 's3', variant: 'session_init', data: { foo: 1 } } as ConversationBlock,
      {
        type: 'notice',
        id: 'n1',
        variant: 'rate_limit',
        data: null,
      } as ConversationBlock,
    ]

    const messages = blocksToMessages(blocks)
    expect(messages).toHaveLength(4)
    expect(messages.every((m) => m.role === 'system')).toBe(true)
    expect(messages[0].content).toBe('compaction complete')
    expect(messages[1].content).toBe('session resumed')
    expect(messages[2].content).toBe('session_init')
    expect(messages[3].content).toBe('rate_limit')
  })

  it('skips unknown / non-conversation block types', () => {
    const blocks = [
      { type: 'progress', id: 'p1' },
      { type: 'turn_boundary', id: 'tb1' },
      { type: 'interaction', id: 'i1' },
      { type: 'team_transcript', id: 'tt1' },
      userBlock(),
    ] as ConversationBlock[]

    const messages = blocksToMessages(blocks)
    expect(messages).toHaveLength(1)
    expect(messages[0].role).toBe('user')
  })
})
