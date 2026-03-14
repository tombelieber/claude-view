import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { McpPanel } from './McpPanel'

describe('McpPanel', () => {
  const defaultProps = {
    queryMcpStatus: vi.fn().mockResolvedValue([
      { name: 'github', status: 'connected' },
      { name: 'slack', status: 'disconnected' },
    ]),
    toggleMcp: vi.fn(),
    reconnectMcp: vi.fn(),
  }

  it('loads and displays server list on mount', async () => {
    render(<McpPanel {...defaultProps} />)
    await waitFor(() => {
      expect(screen.getByText('github')).toBeInTheDocument()
      expect(screen.getByText('slack')).toBeInTheDocument()
    })
  })

  it('shows toggle buttons for each server', async () => {
    render(<McpPanel {...defaultProps} />)
    await waitFor(() => screen.getByText('github'))
    // github is connected → shows "Disable"
    // slack is disconnected → shows "Enable"
    expect(screen.getByText('Disable')).toBeInTheDocument()
    expect(screen.getByText('Enable')).toBeInTheDocument()
  })

  it('calls toggleMcp when toggle button clicked', async () => {
    const props = { ...defaultProps, toggleMcp: vi.fn() }
    render(<McpPanel {...props} />)
    await waitFor(() => screen.getByText('github'))
    fireEvent.click(screen.getByText('Disable'))
    expect(props.toggleMcp).toHaveBeenCalledWith('github', false)
  })

  it('calls reconnectMcp when reconnect button clicked', async () => {
    const props = { ...defaultProps, reconnectMcp: vi.fn() }
    render(<McpPanel {...props} />)
    await waitFor(() => screen.getByText('slack'))
    // Find the reconnect button for slack (second reconnect button)
    const reconnectButtons = screen.getAllByText('Reconnect')
    fireEvent.click(reconnectButtons[1])
    expect(props.reconnectMcp).toHaveBeenCalledWith('slack')
  })

  it('shows error on query failure', async () => {
    const props = {
      ...defaultProps,
      queryMcpStatus: vi.fn().mockRejectedValue(new Error('network')),
    }
    render(<McpPanel {...props} />)
    await waitFor(() => {
      expect(screen.getByText('Failed to load MCP status')).toBeInTheDocument()
    })
  })
})
