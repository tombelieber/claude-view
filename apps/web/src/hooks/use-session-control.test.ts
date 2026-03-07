import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlStatus } from './use-control-session'

// Store the mock implementation so we can change it per test
const mockSendMessage = vi.fn()
let mockStatus: ControlStatus = 'idle'
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
  mockStatus = 'idle'
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

  it('send() sets activeSessionId to trigger WS connection', async () => {
    const { useControlSession } = await import('./use-control-session')
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

    // useControlSession should have been called with the sessionId
    expect(useControlSession).toHaveBeenCalledWith('session-123')
  })

  it('phase derives from controlSession.status — waiting_input maps to ready', async () => {
    mockStatus = 'waiting_input'
    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    // waiting_input maps to 'ready' phase
    expect(result.current.phase).toBe('ready')
    expect(result.current.inputBarState).toBe('active')
  })

  it('connectionHealth returns lost for fatal/failed status', async () => {
    mockStatus = 'fatal'
    mockError = 'Session not found'
    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    expect(result.current.connectionHealth).toBe('lost')
    expect(result.current.phase).toBe('error')
  })

  it('multiple rapid sends do not create multiple connections', async () => {
    const { useControlSession } = await import('./use-control-session')
    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    act(() => {
      result.current.send('first')
      result.current.send('second')
    })

    // Both messages created
    expect(result.current.messages).toHaveLength(2)
    expect(result.current.messages[0].content).toBe('first')
    expect(result.current.messages[1].content).toBe('second')

    // useControlSession called with same sessionId (not called multiple times with different values)
    const calls = (useControlSession as ReturnType<typeof vi.fn>).mock.calls
    const lastCall = calls[calls.length - 1]
    expect(lastCall[0]).toBe('session-123')
  })

  it('drains pending message when WS reaches waiting_input', async () => {
    const { useSessionControl } = await import('./use-session-control')
    const { result, rerender } = renderHook(() => useSessionControl('session-123'))

    // Send a message (triggers setActiveSessionId)
    act(() => {
      result.current.send('hello')
    })

    // Simulate WS connecting and transitioning to waiting_input
    mockStatus = 'waiting_input'

    // Force re-render so the mock returns the new status and effects fire
    await act(async () => {
      rerender()
    })

    // The drain effect sees ready + waiting_input → calls sendMessage
    expect(result.current.phase).toBe('ready')
    expect(mockSendMessage).toHaveBeenCalledWith('hello')
  })
})
