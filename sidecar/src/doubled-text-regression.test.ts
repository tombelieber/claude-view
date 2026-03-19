// sidecar/src/doubled-text-regression.test.ts
// Regression test: SDK V1 with includePartialMessages yields BOTH stream_delta
// AND assistant_text for the same content. If both go through accumulator.push(),
// text gets doubled (stream_delta appends chars, then assistant_text appends full text).
//
// This test uses the REAL SessionRegistry.emitSequenced + StreamAccumulator pipeline.

import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import type { SequencedEvent, ServerEvent } from './protocol.js'
import { RingBuffer } from './ring-buffer.js'
import type { ControlSession } from './session-registry.js'
import { SessionRegistry } from './session-registry.js'
import { StreamAccumulator } from './stream-accumulator.js'

function buildRealSession(controlId: string, sessionId: string): ControlSession {
  return {
    controlId,
    sessionId,
    model: 'claude-sonnet-4-20250514',
    query: {
      supportedModels: vi.fn().mockResolvedValue([]),
      supportedCommands: vi.fn().mockResolvedValue([]),
      supportedAgents: vi.fn().mockResolvedValue([]),
      mcpServerStatus: vi.fn().mockResolvedValue([]),
      accountInfo: vi.fn().mockResolvedValue({}),
      setMcpServers: vi.fn().mockResolvedValue({ ok: true }),
      rewindFiles: vi.fn().mockResolvedValue({ files: [] }),
      interrupt: vi.fn().mockResolvedValue(undefined),
      close: vi.fn(),
    } as unknown as ControlSession['query'],
    bridge: {
      close: vi.fn(),
    } as unknown as ControlSession['bridge'],
    abort: new AbortController(),
    state: 'active',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter: new EventEmitter(),
    eventBuffer: new RingBuffer<{ seq: number; msg: SequencedEvent }>(1000),
    nextSeq: 0,
    permissions: {
      resolvePermission: vi.fn(),
      resolveQuestion: vi.fn(),
      resolvePlan: vi.fn(),
      resolveElicitation: vi.fn(),
      drainInteractive: vi.fn(),
      drainAll: vi.fn(),
    } as unknown as ControlSession['permissions'],
    permissionMode: 'default',
    activeWs: null,
    accumulator: new StreamAccumulator(),
  }
}

describe('Doubled text regression: stream_delta + assistant_text must NOT double text', () => {
  it('accumulator produces "1+1=2" not "1+1=21+1=2" when both stream_delta and assistant_text are pushed', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-dbl', 'sess-dbl')
    registry.register(cs)

    const messageId = 'msg-dbl-1'

    // 1. session_init — unlocks the accumulator gate
    registry.emitSequenced(cs, {
      type: 'session_init',
      tools: ['Read'],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'text',
    } satisfies ServerEvent)

    // 2. stream_delta: content_block_start
    registry.emitSequenced(cs, {
      type: 'stream_delta',
      event: {},
      messageId,
      deltaType: 'content_block_start',
    } satisfies ServerEvent)

    // 3-7. stream_delta: content_block_delta — character by character
    for (const char of ['1', '+', '1', '=', '2']) {
      registry.emitSequenced(cs, {
        type: 'stream_delta',
        event: {},
        messageId,
        deltaType: 'content_block_delta',
        textDelta: char,
      } satisfies ServerEvent)
    }

    // 8. assistant_text — the SDK's final message with complete text
    registry.emitSequenced(cs, {
      type: 'assistant_text',
      text: '1+1=2',
      messageId,
      parentToolUseId: null,
    } satisfies ServerEvent)

    // 9. turn_complete
    registry.emitSequenced(cs, {
      type: 'turn_complete',
      totalCostUsd: 0.001,
      numTurns: 1,
      durationMs: 200,
      durationApiMs: 180,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: '1+1=2',
      stopReason: 'end_turn',
    } satisfies ServerEvent)

    // Verify: get the finalized blocks
    const blocks = cs.accumulator.getBlocks()
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')
    expect(assistantBlocks.length).toBeGreaterThanOrEqual(1)

    // Find text content in the last assistant block
    const lastAssistant = assistantBlocks.at(-1)
    if (!lastAssistant || lastAssistant.type !== 'assistant') {
      throw new Error('Expected at least one assistant block')
    }

    const textSegments = lastAssistant.segments.filter((s) => s.kind === 'text')
    const fullText = textSegments.map((s) => (s.kind === 'text' ? s.text : '')).join('')

    // CRITICAL: Text must be exactly '1+1=2', NOT '1+1=21+1=2'
    expect(fullText).toBe('1+1=2')
    expect(fullText).not.toBe('1+1=21+1=2')
  })
})
