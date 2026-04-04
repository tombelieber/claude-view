import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../../hooks/use-session-channel', () => ({
  useSessionChannel: vi.fn(() => ({
    blocks: [],
    bufferDone: false,
    rawLines: [],
    connectionState: 'connecting',
    error: null,
    sdkConnected: false,
  })),
}))

vi.mock('../../store/monitor-store', () => ({
  useMonitorStore: vi.fn((sel: any) => sel({ displayMode: 'chat', setDisplayMode: vi.fn() })),
}))

// Mock ConversationThread to avoid rendering the full tree
vi.mock('@claude-view/shared/components/conversation/ConversationThread', () => ({
  ConversationThread: vi.fn(() => <div data-testid="conversation-thread" />),
}))

import { useSessionChannel } from '../../hooks/use-session-channel'
import { BlockTerminalPane } from './BlockTerminalPane'

describe('BlockTerminalPane', () => {
  it('renders empty state when no blocks', () => {
    render(<BlockTerminalPane sessionId="s-1" isVisible />)
    expect(screen.getByText('No messages yet')).toBeInTheDocument()
  })

  it('renders ConversationThread when blocks exist', () => {
    vi.mocked(useSessionChannel).mockReturnValue({
      blocks: [{ type: 'user', id: 'u-1', text: 'hi', timestamp: 1000 } as any],
      bufferDone: true,
      rawLines: [],
      connectionState: 'connected',
      error: null,
      sdkConnected: false,
    })
    render(<BlockTerminalPane sessionId="s-1" isVisible />)
    expect(screen.queryByText('No messages yet')).not.toBeInTheDocument()
    expect(screen.getByTestId('conversation-thread')).toBeInTheDocument()
  })

  it('subscribes to block mode via useSessionChannel', () => {
    render(<BlockTerminalPane sessionId="s-1" isVisible />)
    expect(useSessionChannel).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 's-1',
        modes: ['block'],
        enabled: true,
      }),
    )
  })

  it('disables connection when not visible', () => {
    render(<BlockTerminalPane sessionId="s-1" isVisible={false} />)
    expect(useSessionChannel).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: false,
      }),
    )
  })
})
