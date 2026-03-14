import { describe, expect, it } from 'vitest'
import type { AssistantBlock, SystemBlock, UserBlock } from '../types/blocks'
import { historyToBlocks } from './history-to-blocks'

// Local interface for the historical message shape (matches generated Message type)
interface Msg {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress'
  content: string
  uuid?: string | null
  thinking?: string | null
  tool_calls?: Array<{
    name: string
    count: number
    input?: unknown
    category?: string | null
  }> | null
  timestamp?: string | null
}

describe('historyToBlocks', () => {
  it('maps user message to UserBlock', () => {
    const blocks = historyToBlocks([{ role: 'user', content: 'Hello!', uuid: 'u1' } as Msg])
    expect(blocks).toHaveLength(1)
    const block = blocks[0] as UserBlock
    expect(block.type).toBe('user')
    expect(block.text).toBe('Hello!')
    expect(block.status).toBe('sent')
  })

  it('maps assistant message to AssistantBlock with text segment', () => {
    const blocks = historyToBlocks([
      { role: 'assistant', content: 'Sure, I can help.', uuid: 'a1' } as Msg,
    ])
    expect(blocks).toHaveLength(1)
    const block = blocks[0] as AssistantBlock
    expect(block.type).toBe('assistant')
    expect(block.streaming).toBe(false)
    expect(block.segments).toHaveLength(1)
    expect(block.segments[0]).toEqual({
      kind: 'text',
      text: 'Sure, I can help.',
      parentToolUseId: null,
    })
  })

  it('maps assistant thinking to AssistantBlock.thinking', () => {
    const blocks = historyToBlocks([
      { role: 'assistant', content: 'Answer', thinking: 'Let me think...', uuid: 'a1' } as Msg,
    ])
    const block = blocks[0] as AssistantBlock
    expect(block.thinking).toBe('Let me think...')
  })

  it('maps tool_calls to tool segments in AssistantBlock', () => {
    const blocks = historyToBlocks([
      {
        role: 'assistant',
        content: 'I will read the file.',
        uuid: 'a1',
        tool_calls: [{ name: 'Read', count: 1, input: { file: 'main.rs' } }],
      } as Msg,
    ])
    const block = blocks[0] as AssistantBlock
    // text segment + tool segment
    expect(block.segments).toHaveLength(2)
    expect(block.segments[0]).toEqual({
      kind: 'text',
      text: 'I will read the file.',
      parentToolUseId: null,
    })
    const toolSeg = block.segments[1]
    expect(toolSeg.kind).toBe('tool')
    if (toolSeg.kind === 'tool') {
      expect(toolSeg.execution.toolName).toBe('Read')
      expect(toolSeg.execution.status).toBe('complete')
    }
  })

  it('multiple tool_calls produce multiple tool segments', () => {
    const blocks = historyToBlocks([
      {
        role: 'assistant',
        content: '',
        uuid: 'a1',
        tool_calls: [
          { name: 'Read', count: 1, input: { file: 'a.ts' } },
          { name: 'Write', count: 1, input: { file: 'b.ts' } },
        ],
      } as Msg,
    ])
    const block = blocks[0] as AssistantBlock
    const toolSegs = block.segments.filter((s) => s.kind === 'tool')
    expect(toolSegs).toHaveLength(2)
    if (toolSegs[0].kind === 'tool') expect(toolSegs[0].execution.toolName).toBe('Read')
    if (toolSegs[1].kind === 'tool') expect(toolSegs[1].execution.toolName).toBe('Write')
  })

  it('skips text segment when content is empty', () => {
    const blocks = historyToBlocks([
      {
        role: 'assistant',
        content: '',
        uuid: 'a1',
        tool_calls: [{ name: 'Bash', count: 1 }],
      } as Msg,
    ])
    const block = blocks[0] as AssistantBlock
    const textSegs = block.segments.filter((s) => s.kind === 'text')
    expect(textSegs).toHaveLength(0)
  })

  it('maps multiple messages to multiple blocks', () => {
    const blocks = historyToBlocks([
      { role: 'user', content: 'Hello', uuid: 'u1' } as Msg,
      { role: 'assistant', content: 'Hi there!', uuid: 'a1' } as Msg,
      { role: 'user', content: 'Thanks', uuid: 'u2' } as Msg,
    ])
    expect(blocks).toHaveLength(3)
    expect(blocks[0].type).toBe('user')
    expect(blocks[1].type).toBe('assistant')
    expect(blocks[2].type).toBe('user')
  })

  it('handles empty message array', () => {
    const blocks = historyToBlocks([])
    expect(blocks).toHaveLength(0)
  })

  it('skips tool_use and tool_result messages (already embedded in assistant)', () => {
    const blocks = historyToBlocks([
      { role: 'user', content: 'Help', uuid: 'u1' } as Msg,
      { role: 'tool_use', content: 'Read file', uuid: 't1' } as Msg,
      { role: 'tool_result', content: 'file contents', uuid: 'tr1' } as Msg,
      { role: 'assistant', content: 'Done', uuid: 'a1' } as Msg,
    ])
    // tool_use and tool_result are skipped — embedded in tool_calls of assistant
    expect(blocks.filter((b) => b.type === 'user')).toHaveLength(1)
    expect(blocks.filter((b) => b.type === 'assistant')).toHaveLength(1)
  })

  it('assigns unique ids to each block', () => {
    const blocks = historyToBlocks([
      { role: 'user', content: 'A', uuid: 'u1' } as Msg,
      { role: 'assistant', content: 'B', uuid: 'a1' } as Msg,
    ])
    const ids = blocks.map((b) => b.id)
    expect(new Set(ids).size).toBe(2)
  })

  it('propagates raw_json to UserBlock.rawJson', () => {
    const raw = { type: 'user', uuid: 'u1', extra_field: 42 }
    const blocks = historyToBlocks([
      { role: 'user', content: 'hi', uuid: 'u1', raw_json: raw } as any,
    ])
    const block = blocks[0] as UserBlock
    expect(block.rawJson).toEqual(raw)
  })

  it('propagates raw_json to AssistantBlock.rawJson', () => {
    const raw = { type: 'assistant', uuid: 'a1', message: {} }
    const blocks = historyToBlocks([
      { role: 'assistant', content: 'Sure', uuid: 'a1', raw_json: raw } as any,
    ])
    const block = blocks[0] as AssistantBlock
    expect(block.rawJson).toEqual(raw)
  })

  it('converts system message to SystemBlock with rawJson', () => {
    const raw = { type: 'system', subtype: 'api_error', error: { message: 'rate limited' } }
    const blocks = historyToBlocks([
      {
        role: 'system',
        content: 'Error',
        uuid: 's1',
        metadata: { subtype: 'api_error' },
        raw_json: raw,
      } as any,
    ])
    expect(blocks).toHaveLength(1)
    const block = blocks[0] as SystemBlock
    expect(block.type).toBe('system')
    expect(block.variant).toBe('unknown')
    expect(block.rawJson).toEqual(raw)
  })

  it('converts progress message to SystemBlock with rawJson', () => {
    const raw = { type: 'progress', data: { type: 'bash_progress' } }
    const blocks = historyToBlocks([
      { role: 'progress', content: 'Running...', uuid: 'p1', raw_json: raw } as any,
    ])
    expect(blocks).toHaveLength(1)
    const block = blocks[0] as SystemBlock
    expect(block.type).toBe('system')
    expect(block.variant).toBe('unknown')
    expect(block.rawJson).toEqual(raw)
  })
})
