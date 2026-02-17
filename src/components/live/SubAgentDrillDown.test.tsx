import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SubAgentDrillDown } from './SubAgentDrillDown'

// Mock the stream hook
vi.mock('./use-subagent-stream', () => ({
  useSubAgentStream: vi.fn(() => ({
    connectionState: 'connected',
    messages: [
      { type: 'user', content: 'Search for auth code' },
      { type: 'assistant', content: 'Found 3 files related to authentication.' },
    ],
    bufferDone: true,
    reconnect: vi.fn(),
  })),
}))

describe('SubAgentDrillDown', () => {
  const defaultProps = {
    sessionId: 'abc123',
    agentId: 'a951849',
    agentType: 'Explore',
    description: 'Search codebase for auth',
    onClose: vi.fn(),
  }

  it('renders agent type and description in header', () => {
    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('Search codebase for auth')).toBeInTheDocument()
  })

  it('renders the agent id', () => {
    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('id:a951849')).toBeInTheDocument()
  })

  it('renders connection status', () => {
    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('connected')).toBeInTheDocument()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    render(<SubAgentDrillDown {...defaultProps} onClose={onClose} />)
    const closeButton = screen.getByRole('button', { name: /close/i })
    await userEvent.click(closeButton)
    expect(onClose).toHaveBeenCalled()
  })

  it('toggles verbose/compact mode', async () => {
    render(<SubAgentDrillDown {...defaultProps} />)
    // Starts in compact mode
    const toggleButton = screen.getByText('compact')
    expect(toggleButton).toBeInTheDocument()

    // Click to switch to verbose
    await userEvent.click(toggleButton)
    expect(screen.getByText('verbose')).toBeInTheDocument()

    // Click again to switch back
    await userEvent.click(screen.getByText('verbose'))
    expect(screen.getByText('compact')).toBeInTheDocument()
  })
})
