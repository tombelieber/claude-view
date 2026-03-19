// sidecar/src/blocks-no-duplication.test.ts
// Regression test: blocks_snapshot must never contain duplicate messageIds after
// connect/reconnect cycles. The StreamAccumulator is reset on reconnect (new instance),
// so replaying the same events must not double-add blocks.
//
// Uses the REAL SessionRegistry.emitSequenced + StreamAccumulator pipeline.

import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import type { ServerEvent } from './protocol.js'
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
    permissions: {
      resolvePermission: vi.fn(),
      resolveQuestion: vi.fn(),
      resolvePlan: vi.fn(),
      resolveElicitation: vi.fn(),
      drainInteractive: vi.fn(),
      drainAll: vi.fn(),
    } as unknown as ControlSession['permissions'],
    permissionMode: 'default',
    wsClients: new Set(),
    lastSessionInit: null,
    accumulator: new StreamAccumulator(),
  }
}

/** Feed a complete conversation (user + assistant) into the registry via emitSequenced. */
function feedConversation(
  registry: SessionRegistry,
  cs: ControlSession,
  opts: { userText: string; assistantText: string; messageId: string },
): void {
  const sessionInit: ServerEvent = {
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
  }
  registry.emitSequenced(cs, sessionInit)

  registry.emitSequenced(cs, {
    type: 'user_message_echo',
    content: opts.userText,
    timestamp: Date.now() / 1000,
  } satisfies ServerEvent)

  registry.emitSequenced(cs, {
    type: 'assistant_text',
    text: opts.assistantText,
    messageId: opts.messageId,
    parentToolUseId: null,
  } satisfies ServerEvent)

  registry.emitSequenced(cs, {
    type: 'turn_complete',
    totalCostUsd: 0.001,
    numTurns: 1,
    durationMs: 100,
    durationApiMs: 90,
    usage: {},
    modelUsage: {},
    permissionDenials: [],
    result: opts.assistantText,
    stopReason: 'end_turn',
  } satisfies ServerEvent)
}

describe('blocks_snapshot deduplication regression', () => {
  it('blocks_snapshot has no duplicate messageIds after connect/reconnect cycle', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-dedup', 'sess-dedup')
    registry.register(cs)

    const messageId = 'msg-dedup-1'

    // First connection: feed the conversation
    feedConversation(registry, cs, {
      userText: 'Hello',
      assistantText: 'Hi there!',
      messageId,
    })

    // Verify after first connection: no duplicate messageIds in blocks
    const blocksAfterFirst = cs.accumulator.getBlocks()
    const idsAfterFirst = blocksAfterFirst.map((b) => b.id)
    const uniqueIdsAfterFirst = new Set(idsAfterFirst)
    expect(idsAfterFirst.length).toBe(uniqueIdsAfterFirst.size)

    // Reconnect: reset the accumulator (simulates a fresh WS connection)
    cs.accumulator.reset()

    // Replay the same events (reconnect replay)
    feedConversation(registry, cs, {
      userText: 'Hello',
      assistantText: 'Hi there!',
      messageId,
    })

    // After reconnect replay: still no duplicate messageIds
    const blocksAfterReconnect = cs.accumulator.getBlocks()
    const idsAfterReconnect = blocksAfterReconnect.map((b) => b.id)
    const uniqueIdsAfterReconnect = new Set(idsAfterReconnect)
    expect(idsAfterReconnect.length).toBe(uniqueIdsAfterReconnect.size)

    // Sanity: accumulator should have the same number of blocks as on first connect
    expect(blocksAfterReconnect.length).toBe(blocksAfterFirst.length)
  })

  it('assistant text appears exactly once in committed blocks', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-once', 'sess-once')
    registry.register(cs)

    const messageId = 'msg-once-1'
    const assistantText = 'The answer is 42.'

    // session_init
    registry.emitSequenced(cs, {
      type: 'session_init',
      tools: [],
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

    // user echo
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'What is the answer?',
      timestamp: Date.now() / 1000,
    } satisfies ServerEvent)

    // content_block_start — structural event, creates assistant skeleton
    registry.emitSequenced(cs, {
      type: 'stream_delta',
      event: {},
      messageId,
      deltaType: 'content_block_start',
    } satisfies ServerEvent)

    // content_block_delta events — must be filtered, NOT added to accumulator
    for (const char of assistantText) {
      registry.emitSequenced(cs, {
        type: 'stream_delta',
        event: {},
        messageId,
        deltaType: 'content_block_delta',
        textDelta: char,
      } satisfies ServerEvent)
    }

    // assistant_text — the SDK's final authoritative text
    registry.emitSequenced(cs, {
      type: 'assistant_text',
      text: assistantText,
      messageId,
      parentToolUseId: null,
    } satisfies ServerEvent)

    // turn_complete — finalizes the assistant block
    registry.emitSequenced(cs, {
      type: 'turn_complete',
      totalCostUsd: 0.001,
      numTurns: 1,
      durationMs: 150,
      durationApiMs: 130,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: assistantText,
      stopReason: 'end_turn',
    } satisfies ServerEvent)

    // Collect all assistant blocks
    const blocks = cs.accumulator.getBlocks()
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')

    // Must have exactly one assistant block
    expect(assistantBlocks.length).toBe(1)

    // That block must have the text exactly once (not doubled)
    const assistantBlock = assistantBlocks[0]
    if (!assistantBlock || assistantBlock.type !== 'assistant') {
      throw new Error('Expected exactly one assistant block')
    }
    const textSegments = assistantBlock.segments.filter((s) => s.kind === 'text')
    const fullText = textSegments.map((s) => (s.kind === 'text' ? s.text : '')).join('')

    expect(fullText).toBe(assistantText)
    // Explicitly confirm no doubling
    expect(fullText).not.toBe(assistantText + assistantText)
  })
})
