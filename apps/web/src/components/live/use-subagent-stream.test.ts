import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useSubAgentStream } from './use-subagent-stream'

// Mock useTerminalSocket since it creates real WebSockets
vi.mock('../../hooks/use-terminal-socket', () => ({
  useTerminalSocket: vi.fn(() => ({
    connectionState: 'disconnected' as const,
    sendMessage: vi.fn(),
    reconnect: vi.fn(),
  })),
}))

import { useTerminalSocket } from '../../hooks/use-terminal-socket'
const mockUseTerminalSocket = vi.mocked(useTerminalSocket)

describe('useSubAgentStream', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('passes correct URL to useTerminalSocket', () => {
    const onMessage = vi.fn()
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage,
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'abc123/subagents/a951849',
        mode: 'rich',
        enabled: true,
      })
    )
  })

  it('disables when agentId is null', () => {
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: null,
        enabled: true,
        onMessage: vi.fn(),
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: false,
      })
    )
  })

  it('disables when enabled is false even with valid agentId', () => {
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: false,
        onMessage: vi.fn(),
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: false,
      })
    )
  })

  it('returns initial state correctly', () => {
    const { result } = renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage: vi.fn(),
      })
    )

    expect(result.current.connectionState).toBe('disconnected')
    expect(result.current.messages).toEqual([])
    expect(result.current.bufferDone).toBe(false)
    expect(typeof result.current.reconnect).toBe('function')
  })

  it('parses incoming messages into RichMessage array', () => {
    let capturedOnMessage: ((data: string) => void) | undefined
    mockUseTerminalSocket.mockImplementation((opts) => {
      capturedOnMessage = opts.onMessage
      return {
        connectionState: 'connected',
        sendMessage: vi.fn(),
        reconnect: vi.fn(),
      }
    })

    const onMessage = vi.fn()
    const { result } = renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage,
      })
    )

    // Simulate an assistant message arriving
    act(() => {
      capturedOnMessage?.(JSON.stringify({
        type: 'message',
        role: 'assistant',
        content: 'Hello from sub-agent',
      }))
    })

    expect(result.current.messages).toHaveLength(1)
    expect(result.current.messages[0].type).toBe('assistant')
    expect(result.current.messages[0].content).toBe('Hello from sub-agent')
    expect(onMessage).toHaveBeenCalledTimes(1)
  })

  it('sets bufferDone when buffer_end signal arrives', () => {
    let capturedOnMessage: ((data: string) => void) | undefined
    mockUseTerminalSocket.mockImplementation((opts) => {
      capturedOnMessage = opts.onMessage
      return {
        connectionState: 'connected',
        sendMessage: vi.fn(),
        reconnect: vi.fn(),
      }
    })

    const onMessage = vi.fn()
    const { result } = renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage,
      })
    )

    expect(result.current.bufferDone).toBe(false)

    act(() => {
      capturedOnMessage?.(JSON.stringify({ type: 'buffer_end' }))
    })

    expect(result.current.bufferDone).toBe(true)
    // buffer_end should still be forwarded to onMessage
    expect(onMessage).toHaveBeenCalledTimes(1)
  })

  it('skips unparseable messages but still forwards to onMessage', () => {
    let capturedOnMessage: ((data: string) => void) | undefined
    mockUseTerminalSocket.mockImplementation((opts) => {
      capturedOnMessage = opts.onMessage
      return {
        connectionState: 'connected',
        sendMessage: vi.fn(),
        reconnect: vi.fn(),
      }
    })

    const onMessage = vi.fn()
    const { result } = renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage,
      })
    )

    // Send something that parseRichMessage returns null for
    act(() => {
      capturedOnMessage?.('not valid json at all')
    })

    // parseRichMessage returns null for invalid JSON, so no messages accumulated
    expect(result.current.messages).toHaveLength(0)
    // But onMessage should still be called
    expect(onMessage).toHaveBeenCalledWith('not valid json at all')
  })

  it('passes scrollback of 100_000 to useTerminalSocket', () => {
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage: vi.fn(),
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        scrollback: 100_000,
      })
    )
  })
})
