import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../../hooks/use-terminal-socket', () => ({
  useTerminalSocket: vi.fn(() => ({
    connectionState: 'connected' as const,
    sendMessage: vi.fn(),
    reconnect: vi.fn(),
  })),
}))

vi.mock('../../store/monitor-store', () => ({
  useMonitorStore: vi.fn((sel: any) => sel({ displayMode: 'chat', setDisplayMode: vi.fn() })),
}))

vi.mock('../conversation/ConversationThread', () => ({
  ConversationThread: vi.fn(() => <div data-testid="conversation-thread" />),
}))

import { BlockTerminalPane } from './BlockTerminalPane'

describe('BlockTerminalPane integration', () => {
  it('wires sessionId through useBlockSocket to useTerminalSocket', async () => {
    const { useTerminalSocket } = await import('../../hooks/use-terminal-socket')
    render(<BlockTerminalPane sessionId="test-session" isVisible />)
    expect(vi.mocked(useTerminalSocket)).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'test-session',
        mode: 'block',
        enabled: true,
      }),
    )
  })

  it('reads displayMode from monitor-store for registry selection', async () => {
    const { useMonitorStore } = await import('../../store/monitor-store')
    render(<BlockTerminalPane sessionId="test-session" isVisible />)
    expect(vi.mocked(useMonitorStore)).toHaveBeenCalled()
  })
})
