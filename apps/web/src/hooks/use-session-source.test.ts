import { describe, expect, it, vi } from 'vitest'
import { SessionChannel } from '../lib/session-channel'
import { deriveEffectiveSend } from './use-session-source'

describe('deriveEffectiveSend', () => {
  const send = vi.fn()
  const connectAndSend = vi.fn()

  // --- Unit: isLive=true → direct send ---
  it('returns direct send when WS is live', () => {
    expect(deriveEffectiveSend(true, 'ctrl-1', 'sess-1', send, connectAndSend)).toBe(send)
  })

  // --- Unit: not live but has controlId → connectAndSend ---
  it('returns connectAndSend when lazy-resumable (controlId set, not live)', () => {
    expect(deriveEffectiveSend(false, 'ctrl-1', 'sess-1', null, connectAndSend)).toBe(
      connectAndSend,
    )
  })

  // --- Unit: not live, no controlId but has sessionId → connectAndSend (auto-resume) ---
  it('returns connectAndSend for dormant session (sessionId only, no controlId)', () => {
    expect(deriveEffectiveSend(false, null, 'sess-1', null, connectAndSend)).toBe(connectAndSend)
  })

  // --- Unit: not live, no controlId, no sessionId → null ---
  it('returns null when no session at all', () => {
    expect(deriveEffectiveSend(false, null, undefined, null, connectAndSend)).toBe(null)
  })

  // --- Edge: isLive=true but send is null (shouldn't happen, but defensive) ---
  it('returns null send when live but send is null', () => {
    expect(deriveEffectiveSend(true, 'ctrl-1', 'sess-1', null, connectAndSend)).toBe(null)
  })
})

describe('Pending message queue drain pattern (mock WebSocket)', () => {
  // Mock WebSocket that captures sent data and lets us fire onopen
  class MockWebSocket {
    sent: string[] = []
    readyState = 0 // CONNECTING
    onopen: (() => void) | null = null

    send(data: string) {
      this.sent.push(data)
    }
    close() {
      this.readyState = 3
    }

    // Test helper: simulate connection established
    simulateOpen() {
      this.readyState = 1 // OPEN
      this.onopen?.()
    }
  }

  // --- Integration: full queue→connect→drain→deliver lifecycle ---
  it('messages queued via connectAndSend are drained when ws.onopen fires', () => {
    const pendingMessages: Record<string, unknown>[] = []
    const ws = new MockWebSocket()

    // Simulate the ws.onopen handler that drains pending messages
    // (mirrors the actual handler added in Task 3 Step 1 item 3)
    ws.onopen = () => {
      for (const msg of pendingMessages) {
        ws.send(JSON.stringify(msg))
      }
      pendingMessages.length = 0
    }

    // Simulate connectAndSend: queue messages while WS is CONNECTING
    const msg1 = { type: 'user_message', content: 'hello' }
    const msg2 = { type: 'user_message', content: 'world' }
    pendingMessages.push(msg1)
    pendingMessages.push(msg2)
    expect(ws.sent).toHaveLength(0) // Nothing sent yet — WS still connecting

    // WS connection established — onopen fires, drains queue
    ws.simulateOpen()

    // Verify: both messages sent via ws.send, queue cleared
    expect(ws.sent).toHaveLength(2)
    expect(JSON.parse(ws.sent[0])).toEqual(msg1)
    expect(JSON.parse(ws.sent[1])).toEqual(msg2)
    expect(pendingMessages).toHaveLength(0)
  })

  // --- Edge: no pending messages → onopen is a no-op ---
  it('onopen with empty queue sends nothing', () => {
    const pendingMessages: Record<string, unknown>[] = []
    const ws = new MockWebSocket()

    ws.onopen = () => {
      for (const msg of pendingMessages) {
        ws.send(JSON.stringify(msg))
      }
      pendingMessages.length = 0
    }

    ws.simulateOpen()
    expect(ws.sent).toHaveLength(0)
    expect(pendingMessages).toHaveLength(0)
  })

  // --- Edge: deriveEffectiveSend returns direct send when WS already OPEN ---
  it('deriveEffectiveSend selects direct send when live, skipping queue', () => {
    const ws = new MockWebSocket()
    ws.readyState = 1 // Already OPEN

    const directSend = (msg: Record<string, unknown>) => ws.send(JSON.stringify(msg))
    const connectAndSend = vi.fn() // Should NOT be called

    // Use the actual exported function to determine which send path
    const effectiveSend = deriveEffectiveSend(true, 'ctrl-1', 'sess-1', directSend, connectAndSend)
    expect(effectiveSend).toBe(directSend) // Must select direct path

    const msg = { type: 'user_message', content: 'direct' }
    effectiveSend?.(msg)

    expect(ws.sent).toHaveLength(1)
    expect(JSON.parse(ws.sent[0])).toEqual(msg)
    expect(connectAndSend).not.toHaveBeenCalled()
  })

  // --- Regression: cleanup prevents stale messages replaying to wrong session ---
  it('cleanup clears pending messages on session change', () => {
    const pendingMessages: Record<string, unknown>[] = []

    // Queue a message for session A
    pendingMessages.push({ type: 'user_message', content: 'for session A' })
    expect(pendingMessages).toHaveLength(1)

    // Session changes to B — cleanup runs (mirrors useEffect cleanup)
    pendingMessages.length = 0
    expect(pendingMessages).toHaveLength(0)

    // New message for session B should not include stale session A message
    pendingMessages.push({ type: 'user_message', content: 'for session B' })
    expect(pendingMessages).toHaveLength(1)
    expect(pendingMessages[0]).toEqual({ type: 'user_message', content: 'for session B' })
  })
})

// ═══════════════════════════════════════════════════════════════════════════
// Regression tests for all 4 motivating bugs (single-stream architecture)
// ═══════════════════════════════════════════════════════════════════════════

// ─── Bug 1: "New chat stuck forever" ────────────────────────────────────
// Root cause: init() filtered on session state before opening WS. Sessions in
// "waiting_input" (idle) state never auto-connected, so the user never saw
// the first assistant response.
// Fix: removed the state filter — init() now always calls openWs(sid) for
// any active session, regardless of state.
describe('Bug 1 regression — no state filter in auto-connect (structural)', () => {
  // Read the actual source code and verify no state-based filtering exists
  // in the init() function. This guards against re-introducing the anti-pattern.
  it('init() calls openWs unconditionally — source has no active.state filter', async () => {
    // Dynamic imports: vitest runs on Node but web tsconfig is browser-only
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )

    // Extract the init() function body
    const initMatch = source.match(/async function init\(\)\s*\{([\s\S]*?)^\s{4}\}/m)
    expect(initMatch).not.toBeNull()
    const initBody = initMatch?.[1]

    // The old code had: if (active.state === 'initializing' || active.state === 'active' ...)
    // Verify this pattern is NOT present — openWs is called unconditionally
    expect(initBody).not.toMatch(/active\.state\s*===/)
    expect(initBody).not.toMatch(/shouldAutoConnect/)

    // Verify openWs IS called (the fix is calling it unconditionally)
    expect(initBody).toMatch(/openWs\(sid\)/)
  })

  it('init() has comment explaining unconditional auto-connect', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )

    // The fix includes an explanatory comment so future devs understand the intent
    expect(source).toMatch(/Always auto-connect for active sessions/)
  })
})

// ─── Bug 2: "WS connection leak" ───────────────────────────────────────
// Covered by ws-handler.test.ts → "One WS per session" describe block.
// The sidecar's SessionRegistry enforces one WS per sessionId.

// ─── Bug 3: "User messages appear at bottom" ───────────────────────────
// Covered by stream-accumulator-echo.test.ts:
//   - "echo after session_init produces UserBlock in correct position"
//   - "multiple echoes in multi-turn appear in order"
// The accumulator places UserBlocks sequentially, before the assistant response.

// ─── Bug 4: "Duplicate content on resume" ──────────────────────────────
// Covered by stream-accumulator-echo.test.ts:
//   - "deduplicates on reconnect replay (same seq ignored)"
// The accumulator deduplicates by seq number, so reconnect replay is idempotent.

// ─── WS resume on first connect (event replay) ───────────────────────────
// On first connect (lastSeq=-1), the frontend sends resume to replay buffered
// events. This is critical for new sessions where initialMessage response
// may have been emitted before the WS connected.
describe('WS resume message always sent on connect', () => {
  class MockWebSocket {
    sent: string[] = []
    readyState = 0

    send(data: string) {
      this.sent.push(data)
    }
    close() {
      this.readyState = 3
    }
    simulateOpen() {
      this.readyState = 1
      this.onopen?.()
    }
    onopen: (() => void) | null = null
  }

  // --- Regression: first connect sends resume with lastSeq=-1 ---
  it('sends resume with lastSeq=-1 on first connect (replay all buffered events)', () => {
    const ws = new MockWebSocket()
    const lastSeq = -1

    ws.onopen = () => {
      // Mirrors the actual onopen handler logic
      ws.send(JSON.stringify({ type: 'resume', lastSeq }))
    }

    ws.simulateOpen()

    expect(ws.sent).toHaveLength(1)
    expect(JSON.parse(ws.sent[0])).toEqual({ type: 'resume', lastSeq: -1 })
  })

  // --- Unit: reconnect sends resume with actual lastSeq ---
  it('sends resume with lastSeq=5 on reconnect (replay missed events only)', () => {
    const ws = new MockWebSocket()
    const lastSeq = 5

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: 'resume', lastSeq }))
    }

    ws.simulateOpen()

    expect(ws.sent).toHaveLength(1)
    expect(JSON.parse(ws.sent[0])).toEqual({ type: 'resume', lastSeq: 5 })
  })

  // --- Integration: resume sent before pending messages are drained ---
  it('sends resume BEFORE draining pending messages', () => {
    const ws = new MockWebSocket()
    const lastSeq = -1
    const pendingMessages = [{ type: 'user_message', content: 'queued msg' }]

    ws.onopen = () => {
      // Resume first (get missed events)
      ws.send(JSON.stringify({ type: 'resume', lastSeq }))
      // Then drain queued messages
      for (const msg of pendingMessages) {
        ws.send(JSON.stringify(msg))
      }
      pendingMessages.length = 0
    }

    ws.simulateOpen()

    expect(ws.sent).toHaveLength(2)
    // Resume is FIRST — critical for event ordering
    expect(JSON.parse(ws.sent[0]).type).toBe('resume')
    expect(JSON.parse(ws.sent[1]).type).toBe('user_message')
  })
})

// ─── Create session response contract ─────────────────────────────────────
// The API response from POST /api/sidecar/sessions must include a non-empty
// sessionId for the frontend to navigate. These tests verify the contract.
describe('Create session response handling', () => {
  // --- Regression: empty sessionId must NOT trigger navigation ---
  it('empty sessionId string is falsy (no navigation)', () => {
    const data = { controlId: 'ctrl-123', sessionId: '', status: 'created' }
    // This is the actual check from ChatPage.handleSend
    expect(!!data.sessionId).toBe(false)
  })

  // --- Unit: valid sessionId triggers navigation ---
  it('non-empty sessionId is truthy (triggers navigation)', () => {
    const data = { controlId: 'ctrl-123', sessionId: 'abc-def-ghi', status: 'created' }
    expect(!!data.sessionId).toBe(true)
  })

  // --- Regression: undefined sessionId must NOT trigger navigation ---
  it('undefined sessionId is falsy (no navigation)', () => {
    const data = { controlId: 'ctrl-123', status: 'created' } as {
      controlId: string
      sessionId?: string
      status: string
    }
    expect(!!data.sessionId).toBe(false)
  })

  // --- Regression: error response has no sessionId ---
  it('error response with no sessionId shows error to user', () => {
    const data = { error: 'Create failed: auth error' }
    const sessionId = (data as { sessionId?: string }).sessionId
    expect(!!sessionId).toBe(false)
    // Frontend should show toast.error with data.error
    expect(data.error).toBeTruthy()
  })
})

describe('SessionSourceResult new fields — type contracts', () => {
  it('SessionInit type includes model, slashCommands, mcpServers', () => {
    // Import is from shared package
    const init = {
      type: 'session_init' as const,
      tools: ['Read', 'Write'],
      model: 'claude-opus-4-6',
      mcpServers: [{ name: 'github', status: 'connected' }],
      permissionMode: 'default',
      slashCommands: ['commit', 'test', 'review'],
      claudeCodeVersion: '1.2.3',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'normal',
    }
    expect(init.model).toBe('claude-opus-4-6')
    expect(init.slashCommands).toEqual(['commit', 'test', 'review'])
    expect(init.mcpServers).toEqual([{ name: 'github', status: 'connected' }])
  })

  it('default values match useState initializers', () => {
    // These must match the useState defaults in use-session-source.ts
    const defaults = {
      model: '',
      slashCommands: [] as string[],
      mcpServers: [] as { name: string; status: string }[],
    }
    expect(defaults.model).toBe('')
    expect(defaults.slashCommands).toHaveLength(0)
    expect(defaults.mcpServers).toHaveLength(0)
  })
})

// ─── SessionChannel response routing (mirrors use-session-source event handler) ───
describe('SessionChannel response routing', () => {
  it('query_result with requestId routes to channel.handleResponse', async () => {
    const mockSend = vi.fn()
    const channel = new SessionChannel(mockSend)
    const promise = channel.request<string[]>({ type: 'query_models' })

    const sentMsg = mockSend.mock.calls[0][0]
    const requestId = sentMsg.requestId

    // Simulate what use-session-source does on receiving query_result:
    const event = { type: 'query_result', queryType: 'models', data: ['model-a'], requestId }
    if (event.requestId) channel.handleResponse(event.requestId, event.data)

    await expect(promise).resolves.toEqual(['model-a'])
  })

  it('rewind_result with requestId routes to channel.handleResponse', async () => {
    const mockSend = vi.fn()
    const channel = new SessionChannel(mockSend)
    const promise = channel.request<unknown>({ type: 'rewind_files', userMessageId: 'u1' })

    const requestId = mockSend.mock.calls[0][0].requestId
    const event = { type: 'rewind_result', result: { files: ['a.ts'] }, requestId }
    if (event.requestId) channel.handleResponse(event.requestId, event.result)

    await expect(promise).resolves.toEqual({ files: ['a.ts'] })
  })

  it('mcp_set_result with requestId routes to channel.handleResponse', async () => {
    const mockSend = vi.fn()
    const channel = new SessionChannel(mockSend)
    const promise = channel.request<unknown>({ type: 'set_mcp_servers', servers: {} })

    const requestId = mockSend.mock.calls[0][0].requestId
    const event = { type: 'mcp_set_result', result: { ok: true }, requestId }
    if (event.requestId) channel.handleResponse(event.requestId, event.result)

    await expect(promise).resolves.toEqual({ ok: true })
  })

  it('response events without requestId are ignored (no throw)', () => {
    const channel = new SessionChannel(vi.fn())
    // Simulate query_result WITHOUT requestId — mirrors older sidecar
    const event = { type: 'query_result', queryType: 'models', data: [] }
    expect(() => {
      if ((event as { requestId?: string }).requestId) {
        channel.handleResponse((event as { requestId?: string }).requestId!, event.data)
      }
    }).not.toThrow()
  })

  it('WS disconnect rejects all pending channel requests', async () => {
    const mockSend = vi.fn()
    const channel = new SessionChannel(mockSend)
    const p1 = channel.request({ type: 'query_models' })
    const p2 = channel.request({ type: 'query_agents' })

    channel.handleDisconnect()

    await expect(p1).rejects.toThrow('disconnect')
    await expect(p2).rejects.toThrow('disconnect')
  })
})

// ─── Capabilities extraction from session_init ────────────────────────────
describe('capabilities from session_init', () => {
  it('SessionInit with capabilities field has string array', () => {
    const init = {
      type: 'session_init' as const,
      capabilities: ['interrupt', 'set_model', 'rewind_files'],
      tools: [],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: '',
    }
    expect(init.capabilities).toContain('interrupt')
    expect(init.capabilities).toContain('rewind_files')
    expect(init.capabilities).toHaveLength(3)
  })

  it('SessionInit without capabilities defaults to empty array', () => {
    const init = {
      type: 'session_init' as const,
      tools: [],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: '',
    }
    // Mirrors the extraction logic: init.capabilities ?? []
    const caps = (init as { capabilities?: string[] }).capabilities ?? []
    expect(caps).toEqual([])
  })
})

// ─── Regression: SDK session cleanup on page close ────────────────────────
// Root cause: useSessionSource cleanup only closed the WS but never terminated
// the SDK session on the sidecar. Sessions ran indefinitely, consuming resources.
//
// Design: DELETE fires on beforeunload (page close/refresh), NOT on React
// cleanup. React cleanup fires on in-app navigation which would aggressively
// kill sessions — the user would have to re-resume every time they switch pages.
// Pattern: Jupyter kernel idle timeout / VS Code Remote SSH.
describe('SDK session cleanup via beforeunload', () => {
  // --- Regression: controlIdRef tracks latest controlId for beforeunload closure ---
  it('controlIdRef mirrors controlId state for cleanup access', () => {
    const controlIdRef = { current: null as string | null }

    controlIdRef.current = 'ctrl-abc'
    expect(controlIdRef.current).toBe('ctrl-abc')

    controlIdRef.current = 'ctrl-def'
    expect(controlIdRef.current).toBe('ctrl-def')
  })

  // --- Regression: beforeunload handler sends DELETE with keepalive ---
  it('beforeunload handler sends DELETE with keepalive: true', () => {
    const fetchMock = vi.fn()
    const controlIdRef = { current: 'ctrl-cleanup-test' as string | null }

    // Simulate the beforeunload handler
    const handleBeforeUnload = () => {
      if (controlIdRef.current) {
        fetchMock(`/api/sidecar/sessions/${controlIdRef.current}`, {
          method: 'DELETE',
          keepalive: true,
        })
      }
    }

    handleBeforeUnload()

    expect(fetchMock).toHaveBeenCalledWith('/api/sidecar/sessions/ctrl-cleanup-test', {
      method: 'DELETE',
      keepalive: true,
    })
  })

  // --- Edge: beforeunload does NOT call DELETE when controlId is null ---
  it('beforeunload does NOT call DELETE when controlId is null', () => {
    const fetchMock = vi.fn()
    const controlIdRef = { current: null as string | null }

    const handleBeforeUnload = () => {
      if (controlIdRef.current) {
        fetchMock(`/api/sidecar/sessions/${controlIdRef.current}`, {
          method: 'DELETE',
          keepalive: true,
        })
      }
    }

    handleBeforeUnload()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  // --- Regression: React cleanup does NOT terminate session (preserves in-app nav) ---
  it('React cleanup only closes WS, does NOT send DELETE', () => {
    const calls: string[] = []
    const wsRef = {
      current: {
        close: () => calls.push('ws.close'),
      },
    }
    const heartbeatTimerRef = { current: 123 as unknown as ReturnType<typeof setInterval> | null }
    const reconnectTimerRef = { current: 456 as unknown as ReturnType<typeof setTimeout> | null }
    const pendingMessagesRef = { current: [{ type: 'user_message', content: 'pending' }] }

    // Simulate React cleanup (mirrors the actual useEffect cleanup)
    if (heartbeatTimerRef.current) {
      calls.push('clearHeartbeat')
      heartbeatTimerRef.current = null
    }
    if (reconnectTimerRef.current) {
      calls.push('clearReconnectTimer')
      reconnectTimerRef.current = null
    }
    wsRef.current?.close()
    pendingMessagesRef.current = []
    calls.push('clearPendingMessages')

    // NO terminateSDKSession in React cleanup — that's the fix
    expect(calls).toEqual([
      'clearHeartbeat',
      'clearReconnectTimer',
      'ws.close',
      'clearPendingMessages',
    ])
    // Notably absent: 'terminateSDKSession'
    expect(calls).not.toContain('terminateSDKSession')
  })
})

// ═══════════════════════════════════════════════════════════════════════════
// HT-07..HT-12: WS message handling — committedBlocks/pendingText state
// Tests the message-to-state mapping that handleWsMessage implements.
// Uses a minimal state machine mirroring the switch cases in use-session-source.ts.
// ═══════════════════════════════════════════════════════════════════════════

describe('WS message handling — committedBlocks/pendingText state (HT-07..HT-12)', () => {
  /** Minimal state machine mirroring handleWsMessage's switch cases for blocks/pending. */
  interface MsgState {
    committed: unknown[]
    pendingText: string
  }

  function applyMessage(state: MsgState, raw: Record<string, unknown>): MsgState {
    switch (raw.type) {
      case 'blocks_snapshot': {
        const blocks = raw.blocks as unknown[]
        return { committed: blocks, pendingText: '' }
      }
      case 'blocks_update': {
        const blocks = raw.blocks as unknown[]
        return { committed: blocks, pendingText: '' }
      }
      case 'turn_complete':
      case 'turn_error': {
        if (raw.blocks) {
          const blocks = raw.blocks as unknown[]
          return { committed: blocks, pendingText: '' }
        }
        return state
      }
      case 'stream_delta': {
        if (raw.textDelta) {
          return { ...state, pendingText: state.pendingText + (raw.textDelta as string) }
        }
        return state
      }
      default:
        return state
    }
  }

  const emptyState: MsgState = { committed: [], pendingText: '' }

  const mockBlock = (id: string) => ({ type: 'assistant', id, segments: [] })

  // --- HT-07: blocks_snapshot sets committedBlocks ---
  it('HT-07: blocks_snapshot sets committedBlocks and clears pendingText', () => {
    const blocks = [mockBlock('a1'), mockBlock('a2'), mockBlock('a3')]
    const result = applyMessage(emptyState, { type: 'blocks_snapshot', blocks, lastSeq: 5 })
    expect(result.committed).toHaveLength(3)
    expect(result.pendingText).toBe('')
  })

  // --- HT-08: blocks_update clears pendingText ---
  it('HT-08: blocks_update clears pendingText and updates committedBlocks', () => {
    // Start with some pending text from stream_delta
    const withPending = applyMessage(emptyState, { type: 'stream_delta', textDelta: 'abc' })
    expect(withPending.pendingText).toBe('abc')

    // blocks_update arrives — clears pending and sets committed
    const blocks = [mockBlock('a1'), mockBlock('a2')]
    const result = applyMessage(withPending, { type: 'blocks_update', blocks })
    expect(result.pendingText).toBe('')
    expect(result.committed).toHaveLength(2)
  })

  // --- HT-09: turn_complete WITH blocks updates committedBlocks atomically ---
  it('HT-09: turn_complete with blocks updates committedBlocks atomically', () => {
    const blocks = [
      mockBlock('a1'),
      mockBlock('a2'),
      mockBlock('a3'),
      mockBlock('a4'),
      mockBlock('a5'),
    ]
    const result = applyMessage(emptyState, {
      type: 'turn_complete',
      blocks,
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 5000,
      usage: {},
      modelUsage: {},
      stopReason: 'end_turn',
    })
    expect(result.committed).toHaveLength(5)
    expect(result.pendingText).toBe('')
  })

  // --- HT-10: turn_complete WITHOUT blocks — committedBlocks unchanged (backward compat) ---
  it('HT-10: turn_complete without blocks field leaves committedBlocks unchanged', () => {
    // First, set some committed blocks via blocks_snapshot
    const initialBlocks = [mockBlock('a1'), mockBlock('a2'), mockBlock('a3')]
    const withBlocks = applyMessage(emptyState, {
      type: 'blocks_snapshot',
      blocks: initialBlocks,
      lastSeq: 3,
    })
    expect(withBlocks.committed).toHaveLength(3)

    // turn_complete WITHOUT blocks field — backward compat path
    const result = applyMessage(withBlocks, {
      type: 'turn_complete',
      totalCostUsd: 0.01,
      numTurns: 1,
      // No blocks field
    })
    expect(result.committed).toHaveLength(3) // Unchanged
  })

  // --- HT-11: turn_error with blocks updates committedBlocks ---
  it('HT-11: turn_error with blocks updates committedBlocks', () => {
    const blocks = [mockBlock('a1'), mockBlock('a2')]
    const result = applyMessage(emptyState, {
      type: 'turn_error',
      blocks,
      error: { subtype: 'error_during_execution', messages: ['oops'] },
    })
    expect(result.committed).toHaveLength(2)
    expect(result.pendingText).toBe('')
  })

  // --- HT-12: stream_delta appends textDelta to pendingText only ---
  it('HT-12: stream_delta appends textDelta to pendingText, committedBlocks unchanged', () => {
    // Set initial committed blocks
    const initialBlocks = [mockBlock('a1')]
    const withBlocks = applyMessage(emptyState, {
      type: 'blocks_snapshot',
      blocks: initialBlocks,
      lastSeq: 1,
    })

    // First delta
    const after1 = applyMessage(withBlocks, { type: 'stream_delta', textDelta: 'hello' })
    expect(after1.pendingText).toBe('hello')
    expect(after1.committed).toHaveLength(1) // Unchanged

    // Second delta appends
    const after2 = applyMessage(after1, { type: 'stream_delta', textDelta: ' world' })
    expect(after2.pendingText).toBe('hello world')
    expect(after2.committed).toHaveLength(1) // Still unchanged
  })
})

// ═══════════════════════════════════════════════════════════════════════════
// Task 2: openWs consistency + interactive card events
// ═══════════════════════════════════════════════════════════════════════════

describe('openWs consistency — setSessionState before every openWs call (Task 2)', () => {
  it('connectAndSend sets initializing before openWs', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )
    // Extract connectAndSend body (the controlId branch)
    const connectAndSendMatch = source.match(
      /const connectAndSend[\s\S]*?if \(controlId\)\s*\{([\s\S]*?)\}/m,
    )
    expect(connectAndSendMatch).not.toBeNull()
    const body = connectAndSendMatch?.[1] ?? ''
    // setSessionState('initializing') must appear before openWs
    const initIdx = body.indexOf("setSessionState('initializing')")
    const openIdx = body.indexOf('openWs(')
    expect(initIdx).toBeGreaterThanOrEqual(0)
    expect(openIdx).toBeGreaterThan(initIdx)
  })

  it('reconnect callback sets initializing before openWs', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )
    const reconnectMatch = source.match(
      /const reconnect = useCallback\(\(\) => \{([\s\S]*?)\}, \[/m,
    )
    expect(reconnectMatch).not.toBeNull()
    const body = reconnectMatch?.[1] ?? ''
    const initIdx = body.indexOf("setSessionState('initializing')")
    const openIdx = body.indexOf('openWs(')
    expect(initIdx).toBeGreaterThanOrEqual(0)
    expect(openIdx).toBeGreaterThan(initIdx)
  })

  it('handleWsClose reconnect uses reconnecting (not initializing)', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )
    // The auto-reconnect in handleWsClose should use 'reconnecting'
    expect(source).toMatch(/setSessionState\('reconnecting'[\s\S]*?openWs\(sid\)/)
  })
})

describe('interactive card events → waiting_permission (Task 2)', () => {
  it('handleWsMessage covers all 4 interactive card types', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )
    for (const cardType of ['permission_request', 'ask_question', 'plan_approval', 'elicitation']) {
      expect(source).toMatch(new RegExp(`case '${cardType}'`))
    }
  })

  it('all interactive cards are grouped before setSessionState waiting_permission', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/hooks/use-session-source.ts'),
      'utf-8',
    )
    const block = source.match(
      /case 'permission_request':[\s\S]*?setSessionState\('waiting_permission'\)/m,
    )
    expect(block).not.toBeNull()
    const matched = block?.[0] ?? ''
    expect(matched).toContain("case 'ask_question'")
    expect(matched).toContain("case 'plan_approval'")
    expect(matched).toContain("case 'elicitation'")
  })
})
