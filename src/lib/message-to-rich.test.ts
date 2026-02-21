import { describe, it, expect } from 'vitest'
import { messagesToRichMessages } from './message-to-rich'
import type { Message } from '../types/generated'

function makeMsg(overrides: Partial<Message>): Message {
  return { role: 'user', content: '', ...overrides } as Message
}

describe('messagesToRichMessages', () => {
  describe('existing types (regression)', () => {
    it('converts user messages', () => {
      const result = messagesToRichMessages([makeMsg({ role: 'user', content: 'hello' })])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'user', content: 'hello' })
    })

    it('converts assistant messages with thinking', () => {
      const result = messagesToRichMessages([
        makeMsg({ role: 'assistant', content: 'reply', thinking: 'let me think' }),
      ])
      expect(result).toHaveLength(2)
      expect(result[0]).toMatchObject({ type: 'thinking', content: 'let me think' })
      expect(result[1]).toMatchObject({ type: 'assistant', content: 'reply' })
    })

    it('converts tool_use with tool_calls', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'tool_use',
          content: '',
          tool_calls: [{ name: 'Read', input: { file_path: '/foo' } }],
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'tool_use', name: 'Read', category: 'builtin' })
    })

    it('converts tool_result', () => {
      const result = messagesToRichMessages([
        makeMsg({ role: 'tool_result', content: 'file contents here' }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'tool_result', content: 'file contents here' })
    })
  })

  describe('system messages (NEW — was flattened to assistant)', () => {
    it('emits type system with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'system',
          content: 'turn ended',
          metadata: { type: 'turn_duration', durationMs: 1500 },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'system',
        content: 'turn ended',
        metadata: { type: 'turn_duration', durationMs: 1500 },
      })
    })

    it('emits system even with empty content', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'system',
          content: '',
          metadata: { type: 'api_error', error: { code: 'overloaded' } },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0].type).toBe('system')
      expect(result[0].metadata).toEqual({ type: 'api_error', error: { code: 'overloaded' } })
    })
  })

  describe('progress messages (NEW — was skipped)', () => {
    it('emits type progress with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'progress',
          content: '',
          metadata: { type: 'agent_progress', agentId: 'abc', model: 'opus' },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'progress',
        metadata: { type: 'agent_progress', agentId: 'abc', model: 'opus' },
      })
    })
  })

  describe('summary messages (NEW — was skipped)', () => {
    it('emits type summary with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'summary',
          content: 'Session summary text',
          metadata: { summary: 'Session summary text', leafUuid: 'uuid-123' },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'summary',
        content: 'Session summary text',
        metadata: { summary: 'Session summary text', leafUuid: 'uuid-123' },
      })
    })
  })
})
