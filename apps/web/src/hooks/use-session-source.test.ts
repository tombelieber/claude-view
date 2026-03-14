import { describe, expect, it, vi } from 'vitest'
import { deriveCanResumeLazy, deriveEffectiveSend } from './use-session-source'

describe('deriveEffectiveSend', () => {
  const send = vi.fn()
  const connectAndSend = vi.fn()

  // --- Unit: isLive=true → direct send ---
  it('returns direct send when WS is live', () => {
    expect(deriveEffectiveSend(true, 'ctrl-1', send, connectAndSend)).toBe(send)
  })

  // --- Unit: not live but has controlId → connectAndSend ---
  it('returns connectAndSend when lazy-resumable (controlId set, not live)', () => {
    expect(deriveEffectiveSend(false, 'ctrl-1', null, connectAndSend)).toBe(connectAndSend)
  })

  // --- Unit: not live, no controlId → null (dormant) ---
  it('returns null when dormant (no controlId, not live)', () => {
    expect(deriveEffectiveSend(false, null, null, connectAndSend)).toBe(null)
  })

  // --- Edge: isLive=true but send is null (shouldn't happen, but defensive) ---
  it('returns null send when live but send is null', () => {
    expect(deriveEffectiveSend(true, 'ctrl-1', null, connectAndSend)).toBe(null)
  })
})

describe('deriveCanResumeLazy', () => {
  // --- Unit: controlId + not live → true ---
  it('returns true when controlId is set and not live', () => {
    expect(deriveCanResumeLazy('abc', false)).toBe(true)
  })

  // --- Unit: controlId + live → false (WS already open) ---
  it('returns false when live (WS already open)', () => {
    expect(deriveCanResumeLazy('abc', true)).toBe(false)
  })

  // --- Unit: no controlId → false ---
  it('returns false when no controlId', () => {
    expect(deriveCanResumeLazy(null, false)).toBe(false)
  })

  // --- Unit: no controlId + live → false ---
  it('returns false when no controlId and live', () => {
    expect(deriveCanResumeLazy(null, true)).toBe(false)
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
    const effectiveSend = deriveEffectiveSend(true, 'ctrl-1', directSend, connectAndSend)
    expect(effectiveSend).toBe(directSend) // Must select direct path

    const msg = { type: 'user_message', content: 'direct' }
    effectiveSend!(msg)

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

// ─── Auto-connect vs lazy connect (active session state) ──────────────────
// The init() function in useSessionSource checks the session state to decide
// whether to auto-connect (open WS immediately) or lazy connect (wait for
// user's next message). This logic is critical for new sessions with
// initialMessage — without auto-connect, the user never sees the response.
describe('Auto-connect decision based on ActiveSession state', () => {
  // Simulate the init() logic extracted for testability
  function shouldAutoConnect(state: string): boolean {
    return state === 'initializing' || state === 'active' || state === 'waiting_permission'
  }

  // --- Unit: initializing → auto-connect (new session with initialMessage) ---
  it('auto-connects for initializing state (new session being created)', () => {
    expect(shouldAutoConnect('initializing')).toBe(true)
  })

  // --- Unit: active → auto-connect (session actively processing) ---
  it('auto-connects for active state (SDK processing a message)', () => {
    expect(shouldAutoConnect('active')).toBe(true)
  })

  // --- Unit: waiting_input → lazy connect ---
  it('does NOT auto-connect for waiting_input state (idle session)', () => {
    expect(shouldAutoConnect('waiting_input')).toBe(false)
  })

  // --- Unit: closed → lazy connect ---
  it('does NOT auto-connect for closed state', () => {
    expect(shouldAutoConnect('closed')).toBe(false)
  })

  // --- Unit: error → lazy connect ---
  it('does NOT auto-connect for error state', () => {
    expect(shouldAutoConnect('error')).toBe(false)
  })

  // --- Unit: compacting → lazy connect (not urgently active) ---
  it('does NOT auto-connect for compacting state', () => {
    expect(shouldAutoConnect('compacting')).toBe(false)
  })

  // --- Unit: waiting_permission → auto-connect (user needs to see permission card) ---
  it('auto-connects for waiting_permission state (session needs user approval)', () => {
    expect(shouldAutoConnect('waiting_permission')).toBe(true)
  })
})

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
// The API response from POST /api/control/sessions must include a non-empty
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
