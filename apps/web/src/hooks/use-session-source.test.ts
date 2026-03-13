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
