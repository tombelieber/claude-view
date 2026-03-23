import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useSidecarConnection } from '../use-sidecar-connection'

// --- Mock WebSocket ---

class MockWebSocket {
  static instances: MockWebSocket[] = []
  url: string
  readyState = 0 // CONNECTING
  onopen: (() => void) | null = null
  onclose: ((event: { code: number; reason: string }) => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onerror: (() => void) | null = null
  send = vi.fn()
  close = vi.fn()

  constructor(url: string) {
    this.url = url
    MockWebSocket.instances.push(this)
  }

  // Test helpers
  simulateOpen() {
    this.readyState = 1 // OPEN
    this.onopen?.()
  }

  simulateMessage(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) })
  }

  simulateClose(code = 1000, reason = '') {
    this.readyState = 3 // CLOSED
    this.onclose?.({ code, reason })
  }
}

// Stub WebSocket globally
const OriginalWebSocket = globalThis.WebSocket

beforeEach(() => {
  MockWebSocket.instances = []
  globalThis.WebSocket = MockWebSocket as unknown as typeof WebSocket
  vi.useFakeTimers()
})

afterEach(() => {
  globalThis.WebSocket = OriginalWebSocket
  vi.useRealTimers()
  vi.restoreAllMocks()
})

describe('useSidecarConnection', () => {
  it('connects to /ws/chat/:sessionId on mount', () => {
    renderHook(() => useSidecarConnection('test-123'))
    expect(MockWebSocket.instances).toHaveLength(1)
    expect(MockWebSocket.instances[0].url).toContain('/ws/chat/test-123')
  })

  it('sets isLive=true when WS opens', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))
    expect(result.current.isLive).toBe(false)

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })
    expect(result.current.isLive).toBe(true)
  })

  it('sets isLive=false when WS closes', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })
    expect(result.current.isLive).toBe(true)

    act(() => {
      MockWebSocket.instances[0].simulateClose()
    })
    expect(result.current.isLive).toBe(false)
  })

  it('updates committedBlocks on blocks_snapshot message', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })

    const mockBlocks = [{ type: 'user', id: 'u1', text: 'hello', timestamp: 1700000000 }]

    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'blocks_snapshot',
        blocks: mockBlocks,
      })
    })

    expect(result.current.committedBlocks).toEqual(mockBlocks)
  })

  it('updates committedBlocks on blocks_update message', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })

    const mockBlocks = [
      { type: 'user', id: 'u1', text: 'hello', timestamp: 1700000000 },
      { type: 'assistant', id: 'a1', segments: [], streaming: false },
    ]

    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'blocks_update',
        blocks: mockBlocks,
      })
    })

    expect(result.current.committedBlocks).toEqual(mockBlocks)
  })

  it('clears pendingText on blocks_update', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })

    // Simulate some pending text via stream_delta
    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'stream_delta',
        textDelta: 'hello world',
      })
    })
    expect(result.current.pendingText).toBe('hello world')

    // blocks_update should clear pendingText
    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'blocks_update',
        blocks: [],
      })
    })
    expect(result.current.pendingText).toBe('')
  })

  it('reconnects after WS close with backoff', () => {
    renderHook(() => useSidecarConnection('test-123'))
    expect(MockWebSocket.instances).toHaveLength(1)

    // Close with recoverable code (normal close triggers reconnect)
    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })
    act(() => {
      MockWebSocket.instances[0].simulateClose(1006, 'abnormal')
    })

    // Advance past first backoff (1s)
    act(() => {
      vi.advanceTimersByTime(1100)
    })

    // Should have opened a second connection
    expect(MockWebSocket.instances.length).toBeGreaterThanOrEqual(2)
  })

  it('receives fresh blocks_snapshot on reconnect — no duplicate blocks', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    const initialBlocks = [{ type: 'user', id: 'u1', text: 'hello', timestamp: 1700000000 }]

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })
    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'blocks_snapshot',
        blocks: initialBlocks,
      })
    })
    expect(result.current.committedBlocks).toHaveLength(1)

    // Disconnect and reconnect
    act(() => {
      MockWebSocket.instances[0].simulateClose(1006, 'abnormal')
    })
    act(() => {
      vi.advanceTimersByTime(1100)
    })

    const reconnectedWs = MockWebSocket.instances[MockWebSocket.instances.length - 1]

    const updatedBlocks = [
      { type: 'user', id: 'u1', text: 'hello', timestamp: 1700000000 },
      { type: 'assistant', id: 'a1', segments: [], streaming: false },
    ]

    act(() => {
      reconnectedWs.simulateOpen()
    })
    act(() => {
      reconnectedWs.simulateMessage({
        type: 'blocks_snapshot',
        blocks: updatedBlocks,
      })
    })

    // Should have exactly the new blocks, not old + new
    expect(result.current.committedBlocks).toEqual(updatedBlocks)
    expect(result.current.committedBlocks).toHaveLength(2)
  })

  it('status transitions: active → idle → error follow session_state events', () => {
    const { result } = renderHook(() => useSidecarConnection('test-123'))

    act(() => {
      MockWebSocket.instances[0].simulateOpen()
    })

    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'session_state',
        status: 'active',
      })
    })
    expect(result.current.status).toBe('active')

    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'session_state',
        status: 'idle',
      })
    })
    expect(result.current.status).toBe('idle')

    act(() => {
      MockWebSocket.instances[0].simulateMessage({
        type: 'session_state',
        status: 'error',
      })
    })
    expect(result.current.status).toBe('error')
  })

  it('does not connect when skip=true (watching mode)', () => {
    renderHook(() => useSidecarConnection('test-123', { skip: true }))
    expect(MockWebSocket.instances).toHaveLength(0)
  })
})
