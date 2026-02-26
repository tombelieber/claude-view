import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SubAgentDrillDown } from './SubAgentDrillDown'
import { useSubAgentStream } from './use-subagent-stream'

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

describe('SubAgentDrillDown integration', () => {
  const defaultProps = {
    sessionId: 'abc123',
    agentId: 'a951849',
    agentType: 'Explore',
    description: 'Search codebase',
    onClose: vi.fn(),
  }

  it('shows connecting state before buffer loads', () => {
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'connecting',
      messages: [],
      bufferDone: false,
      reconnect: vi.fn(),
    })

    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('connecting')).toBeInTheDocument()
  })

  it('shows error state', () => {
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'error',
      messages: [],
      bufferDone: false,
      reconnect: vi.fn(),
    })

    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('error')).toBeInTheDocument()
  })

  it('shows disconnected state', () => {
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'disconnected',
      messages: [],
      bufferDone: false,
      reconnect: vi.fn(),
    })

    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('disconnected')).toBeInTheDocument()
  })

  it('passes messages to RichPane without crashing', () => {
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'connected',
      messages: [
        { type: 'user', content: 'Find the auth module' },
        { type: 'assistant', content: 'I found auth.ts in src/lib/' },
      ],
      bufferDone: true,
      reconnect: vi.fn(),
    })

    // RichPane uses react-virtuoso which requires real DOM measurements,
    // so we verify the component renders without crashing and the header
    // is present (messages are rendered by Virtuoso in a virtual list)
    render(<SubAgentDrillDown {...defaultProps} />)
    expect(screen.getByText('connected')).toBeInTheDocument()
    expect(screen.getByText('Explore')).toBeInTheDocument()
  })

  it('renders with empty messages list', () => {
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'connected',
      messages: [],
      bufferDone: true,
      reconnect: vi.fn(),
    })

    render(<SubAgentDrillDown {...defaultProps} />)
    // Should still render header without crashing
    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('Search codebase')).toBeInTheDocument()
  })
})
