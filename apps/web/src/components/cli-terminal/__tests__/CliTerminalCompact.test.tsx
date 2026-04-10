import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock useCliTerminal — avoids xterm.js DOM requirements in happy-dom
vi.mock('../useCliTerminal', () => ({
  useCliTerminal: vi.fn(() => ({
    status: { state: 'connecting' },
    sendKeys: vi.fn(),
    reconnect: vi.fn(),
    focus: vi.fn(),
  })),
}))

// Mock xterm CSS import
vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

const { CliTerminalCompact } = await import('../CliTerminalCompact')

describe('CliTerminalCompact', () => {
  it('renders the expand button when onExpand is provided', () => {
    const onExpand = vi.fn()
    render(<CliTerminalCompact tmuxSessionId="sess-1" onExpand={onExpand} />)

    const button = screen.getByRole('button', { name: 'Expand' })
    expect(button).toBeInTheDocument()
  })

  it('calls onExpand when the expand button is clicked', () => {
    const onExpand = vi.fn()
    render(<CliTerminalCompact tmuxSessionId="sess-1" onExpand={onExpand} />)

    const button = screen.getByRole('button', { name: 'Expand' })
    fireEvent.click(button)

    expect(onExpand).toHaveBeenCalledTimes(1)
  })

  it('hides the expand button when onExpand is not provided', () => {
    render(<CliTerminalCompact tmuxSessionId="sess-1" />)

    expect(screen.queryByRole('button', { name: 'Expand' })).not.toBeInTheDocument()
  })

  it('renders with h-48 height class for compact view', () => {
    const { container } = render(<CliTerminalCompact tmuxSessionId="sess-1" />)

    // The CliTerminal inside should have h-48 class
    expect(container.innerHTML).toContain('h-48')
  })
})
