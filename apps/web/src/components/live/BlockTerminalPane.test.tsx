import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../../hooks/use-block-socket', () => ({
  useBlockSocket: vi.fn(() => ({ blocks: [], bufferDone: false, connectionState: 'connecting' })),
}))

vi.mock('../../store/monitor-store', () => ({
  useMonitorStore: vi.fn((sel: any) => sel({ displayMode: 'chat', setDisplayMode: vi.fn() })),
}))

// Mock ConversationThread to avoid rendering the full tree
vi.mock('../conversation/ConversationThread', () => ({
  ConversationThread: vi.fn(() => <div data-testid="conversation-thread" />),
}))

import { useBlockSocket } from '../../hooks/use-block-socket'
import { BlockTerminalPane } from './BlockTerminalPane'

describe('BlockTerminalPane', () => {
  it('renders empty state when no blocks', () => {
    render(<BlockTerminalPane sessionId="s-1" isVisible />)
    expect(screen.getByText('No messages yet')).toBeInTheDocument()
  })

  it('renders ConversationThread when blocks exist', () => {
    vi.mocked(useBlockSocket).mockReturnValue({
      blocks: [{ type: 'user', id: 'u-1', text: 'hi', timestamp: 1000 } as any],
      bufferDone: true,
      connectionState: 'connected',
    })
    render(<BlockTerminalPane sessionId="s-1" isVisible />)
    expect(screen.queryByText('No messages yet')).not.toBeInTheDocument()
    expect(screen.getByTestId('conversation-thread')).toBeInTheDocument()
  })

  it('passes agentId to useBlockSocket for sub-agent view', () => {
    render(<BlockTerminalPane sessionId="s-1" isVisible agentId="agent-1" />)
    expect(useBlockSocket).toHaveBeenCalledWith(expect.objectContaining({ agentId: 'agent-1' }))
  })
})
