// sidecar/src/blocks-snapshot.integration.test.ts
// IT-01..IT-09: Integration tests for the full pipeline:
//   event → emitSequenced → accumulator → handleWebSocket → blocks_snapshot/blocks_update
//
// Uses REAL: SessionRegistry, StreamAccumulator, EventEmitter
// Mocks ONLY: WebSocket, ControlSession stubs (query, bridge, abort, permissions)

import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import type { ServerEvent } from './protocol.js'
import type { ControlSession } from './session-registry.js'
import { SessionRegistry } from './session-registry.js'
import { StreamAccumulator } from './stream-accumulator.js'
import { handleWebSocket } from './ws-handler.js'

// ── Helpers ──────────────────────────────────────────────────────────────

function createMockWs() {
  const listeners: Record<string, (...args: unknown[]) => void> = {}
  return {
    send: vi.fn(),
    close: vi.fn(),
    readyState: 1,
    OPEN: 1,
    on: vi.fn((event: string, cb: (...args: unknown[]) => void) => {
      listeners[event] = cb
    }),
    _listeners: listeners,
  }
}

function getAllSentMessages(ws: ReturnType<typeof createMockWs>): Record<string, unknown>[] {
  return ws.send.mock.calls.map((c: unknown[]) => JSON.parse(c[0] as string))
}

/** Builds a real ControlSession with real accumulator, emitter.
 *  Only query/bridge/abort/permissions are minimal stubs. */
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

/** Standard session_init event payload. */
function sessionInitEvent(): ServerEvent {
  return {
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
}

function assistantTextEvent(text: string, messageId = 'msg-1'): ServerEvent {
  return {
    type: 'assistant_text',
    text,
    messageId,
    parentToolUseId: null,
  }
}

function turnCompleteEvent(): ServerEvent {
  return {
    type: 'turn_complete',
    totalCostUsd: 0.01,
    numTurns: 1,
    durationMs: 500,
    durationApiMs: 400,
    usage: {},
    modelUsage: {},
    permissionDenials: [],
    result: 'done',
    stopReason: null,
  }
}

// ── IT-01: Full pipeline — event → accumulator → blocks_snapshot on WS connect ─

describe('IT-01: Full pipeline — blocks_snapshot on WS connect', () => {
  it('accumulates events via emitSequenced and delivers blocks_snapshot on WS connect', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-1')
    registry.register(cs)

    // Push events through the real pipeline
    registry.emitSequenced(cs, sessionInitEvent())
    registry.emitSequenced(cs, assistantTextEvent('Hello world'))

    // Now connect a WS
    const ws = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws as any, 'ctrl-test', registry)

    const messages = getAllSentMessages(ws)
    const snapshot = messages.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot).toBeDefined()

    const blocks = snapshot?.blocks as { type: string }[]
    // Should have: system(session_init) + assistant(streaming)
    const systemBlocks = blocks.filter((b) => b.type === 'system')
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')
    expect(systemBlocks.length).toBeGreaterThanOrEqual(1)
    expect(assistantBlocks).toHaveLength(1)

    // Verify assistant block has the text
    const assistant = assistantBlocks[0] as { segments: { kind: string; text?: string }[] }
    const textSeg = assistant.segments.find((s) => s.kind === 'text')
    expect(textSeg).toBeDefined()
    expect(textSeg?.text).toBe('Hello world')
  })
})

// ── IT-02: Content event → blocks_update immediately sent ─────────────────

describe('IT-02: Pipeline — content event triggers blocks_update on connected WS', () => {
  it('sends both the raw event and blocks_update when assistant_text fires', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-2')
    registry.register(cs)

    // Initialize accumulator so it accepts events
    registry.emitSequenced(cs, sessionInitEvent())

    // Connect WS first (registers emitter listener)
    const ws = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws as any, 'ctrl-test', registry)

    // Clear sends from connect (heartbeat_config, session_status, blocks_snapshot)
    ws.send.mockClear()

    // Now emit an assistant_text — emitter listener in ws-handler should relay it
    registry.emitSequenced(cs, assistantTextEvent('streaming chunk'))

    const messages = getAllSentMessages(ws)
    // Expect: 1) the raw assistant_text event, 2) blocks_update
    expect(messages.length).toBeGreaterThanOrEqual(2)
    expect(messages[0].type).toBe('assistant_text')
    const blocksUpdate = messages.find((m) => m.type === 'blocks_update')
    expect(blocksUpdate).toBeDefined()
    expect((blocksUpdate?.blocks as unknown[]).length).toBeGreaterThan(0)
  })
})

// ── IT-03: turn_complete → blocks embedded in single WS message ───────────

describe('IT-03: turn_complete has embedded blocks, no separate blocks_update', () => {
  it('sends turn_complete with blocks field and no extra blocks_update', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-3')
    registry.register(cs)

    registry.emitSequenced(cs, sessionInitEvent())
    registry.emitSequenced(cs, assistantTextEvent('Some response'))

    // Connect WS
    const ws = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws as any, 'ctrl-test', registry)
    ws.send.mockClear()

    // Emit turn_complete — blocks should be embedded, NOT sent separately
    registry.emitSequenced(cs, turnCompleteEvent())

    const messages = getAllSentMessages(ws)
    const turnComplete = messages.find((m) => m.type === 'turn_complete')
    expect(turnComplete).toBeDefined()
    expect(turnComplete?.blocks).toBeDefined()
    expect((turnComplete?.blocks as unknown[]).length).toBeGreaterThan(0)

    // No separate blocks_update
    const blocksUpdate = messages.find((m) => m.type === 'blocks_update')
    expect(blocksUpdate).toBeUndefined()
  })
})

// ── IT-04: WS reconnect → snapshot replays current accumulator state ──────

describe('IT-04: WS reconnect replays full accumulator state', () => {
  it('delivers all accumulated blocks on reconnect', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-4')
    registry.register(cs)

    // Accumulate 3 turns
    registry.emitSequenced(cs, sessionInitEvent())
    for (let turn = 1; turn <= 3; turn++) {
      registry.emitSequenced(cs, assistantTextEvent(`Turn ${turn}`, `msg-${turn}`))
      registry.emitSequenced(cs, turnCompleteEvent())
    }

    // First WS connect
    const ws1 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws1 as any, 'ctrl-test', registry)

    // Disconnect (trigger close listener)
    const closeCb = ws1._listeners.close
    expect(closeCb).toBeDefined()
    closeCb()

    // Reconnect with new WS
    const ws2 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws2 as any, 'ctrl-test', registry)

    const messages = getAllSentMessages(ws2)
    const snapshot = messages.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot).toBeDefined()

    const blocks = snapshot?.blocks as { type: string }[]
    // Should have: system blocks + 3 assistant blocks (finalized) + 3 turn_boundary blocks
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')
    const boundaryBlocks = blocks.filter((b) => b.type === 'turn_boundary')
    expect(assistantBlocks).toHaveLength(3)
    expect(boundaryBlocks).toHaveLength(3)
  })
})

// ── IT-05: Multi-client — both WS connections coexist ──────────────────────

describe('IT-05: Multi-client — both WS connections coexist in wsClients Set', () => {
  it('both WS clients get blocks_snapshot and coexist', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-5')
    registry.register(cs)

    registry.emitSequenced(cs, sessionInitEvent())
    registry.emitSequenced(cs, assistantTextEvent('data'))

    // First WS
    const ws1 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws1 as any, 'ctrl-test', registry)

    // Second WS — both should coexist
    const ws2 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws2 as any, 'ctrl-test', registry)

    // First WS was NOT closed (multi-client)
    expect(ws1.close).not.toHaveBeenCalled()
    expect(cs.wsClients.size).toBe(2)

    // Both got blocks_snapshot
    const messages1 = getAllSentMessages(ws1)
    const snapshot1 = messages1.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot1).toBeDefined()

    const messages2 = getAllSentMessages(ws2)
    const snapshot2 = messages2.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot2).toBeDefined()
    expect((snapshot2?.blocks as unknown[]).length).toBeGreaterThan(0)
  })
})

// ── IT-06: Accumulator captures every emitted event (no drift) ────────────

describe('IT-06: Accumulator captures every emitted event', () => {
  it('accumulator reflects all 5 pushed events', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-6')
    registry.register(cs)

    // 5 events: session_init + 3 assistant_text + tool_use_start
    registry.emitSequenced(cs, sessionInitEvent())
    registry.emitSequenced(cs, assistantTextEvent('chunk1', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('chunk2', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('chunk3', 'msg-1'))
    registry.emitSequenced(cs, {
      type: 'tool_use_start',
      toolName: 'Read',
      toolInput: { file_path: '/tmp/x.ts' },
      toolUseId: 'tool-1',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    // Accumulator blocks reflect the state
    const blocks = cs.accumulator.getBlocks()
    const systemBlocks = blocks.filter((b) => b.type === 'system')
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')
    expect(systemBlocks.length).toBeGreaterThanOrEqual(1)
    expect(assistantBlocks).toHaveLength(1)

    // The assistant block has text + tool segments
    const assistant = assistantBlocks[0]
    if (assistant.type !== 'assistant') throw new Error('expected assistant')
    const textSeg = assistant.segments.find((s) => s.kind === 'text')
    const toolSeg = assistant.segments.find((s) => s.kind === 'tool')
    expect(textSeg).toBeDefined()
    expect(textSeg?.kind === 'text' && textSeg?.text).toBe('chunk1chunk2chunk3')
    expect(toolSeg).toBeDefined()
    if (toolSeg?.kind !== 'tool') throw new Error('expected tool')
    expect(toolSeg?.execution.toolName).toBe('Read')
  })
})

// ── IT-08: Reconnect after disconnect — new snapshot covers everything ──────

describe('IT-08: Reconnect after disconnect — new snapshot covers everything', () => {
  it('new WS connect gets full snapshot covering all events', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-8')
    registry.register(cs)

    // Accumulate 5 events
    registry.emitSequenced(cs, sessionInitEvent())
    registry.emitSequenced(cs, assistantTextEvent('a', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('b', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('c', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('d', 'msg-1'))

    // First WS connect
    const ws1 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws1 as any, 'ctrl-test', registry)

    // Disconnect
    const closeCb = ws1._listeners.close
    closeCb()

    // 2 more events while disconnected
    registry.emitSequenced(cs, assistantTextEvent('e', 'msg-1'))
    registry.emitSequenced(cs, assistantTextEvent('f', 'msg-1'))

    // New WS connects — gets fresh snapshot covering all events
    const ws2 = createMockWs()
    // biome-ignore lint/suspicious/noExplicitAny: test mock
    handleWebSocket(ws2 as any, 'ctrl-test', registry)

    const messages = getAllSentMessages(ws2)
    const snapshot = messages.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot).toBeDefined()

    // Blocks include all text
    const blocks = snapshot?.blocks as {
      type: string
      segments?: { kind: string; text?: string }[]
    }[]
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')
    expect(assistantBlocks).toHaveLength(1)
    const textSeg = assistantBlocks[0].segments?.find((s) => s.kind === 'text')
    expect(textSeg?.text).toBe('abcdef')
  })
})

// ── IT-09: user_message_echo accumulated via emitSequenced ────────────────

describe('IT-09: user_message_echo is accumulated via emitSequenced', () => {
  it('emitSequenced pushes user_message_echo into accumulator as UserBlock', () => {
    const registry = new SessionRegistry()
    const cs = buildRealSession('ctrl-test', 'sess-9')
    registry.register(cs)

    // user_message_echo bypasses the session_init gate
    const now = Date.now() / 1000
    registry.emitSequenced(cs, {
      type: 'user_message_echo',
      content: 'Hello',
      timestamp: now,
    })

    const blocks = cs.accumulator.getBlocks()
    const userBlocks = blocks.filter((b) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)

    const userBlock = userBlocks[0]
    if (userBlock.type !== 'user') throw new Error('expected user')
    expect(userBlock.text).toBe('Hello')
    expect(userBlock.timestamp).toBe(now)
  })
})
