import { describe, it, expect } from 'vitest'
import { pairMessages } from './use-paired-messages'
import type { RichMessage } from '../components/live/RichPane'

function msg(type: RichMessage['type'], overrides: Partial<RichMessage> = {}): RichMessage {
  return { type, content: '', ts: Date.now() / 1000, ...overrides }
}

describe('pairMessages', () => {
  it('wraps non-tool messages as kind=message', () => {
    const input = [msg('user'), msg('assistant')]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('message')
    expect(result[1].kind).toBe('message')
  })

  it('pairs tool_use with its subsequent tool_result', () => {
    const input = [
      msg('tool_use', { name: 'Edit', ts: 100 }),
      msg('tool_result', { content: 'ok', ts: 101 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(1)
    expect(result[0].kind).toBe('tool_pair')
    if (result[0].kind === 'tool_pair') {
      expect(result[0].toolUse.name).toBe('Edit')
      expect(result[0].toolResult?.content).toBe('ok')
    }
  })

  it('skips thinking messages between tool_use and tool_result', () => {
    const input = [
      msg('tool_use', { name: 'Bash', ts: 100 }),
      msg('thinking', { content: 'hmm', ts: 100.5 }),
      msg('tool_result', { content: 'done', ts: 101 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('message') // thinking
    expect(result[1].kind).toBe('tool_pair')
  })

  it('emits unpaired tool_use with null toolResult when no result follows', () => {
    const input = [
      msg('tool_use', { name: 'Read', ts: 100 }),
      msg('user', { content: 'hello', ts: 102 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('tool_pair')
    if (result[0].kind === 'tool_pair') {
      expect(result[0].toolResult).toBeNull()
    }
    expect(result[1].kind).toBe('message')
  })

  it('handles consecutive tool pairs', () => {
    const input = [
      msg('tool_use', { name: 'Read', ts: 100 }),
      msg('tool_result', { content: 'file content', ts: 101 }),
      msg('tool_use', { name: 'Edit', ts: 102 }),
      msg('tool_result', { content: 'ok', ts: 103 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('tool_pair')
    expect(result[1].kind).toBe('tool_pair')
  })

  it('handles empty input', () => {
    expect(pairMessages([])).toEqual([])
  })

  it('handles tool_result without preceding tool_use as standalone', () => {
    const input = [msg('tool_result', { content: 'orphan' })]
    const result = pairMessages(input)
    expect(result).toHaveLength(1)
    expect(result[0].kind).toBe('message')
  })
})
