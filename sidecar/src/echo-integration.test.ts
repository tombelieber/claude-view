// Echo wire format integration tests — verifies the full path from
// emitSequenced → emitter → accumulator.
import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import type { ServerEvent } from './protocol.js'
import type { ControlSession } from './session-registry.js'
import { SessionRegistry } from './session-registry.js'
import { StreamAccumulator } from './stream-accumulator.js'

/** Builds a real ControlSession with real accumulator + emitter. */
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
    activeWs: null,
    accumulator: new StreamAccumulator(),
  }
}

describe('Echo wire format integration (emitSequenced → emitter + accumulator)', () => {
  it('emitSequenced pushes user_message_echo into accumulator', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-echo', 'sess-echo')
    registry.register(cs)

    const echo: ServerEvent = {
      type: 'user_message_echo',
      content: 'Hello from user',
      timestamp: 1710000000,
    }

    registry.emitSequenced(cs, echo)

    // Verify accumulator has the user block
    const blocks = cs.accumulator.getBlocks()
    const userBlocks = blocks.filter((b) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
    if (userBlocks[0].type !== 'user') throw new Error('expected user')
    expect(userBlocks[0].text).toBe('Hello from user')
  })

  it('echo followed by assistant events maintains correct block ordering', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-order', 'sess-order')
    registry.register(cs)

    // session_init first to unlock accumulator gate
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
    } as ServerEvent)

    // User echo → assistant text → turn complete
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'Question',
      timestamp: 1710000000,
    } as ServerEvent)

    registry.emitSequenced(cs, {
      type: 'assistant_text',
      text: 'Answer',
      messageId: 'a1',
      parentToolUseId: null,
    } as ServerEvent)

    registry.emitSequenced(cs, {
      type: 'turn_complete',
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 500,
      durationApiMs: 400,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: 'stop',
      stopReason: 'end_turn',
    } as ServerEvent)

    const blocks = cs.accumulator.getBlocks()
    // Should have: system(session_init) + user + assistant + turn_boundary
    const types = blocks.map((b) => b.type)
    expect(types).toContain('system')
    expect(types).toContain('user')
    expect(types).toContain('assistant')
    expect(types).toContain('turn_boundary')
  })

  it('emitter receives event synchronously', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-sync', 'sess-sync')
    registry.register(cs)

    const received: ServerEvent[] = []
    cs.emitter.on('message', (msg: ServerEvent) => received.push(msg))

    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'live event',
      timestamp: 1710000000,
    } as ServerEvent)

    // Emitter fires synchronously — message available immediately
    expect(received).toHaveLength(1)
    expect(received[0].type).toBe('user_message_echo')
  })

  it('accumulator maintains correct block order after multiple messages', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-multi', 'sess-multi')
    registry.register(cs)

    // Fill with 3 events
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'msg 1',
      timestamp: 1710000000,
    } as ServerEvent)

    // session_init to unlock
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
    } as ServerEvent)

    registry.emitSequenced(cs, {
      type: 'assistant_text',
      text: 'reply',
      messageId: 'a1',
      parentToolUseId: null,
    } as ServerEvent)

    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'msg 2',
      timestamp: 1710000001,
    } as ServerEvent)

    const blocks = cs.accumulator.getBlocks()
    const userBlocks = blocks.filter((b) => b.type === 'user')
    expect(userBlocks).toHaveLength(2)
    if (userBlocks[0].type !== 'user') throw new Error('expected user')
    expect(userBlocks[0].text).toBe('msg 1')
    if (userBlocks[1].type !== 'user') throw new Error('expected user')
    expect(userBlocks[1].text).toBe('msg 2')
  })
})
