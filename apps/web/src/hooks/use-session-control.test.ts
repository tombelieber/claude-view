import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlStatus } from './use-control-session'

// Store the mock implementation so we can change it per test
const mockSendMessage = vi.fn()
let mockStatus: ControlStatus = 'disconnected'
let mockStreamingContent = ''
let mockError: string | null = null

vi.mock('./use-control-session', () => ({
  useControlSession: vi.fn(() => ({
    status: mockStatus,
    messages: [],
    streamingContent: mockStreamingContent,
    streamingMessageId: '',
    contextUsage: 0,
    turnCount: 0,
    sessionCost: null,
    lastTurnCost: null,
    permissionRequest: null,
    askQuestion: null,
    planApproval: null,
    elicitation: null,
    error: mockError,
    sendMessage: mockSendMessage,
    sendRaw: vi.fn(),
    respondPermission: vi.fn(),
    answerQuestion: vi.fn(),
    approvePlan: vi.fn(),
    submitElicitation: vi.fn(),
  })),
}))

beforeEach(() => {
  vi.restoreAllMocks()
  mockStatus = 'disconnected'
  mockStreamingContent = ''
  mockError = null
  mockSendMessage.mockClear()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('useSessionControl', () => {
  it('starts in idle phase with dormant inputBarState', async () => {
    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))
    expect(result.current.phase).toBe('idle')
    expect(result.current.inputBarState).toBe('dormant')
    expect(result.current.messages).toEqual([])
    expect(result.current.connectionHealth).toBe('ok')
  })

  it('creates optimistic message and starts resume on send()', async () => {
    // Use a never-resolving promise to freeze the state at 'resuming'
    vi.spyOn(globalThis, 'fetch').mockReturnValue(new Promise(() => {}))

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    act(() => {
      result.current.send('hello')
    })

    // Message created optimistically
    expect(result.current.messages).toHaveLength(1)
    expect(result.current.messages[0].content).toBe('hello')
    expect(result.current.messages[0].status).toBe('optimistic')
    expect(result.current.messages[0].role).toBe('user')
    // Phase is resuming (fetch still pending)
    expect(result.current.phase).toBe('resuming')
    expect(result.current.inputBarState).toBe('resuming')
  })

  it('transitions to connecting on successful resume', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify({ controlId: 'ctrl-1', sessionId: 'session-123' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    await act(async () => {
      result.current.send('hello')
    })

    // After fetch resolves, phase should be 'connecting'
    expect(result.current.phase).toBe('connecting')
    expect(result.current.messages[0].status).toBe('optimistic')
  })

  it('marks message as failed when resume POST fails', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response('Internal Server Error', { status: 500 }),
    )

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    await act(async () => {
      result.current.send('hello')
    })

    expect(result.current.messages[0].status).toBe('failed')
    expect(result.current.phase).toBe('error')
    expect(result.current.error).toBe('Failed to resume session')
  })

  it('prevents double-resume on rapid sends', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockReturnValue(new Promise(() => {}))

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    act(() => {
      result.current.send('first')
      result.current.send('second')
    })

    // Only one fetch call — second send is queued
    expect(fetchSpy).toHaveBeenCalledTimes(1)
    expect(result.current.messages).toHaveLength(2)
    expect(result.current.messages[0].content).toBe('first')
    expect(result.current.messages[1].content).toBe('second')
  })

  it('handles already_active resume response correctly', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          controlId: 'ctrl-existing',
          sessionId: 'session-123',
          status: 'already_active',
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    )

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    await act(async () => {
      result.current.send('hello')
    })

    // Should still transition to connecting — already_active means a control session exists
    expect(result.current.phase).toBe('connecting')
    expect(result.current.messages[0].status).toBe('optimistic')
  })

  it('drains pending message when WS reaches waiting_input', async () => {
    // Start with a successful resume
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify({ controlId: 'ctrl-1', sessionId: 'session-123' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const { useSessionControl } = await import('./use-session-control')
    let sid = 'session-123'
    const { result, rerender } = renderHook(() => useSessionControl(sid))

    // Send a message (triggers resume)
    await act(async () => {
      result.current.send('hello')
    })

    expect(result.current.phase).toBe('connecting')

    // Simulate WS connecting and transitioning to waiting_input
    mockStatus = 'waiting_input'

    // Force re-render so the mock returns the new status and effects fire
    await act(async () => {
      sid = 'session-123' // same value, but rerender triggers hook re-execution
      rerender()
    })

    // The phase transition effect sees waiting_input → sets phase to 'ready'
    // The drain effect sees ready + waiting_input → calls sendMessage
    expect(result.current.phase).toBe('ready')
    expect(mockSendMessage).toHaveBeenCalledWith('hello')
  })
})
