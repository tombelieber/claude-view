import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock useCliTerminal — avoids xterm.js DOM requirements in happy-dom
const mockUseCliTerminal = vi.fn(() => ({
  isConnected: false,
  error: null,
  sendKeys: vi.fn(),
  reconnect: vi.fn(),
}))

vi.mock('../useCliTerminal', () => ({
  useCliTerminal: (...args: unknown[]) => mockUseCliTerminal(...args),
}))

// Mock xterm CSS import
vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

const { CliTerminalPanel } = await import('../CliTerminalPanel')

// Minimal mock of IDockviewPanelProps — only `params` is used by the component.
function makePanelProps(tmuxSessionId: string) {
  return { params: { tmuxSessionId } } as unknown as Parameters<typeof CliTerminalPanel>[0]
}

describe('CliTerminalPanel', () => {
  it('renders with a tmuxSessionId param', () => {
    const { container } = render(<CliTerminalPanel {...makePanelProps('test-session-123')} />)

    // The panel should render a container div (the CliTerminal wrapper)
    expect(container.firstElementChild).not.toBeNull()
    // Should have the h-full class from the panel wrapper
    expect(container.innerHTML).toContain('h-full')
  })

  it('passes tmuxSessionId through to CliTerminal', () => {
    mockUseCliTerminal.mockClear()

    render(<CliTerminalPanel {...makePanelProps('my-tmux-sess')} />)

    // useCliTerminal should have been called with the tmuxSessionId
    expect(mockUseCliTerminal).toHaveBeenCalledWith(
      expect.objectContaining({ tmuxSessionId: 'my-tmux-sess' }),
    )
  })
})
