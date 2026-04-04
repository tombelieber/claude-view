import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../../hooks/use-session-channel', () => ({
  useSessionChannel: vi.fn(() => ({
    blocks: [],
    bufferDone: false,
    rawLines: [],
    connectionState: 'connected' as const,
    error: null,
    sdkConnected: false,
  })),
}))

vi.mock('../../store/monitor-store', () => ({
  useMonitorStore: vi.fn((sel: any) => sel({ displayMode: 'chat', setDisplayMode: vi.fn() })),
}))

vi.mock('@claude-view/shared/components/conversation/ConversationThread', () => ({
  ConversationThread: vi.fn(() => <div data-testid="conversation-thread" />),
}))

import { useSessionChannel } from '../../hooks/use-session-channel'
import { useMonitorStore } from '../../store/monitor-store'
import { BlockTerminalPane } from './BlockTerminalPane'

describe('BlockTerminalPane integration', () => {
  it('wires sessionId through useSessionChannel in block mode', () => {
    render(<BlockTerminalPane sessionId="test-session" isVisible />)
    expect(vi.mocked(useSessionChannel)).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'test-session',
        modes: ['block'],
        enabled: true,
      }),
    )
  })

  it('reads displayMode from monitor-store for registry selection', () => {
    render(<BlockTerminalPane sessionId="test-session" isVisible />)
    expect(vi.mocked(useMonitorStore)).toHaveBeenCalled()
  })
})
