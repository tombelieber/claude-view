// sidecar/src/blocks-snapshot.test.ts
// UT-01..UT-10: Tests for StreamAccumulator copy + ws-handler blocks_snapshot/blocks_update behavior
import { describe, expect, it, vi } from 'vitest'
import { StreamAccumulator } from './stream-accumulator.js'
import { handleWebSocket } from './ws-handler.js'

// ── UT-01: StreamAccumulator sanity check ───────────────────────────────

describe('UT-01: StreamAccumulator basic accumulation', () => {
  it('produces correct blocks from assistant_text + tool_use_start events', () => {
    const acc = new StreamAccumulator()

    // Must send session_init first to unlock the gate
    acc.push({
      type: 'session_init',
      seq: 0,
      sessionId: 'sess-1',
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
    })

    acc.push({
      type: 'assistant_text',
      seq: 1,
      text: 'Hello ',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    acc.push({
      type: 'assistant_text',
      seq: 2,
      text: 'world',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    acc.push({
      type: 'tool_use_start',
      seq: 3,
      toolName: 'Read',
      toolInput: { file_path: '/tmp/a.ts' },
      toolUseId: 'tool-1',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const blocks = acc.getBlocks()
    // Expect: [system(session_init), assistant(streaming)]
    expect(blocks).toHaveLength(2)

    // First block is system (session_init)
    expect(blocks[0].type).toBe('system')

    // Second block is assistant with text segment + tool segment
    const assistant = blocks[1]
    expect(assistant.type).toBe('assistant')
    if (assistant.type !== 'assistant') throw new Error('expected assistant')
    expect(assistant.segments).toHaveLength(2)
    expect(assistant.segments[0]).toEqual({
      kind: 'text',
      text: 'Hello world',
      parentToolUseId: null,
    })
    expect(assistant.segments[1].kind).toBe('tool')
    if (assistant.segments[1].kind !== 'tool') throw new Error('expected tool')
    expect(assistant.segments[1].execution.toolName).toBe('Read')
    expect(assistant.segments[1].execution.status).toBe('running')
  })
})

// ── Helpers for UT-02..10 ───────────────────────────────────────────────

function createMockAccumulator() {
  return {
    getBlocks: vi.fn().mockReturnValue([]),
    push: vi.fn(),
    finalize: vi.fn(),
    reset: vi.fn(),
  }
}

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

function createMockSession(overrides?: Record<string, unknown>) {
  return {
    wsClients: new Set(),
    lastSessionInit: null,
    state: 'active',
    emitter: { on: vi.fn(), removeListener: vi.fn() },
    permissions: {
      resolvePermission: vi.fn(),
      resolveQuestion: vi.fn(),
      resolvePlan: vi.fn(),
      resolveElicitation: vi.fn(),
      drainInteractive: vi.fn(),
    },
    query: {
      supportedModels: vi.fn().mockResolvedValue([]),
      supportedCommands: vi.fn().mockResolvedValue([]),
      supportedAgents: vi.fn().mockResolvedValue([]),
      mcpServerStatus: vi.fn().mockResolvedValue([]),
      accountInfo: vi.fn().mockResolvedValue({}),
      setMcpServers: vi.fn().mockResolvedValue({ ok: true }),
      rewindFiles: vi.fn().mockResolvedValue({ files: [] }),
    },
    accumulator: createMockAccumulator(),
    ...overrides,
  }
}

function createMockRegistry(session: ReturnType<typeof createMockSession>) {
  return {
    get: vi.fn().mockReturnValue(session),
    emitSequenced: vi.fn(),
  }
}

function getAllSentMessages(ws: ReturnType<typeof createMockWs>): Record<string, unknown>[] {
  return ws.send.mock.calls.map((c: unknown[]) => JSON.parse(c[0] as string))
}

// ── UT-02: First WS message on connect is blocks_snapshot ───────────────

describe('UT-02: blocks_snapshot sent on connect', () => {
  it('sends blocks_snapshot as a WS message on connect', () => {
    const session = createMockSession()
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const messages = getAllSentMessages(ws)
    const snapshot = messages.find((m) => m.type === 'blocks_snapshot')
    expect(snapshot).toBeDefined()
    expect(snapshot!.blocks).toEqual([])
  })
})

// ── UT-03: blocks_snapshot sent AFTER heartbeat_config ──────────────────

describe('UT-03: blocks_snapshot after heartbeat_config', () => {
  it('sends heartbeat_config before blocks_snapshot', () => {
    const session = createMockSession()
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const messages = getAllSentMessages(ws)
    const heartbeatIdx = messages.findIndex((m) => m.type === 'heartbeat_config')
    const snapshotIdx = messages.findIndex((m) => m.type === 'blocks_snapshot')

    expect(heartbeatIdx).toBeGreaterThanOrEqual(0)
    expect(snapshotIdx).toBeGreaterThanOrEqual(0)
    expect(snapshotIdx).toBeGreaterThan(heartbeatIdx)
  })
})

// ── UT-04: turn_complete relayed with blocks field ──────────────────────

describe('UT-04: turn_complete includes blocks', () => {
  it('relays turn_complete with blocks field attached', () => {
    const session = createMockSession()
    session.accumulator.getBlocks.mockReturnValue([{ type: 'assistant', id: 'a1' }])
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    // Get the emitter listener that ws-handler registered
    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    expect(emitterOnCall).toBeDefined()
    const onMessage = emitterOnCall![1] as (msg: unknown) => void

    // Fire a turn_complete event through the emitter
    onMessage({
      type: 'turn_complete',
      seq: 10,
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 500,
      durationApiMs: 400,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: 'done',
      stopReason: null,
    })

    const messages = getAllSentMessages(ws)
    const turnComplete = messages.find((m) => m.type === 'turn_complete')
    expect(turnComplete).toBeDefined()
    expect(turnComplete!.blocks).toEqual([{ type: 'assistant', id: 'a1' }])
  })
})

// ── UT-05: turn_error relayed with blocks field ─────────────────────────

describe('UT-05: turn_error includes blocks', () => {
  it('relays turn_error with blocks field attached', () => {
    const session = createMockSession()
    session.accumulator.getBlocks.mockReturnValue([{ type: 'notice', id: 'n1' }])
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void

    onMessage({
      type: 'turn_error',
      seq: 11,
      subtype: 'error_during_execution',
      errors: ['boom'],
      permissionDenials: [],
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 100,
      usage: {},
      modelUsage: {},
      stopReason: null,
    })

    const messages = getAllSentMessages(ws)
    const turnError = messages.find((m) => m.type === 'turn_error')
    expect(turnError).toBeDefined()
    expect(turnError!.blocks).toEqual([{ type: 'notice', id: 'n1' }])
  })
})

// ── UT-06: assistant_text fires blocks_update ───────────────────────────

describe('UT-06: assistant_text triggers blocks_update', () => {
  it('sends blocks_update after relaying assistant_text', () => {
    const session = createMockSession()
    session.accumulator.getBlocks.mockReturnValue([{ type: 'assistant', id: 'a1' }])
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void

    // Clear prior sends (heartbeat_config, blocks_snapshot, etc.)
    ws.send.mockClear()

    onMessage({
      type: 'assistant_text',
      seq: 5,
      text: 'Hello',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const messages = getAllSentMessages(ws)
    // Should have: 1) the event itself, 2) blocks_update
    expect(messages).toHaveLength(2)
    expect(messages[0].type).toBe('assistant_text')
    expect(messages[1].type).toBe('blocks_update')
    expect(messages[1].blocks).toEqual([{ type: 'assistant', id: 'a1' }])
  })
})

// ── UT-07: tool_use_start fires blocks_update ───────────────────────────

describe('UT-07: tool_use_start triggers blocks_update', () => {
  it('sends blocks_update after relaying tool_use_start', () => {
    const session = createMockSession()
    session.accumulator.getBlocks.mockReturnValue([])
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void
    ws.send.mockClear()

    onMessage({
      type: 'tool_use_start',
      seq: 6,
      toolName: 'Read',
      toolInput: {},
      toolUseId: 'tool-1',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const messages = getAllSentMessages(ws)
    expect(messages).toHaveLength(2)
    expect(messages[0].type).toBe('tool_use_start')
    expect(messages[1].type).toBe('blocks_update')
  })
})

// ── UT-08: assistant_thinking fires blocks_update ───────────────────────

describe('UT-08: assistant_thinking triggers blocks_update', () => {
  it('sends blocks_update after relaying assistant_thinking', () => {
    const session = createMockSession()
    session.accumulator.getBlocks.mockReturnValue([])
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void
    ws.send.mockClear()

    onMessage({
      type: 'assistant_thinking',
      seq: 7,
      thinking: 'hmm...',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const messages = getAllSentMessages(ws)
    expect(messages).toHaveLength(2)
    expect(messages[0].type).toBe('assistant_thinking')
    expect(messages[1].type).toBe('blocks_update')
  })
})

// ── UT-09: session_init does NOT fire blocks_update ─────────────────────

describe('UT-09: session_init does NOT trigger blocks_update', () => {
  it('relays session_init without blocks_update', () => {
    const session = createMockSession()
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void
    ws.send.mockClear()

    onMessage({
      type: 'session_init',
      seq: 1,
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
    })

    const messages = getAllSentMessages(ws)
    // Should have just the relayed event, no blocks_update
    expect(messages).toHaveLength(1)
    expect(messages[0].type).toBe('session_init')
    const blocksUpdate = messages.find((m) => m.type === 'blocks_update')
    expect(blocksUpdate).toBeUndefined()
  })
})

// ── UT-10: stream_delta does NOT fire blocks_update ─────────────────────

describe('UT-10: stream_delta does NOT trigger blocks_update', () => {
  it('relays stream_delta without blocks_update', () => {
    const session = createMockSession()
    const registry = createMockRegistry(session)
    const ws = createMockWs()

    // biome-ignore lint/suspicious/noExplicitAny: test mocks
    handleWebSocket(ws as any, 'ctrl-1', registry as any)

    const emitterOnCall = session.emitter.on.mock.calls.find((c: unknown[]) => c[0] === 'message')
    const onMessage = emitterOnCall![1] as (msg: unknown) => void
    ws.send.mockClear()

    onMessage({
      type: 'stream_delta',
      seq: 8,
      event: {},
      messageId: 'msg-1',
      deltaType: 'content_block_delta',
      textDelta: 'hi',
    })

    const messages = getAllSentMessages(ws)
    // Should have just the relayed event, no blocks_update
    expect(messages).toHaveLength(1)
    expect(messages[0].type).toBe('stream_delta')
    const blocksUpdate = messages.find((m) => m.type === 'blocks_update')
    expect(blocksUpdate).toBeUndefined()
  })
})
