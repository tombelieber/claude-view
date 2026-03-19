import type { AssistantBlock, ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, it } from 'vitest'
import { appendPendingText } from './append-pending-text'

// ── Test helpers ──────────────────────────────────────────────────────────

function makeUserBlock(text: string): UserBlock {
  return { type: 'user', id: `u-${text}`, text, timestamp: 1000 }
}

function makeAssistantBlock(
  segments: AssistantBlock['segments'],
  overrides?: Partial<AssistantBlock>,
): AssistantBlock {
  return {
    type: 'assistant',
    id: 'a-1',
    segments,
    streaming: false,
    ...overrides,
  }
}

// ── HT-01..HT-06: appendPendingText (pure function) ─────────────────────

describe('appendPendingText', () => {
  // HT-01: empty pendingText returns blocks unchanged (identity)
  it('HT-01: empty pendingText returns blocks unchanged (referential identity)', () => {
    const blocks: ConversationBlock[] = [makeAssistantBlock([{ kind: 'text', text: 'Hello' }])]
    const result = appendPendingText(blocks, '')
    expect(result).toBe(blocks) // referential identity
  })

  // HT-02: empty blocks returns same empty array (identity)
  it('HT-02: empty blocks returns same empty array (referential identity)', () => {
    const blocks: ConversationBlock[] = []
    const result = appendPendingText(blocks, 'hello')
    expect(result).toBe(blocks) // returns original empty array
  })

  // HT-03: last block not assistant returns blocks unchanged (identity)
  it('HT-03: last block not assistant returns blocks unchanged (referential identity)', () => {
    const blocks: ConversationBlock[] = [makeUserBlock('hello')]
    const result = appendPendingText(blocks, 'hello')
    expect(result).toBe(blocks) // referential identity
  })

  // HT-04: appends to last text segment
  it('HT-04: appends pendingText to last text segment of last assistant block', () => {
    const blocks: ConversationBlock[] = [makeAssistantBlock([{ kind: 'text', text: 'Hello ' }])]
    const result = appendPendingText(blocks, 'world')
    expect(result).not.toBe(blocks) // new array
    const last = result[result.length - 1] as AssistantBlock
    expect(last.segments).toHaveLength(1)
    expect(last.segments[0]).toEqual({ kind: 'text', text: 'Hello world' })
  })

  // HT-05: no text segment creates new text segment
  it('HT-05: no text segment creates new text segment at end', () => {
    const blocks: ConversationBlock[] = [
      makeAssistantBlock([
        {
          kind: 'tool',
          execution: {
            toolName: 'bash',
            toolInput: {},
            toolUseId: 'tu-1',
            status: 'complete',
          },
        },
      ]),
    ]
    const result = appendPendingText(blocks, 'hello')
    const last = result[result.length - 1] as AssistantBlock
    expect(last.segments).toHaveLength(2)
    expect(last.segments[1]).toEqual({ kind: 'text', text: 'hello' })
  })

  // HT-06: multiple segments appends to last text only
  it('HT-06: multiple segments — appends to last text, first text unchanged', () => {
    const blocks: ConversationBlock[] = [
      makeAssistantBlock([
        { kind: 'text', text: 'A' },
        {
          kind: 'tool',
          execution: {
            toolName: 'bash',
            toolInput: {},
            toolUseId: 'tu-2',
            status: 'complete',
          },
        },
        { kind: 'text', text: 'B' },
      ]),
    ]
    const result = appendPendingText(blocks, ' appended')
    const last = result[result.length - 1] as AssistantBlock
    expect(last.segments).toHaveLength(3)
    // First text segment unchanged
    expect(last.segments[0]).toEqual({ kind: 'text', text: 'A' })
    // Last text segment has appended text
    expect(last.segments[2]).toEqual({ kind: 'text', text: 'B appended' })
  })
})
