import type { SDKMessage } from '@anthropic-ai/claude-agent-sdk'
// sidecar/src/event-mapper.test.ts
import { describe, expect, it } from 'vitest'
import { mapSdkMessage } from './event-mapper.js'
import type { SessionInit, StreamDelta, TurnComplete, TurnError } from './protocol.js'

// Helper to create a minimal SDKAssistantMessage
function assistantMsg(
  content: { type: string; [k: string]: unknown }[],
  opts?: { error?: string },
): SDKMessage {
  return {
    type: 'assistant',
    message: {
      content,
      role: 'assistant',
      id: 'msg_1',
      type: 'message',
      model: 'claude-sonnet-4-20250514',
      stop_reason: null,
      stop_sequence: null,
      usage: { input_tokens: 0, output_tokens: 0 },
    },
    parent_tool_use_id: null,
    error: opts?.error,
    uuid: '00000000-0000-0000-0000-000000000001' as `${string}-${string}-${string}-${string}-${string}`,
    session_id: 'sess-1',
  } as unknown as SDKMessage
}

describe('mapSdkMessage', () => {
  describe('assistant messages', () => {
    it('maps text blocks to assistant_text', () => {
      const events = mapSdkMessage(assistantMsg([{ type: 'text', text: 'Hello world' }]))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_text',
        text: 'Hello world',
      })
    })

    it('maps tool_use blocks to tool_use_start', () => {
      const events = mapSdkMessage(
        assistantMsg([
          { type: 'tool_use', id: 'tu_1', name: 'Read', input: { file_path: '/foo' } },
        ]),
      )
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'tool_use_start',
        toolName: 'Read',
        toolUseId: 'tu_1',
      })
    })

    it('maps thinking blocks to assistant_thinking', () => {
      const events = mapSdkMessage(
        assistantMsg([{ type: 'thinking', thinking: 'Let me think...' }]),
      )
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_thinking',
        thinking: 'Let me think...',
      })
    })

    it('maps multiple content blocks to multiple events', () => {
      const events = mapSdkMessage(
        assistantMsg([
          { type: 'text', text: 'First' },
          { type: 'tool_use', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
          { type: 'text', text: 'Second' },
        ]),
      )
      expect(events).toHaveLength(3)
      expect(events[0].type).toBe('assistant_text')
      expect(events[1].type).toBe('tool_use_start')
      expect(events[2].type).toBe('assistant_text')
    })

    it('emits assistant_error when error field is set', () => {
      const events = mapSdkMessage(assistantMsg([], { error: 'rate_limit' }))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_error',
        error: 'rate_limit',
      })
    })
  })

  describe('result messages', () => {
    it('maps success result to turn_complete with real data', () => {
      const msg = {
        type: 'result',
        subtype: 'success',
        total_cost_usd: 0.0042,
        num_turns: 3,
        duration_ms: 12000,
        duration_api_ms: 8000,
        usage: { input_tokens: 1000, output_tokens: 500 },
        modelUsage: {
          'claude-sonnet-4-20250514': {
            inputTokens: 1000,
            outputTokens: 500,
            cacheReadInputTokens: 200,
            cacheCreationInputTokens: 0,
            webSearchRequests: 0,
            costUSD: 0.0042,
            contextWindow: 200000,
            maxOutputTokens: 16384,
          },
        },
        permission_denials: [],
        result: 'Done!',
        stop_reason: 'end_turn',
        fast_mode_state: 'off',
        uuid: '00000000-0000-0000-0000-000000000002',
        session_id: 'sess-1',
      } as unknown as SDKMessage

      const events = mapSdkMessage(msg)
      expect(events).toHaveLength(1)
      const e = events[0] as TurnComplete
      expect(e.type).toBe('turn_complete')
      expect(e.totalCostUsd).toBe(0.0042)
      expect(e.numTurns).toBe(3)
      expect(e.modelUsage['claude-sonnet-4-20250514'].contextWindow).toBe(200000)
      expect(e.result).toBe('Done!')
      expect(e.fastModeState).toBe('off')
    })

    it('maps error result to turn_error', () => {
      const msg = {
        type: 'result',
        subtype: 'error_max_turns',
        total_cost_usd: 0.05,
        num_turns: 10,
        duration_ms: 60000,
        errors: ['Max turns reached'],
        permission_denials: [{ tool_name: 'Bash', tool_use_id: 'tu_1', tool_input: {} }],
        usage: {},
        modelUsage: {},
        uuid: 'u2',
        session_id: 's1',
      } as unknown as SDKMessage

      const events = mapSdkMessage(msg)
      expect(events).toHaveLength(1)
      expect(events[0].type).toBe('turn_error')
      expect((events[0] as TurnError).subtype).toBe('error_max_turns')
      expect((events[0] as TurnError).errors).toEqual(['Max turns reached'])
      expect((events[0] as TurnError).permissionDenials).toHaveLength(1)
    })
  })

  describe('user messages (tool results)', () => {
    it('maps tool_result blocks to tool_use_result', () => {
      const msg = {
        type: 'user',
        message: {
          role: 'user',
          content: [
            {
              type: 'tool_result',
              tool_use_id: 'tu_1',
              content: 'file contents here',
              is_error: false,
            },
          ],
        },
        parent_tool_use_id: null,
        uuid: 'u3',
        session_id: 's1',
      } as unknown as SDKMessage

      const events = mapSdkMessage(msg)
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'tool_use_result',
        toolUseId: 'tu_1',
        output: 'file contents here',
        isError: false,
        isReplay: false,
      })
    })
  })

  describe('system messages', () => {
    it('maps init to session_init', () => {
      const msg = {
        type: 'system',
        subtype: 'init',
        tools: ['Read', 'Edit', 'Bash'],
        model: 'claude-sonnet-4-20250514',
        mcp_servers: [{ name: 'context7', status: 'connected' }],
        permissionMode: 'default',
        slash_commands: ['/help', '/clear'],
        claude_code_version: '1.2.3',
        cwd: '/home/user/project',
        agents: ['code-reviewer'],
        skills: ['commit'],
        output_style: 'normal',
        uuid: 'u4',
        session_id: 's1',
      } as unknown as SDKMessage

      const events = mapSdkMessage(msg)
      expect(events).toHaveLength(1)
      expect(events[0].type).toBe('session_init')
      const e = events[0] as SessionInit
      expect(e.tools).toEqual(['Read', 'Edit', 'Bash'])
      expect(e.model).toBe('claude-sonnet-4-20250514')
      expect(e.cwd).toBe('/home/user/project')
    })

    it('maps unknown system subtype to unknown_sdk_event', () => {
      const msg = {
        type: 'system',
        subtype: 'future_thing',
        uuid: 'u5',
        session_id: 's1',
      } as unknown as SDKMessage
      const events = mapSdkMessage(msg)
      expect(events[0].type).toBe('unknown_sdk_event')
    })
  })

  describe('stream_event (StreamDelta enrichment)', () => {
    function streamMsg(event: Record<string, unknown>, uuid?: string): SDKMessage {
      return {
        type: 'stream_event',
        event,
        parent_tool_use_id: null,
        uuid: uuid ?? '00000000-0000-0000-0000-000000000010',
        session_id: 'sess-1',
      } as unknown as SDKMessage
    }

    it('extracts textDelta from content_block_delta with text_delta', () => {
      const events = mapSdkMessage(
        streamMsg({
          type: 'content_block_delta',
          delta: { type: 'text_delta', text: 'hello ' },
        }),
      )
      expect(events).toHaveLength(1)
      const e = events[0] as StreamDelta
      expect(e.type).toBe('stream_delta')
      expect(e.deltaType).toBe('content_block_delta')
      expect(e.textDelta).toBe('hello ')
      expect(e.thinkingDelta).toBeUndefined()
      expect(e.toolInputDelta).toBeUndefined()
    })

    it('extracts thinkingDelta from content_block_delta with thinking_delta', () => {
      const events = mapSdkMessage(
        streamMsg({
          type: 'content_block_delta',
          delta: { type: 'thinking_delta', thinking: 'Let me consider...' },
        }),
      )
      const e = events[0] as StreamDelta
      expect(e.deltaType).toBe('content_block_delta')
      expect(e.thinkingDelta).toBe('Let me consider...')
      expect(e.textDelta).toBeUndefined()
      expect(e.toolInputDelta).toBeUndefined()
    })

    it('extracts toolInputDelta from content_block_delta with input_json_delta', () => {
      const events = mapSdkMessage(
        streamMsg({
          type: 'content_block_delta',
          delta: { type: 'input_json_delta', partial_json: '{"file_' },
        }),
      )
      const e = events[0] as StreamDelta
      expect(e.deltaType).toBe('content_block_delta')
      expect(e.toolInputDelta).toBe('{"file_')
      expect(e.textDelta).toBeUndefined()
      expect(e.thinkingDelta).toBeUndefined()
    })

    it('handles content_block_start without extracting deltas', () => {
      const events = mapSdkMessage(
        streamMsg({
          type: 'content_block_start',
          content_block: { type: 'text', text: '' },
        }),
      )
      const e = events[0] as StreamDelta
      expect(e.deltaType).toBe('content_block_start')
      expect(e.textDelta).toBeUndefined()
      expect(e.thinkingDelta).toBeUndefined()
      expect(e.toolInputDelta).toBeUndefined()
    })

    it('handles content_block_stop without extracting deltas', () => {
      const events = mapSdkMessage(streamMsg({ type: 'content_block_stop', index: 0 }))
      const e = events[0] as StreamDelta
      expect(e.deltaType).toBe('content_block_stop')
      expect(e.textDelta).toBeUndefined()
    })

    it('uses empty string messageId when uuid is missing', () => {
      const msg = {
        type: 'stream_event',
        event: { type: 'message_start' },
        parent_tool_use_id: null,
        session_id: 'sess-1',
      } as unknown as SDKMessage
      const events = mapSdkMessage(msg)
      const e = events[0] as StreamDelta
      expect(e.messageId).toBe('')
      expect(e.deltaType).toBe('message_start')
    })

    it('preserves raw event object in event field', () => {
      const rawEvent = { type: 'content_block_delta', delta: { type: 'text_delta', text: 'x' } }
      const events = mapSdkMessage(streamMsg(rawEvent))
      const e = events[0] as StreamDelta
      expect(e.event).toEqual(rawEvent)
    })
  })

  describe('session_init sessionId', () => {
    it('includes sessionId from raw SDK message', () => {
      const msg = {
        type: 'system',
        subtype: 'init',
        tools: [],
        model: 'claude-sonnet-4-20250514',
        mcp_servers: [],
        permissionMode: 'default',
        slash_commands: [],
        claude_code_version: '1.0.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        output_style: 'normal',
        uuid: 'u1',
        session_id: 'sess-abc-123',
      } as unknown as SDKMessage
      const events = mapSdkMessage(msg)
      const e = events[0] as SessionInit
      expect(e.sessionId).toBe('sess-abc-123')
    })
  })

  describe('session_init capabilities', () => {
    it('includes capabilities array with all 13 V1 control methods', () => {
      const msg = {
        type: 'system',
        subtype: 'init',
        tools: [],
        model: 'claude-sonnet-4-20250514',
        mcp_servers: [],
        permissionMode: 'default',
        slash_commands: [],
        claude_code_version: '1.0.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        output_style: '',
        session_id: 'sess-1',
      } as unknown as SDKMessage
      const events = mapSdkMessage(msg)
      const init = events[0] as SessionInit
      expect(init.type).toBe('session_init')
      expect(init.capabilities).toContain('interrupt')
      expect(init.capabilities).toContain('rewind_files')
      expect(init.capabilities).toContain('set_model')
      expect(init.capabilities).toContain('query_mcp_status')
      expect(init.capabilities).toHaveLength(13)
    })
  })

  describe('unknown message types', () => {
    it('maps completely unknown type to unknown_sdk_event', () => {
      const msg = { type: 'brand_new_thing', data: 42 } as unknown as SDKMessage
      const events = mapSdkMessage(msg)
      expect(events[0]).toMatchObject({
        type: 'unknown_sdk_event',
        sdkType: 'brand_new_thing',
      })
    })
  })
})
