import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock useCliTerminal — avoids xterm.js DOM requirements in happy-dom
const mockUseCliTerminal = vi.fn(() => ({
  isConnected: false,
  error: null,
  sendKeys: vi.fn(),
}))

vi.mock('../useCliTerminal', () => ({
  useCliTerminal: (...args: unknown[]) => mockUseCliTerminal(...args),
}))

// Mock xterm CSS import
vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

const { CliTerminalPanel } = await import('../CliTerminalPanel')

describe('CliTerminalPanel', () => {
  it('renders with a tmuxSessionId param', () => {
    const props = {
      params: { tmuxSessionId: 'test-session-123' },
    } as Parameters<typeof CliTerminalPanel>[0]

    const { container } = render(<CliTerminalPanel {...props} />)

    // The panel should render a container div (the CliTerminal wrapper)
    expect(container.firstElementChild).not.toBeNull()
    // Should have the h-full class from the panel wrapper
    expect(container.innerHTML).toContain('h-full')
  })

  it('passes tmuxSessionId through to CliTerminal', () => {
    mockUseCliTerminal.mockClear()

    const props = {
      params: { tmuxSessionId: 'my-tmux-sess' },
    } as Parameters<typeof CliTerminalPanel>[0]

    render(<CliTerminalPanel {...props} />)

    // useCliTerminal should have been called with the tmuxSessionId
    expect(mockUseCliTerminal).toHaveBeenCalledWith(
      expect.objectContaining({ tmuxSessionId: 'my-tmux-sess' }),
    )
  })
})
