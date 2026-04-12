import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock useCliTerminal to avoid xterm DOM requirements
vi.mock('../useCliTerminal', () => ({
  useCliTerminal: vi.fn(() => ({
    status: { state: 'connected' },
    sendKeys: vi.fn(),
    reconnect: vi.fn(),
    focus: vi.fn(),
  })),
}))

vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

const { CliTerminal } = await import('../CliTerminal')

describe('CliTerminal', () => {
  describe('default mode (not embedded)', () => {
    it('renders status bar', () => {
      const { container } = render(<CliTerminal tmuxSessionId="tmux-1" />)
      expect(screen.getByText('Connected')).toBeInTheDocument()
      // Status bar has the absolute positioning
      const statusBar = container.querySelector('.absolute.top-0')
      expect(statusBar).not.toBeNull()
    })

    it('renders terminal container with pt-5 padding', () => {
      const { container } = render(<CliTerminal tmuxSessionId="tmux-1" />)
      // The terminal container div (second child, has the ref) should have pt-5
      const termContainer = container.querySelector('.pt-5')
      expect(termContainer).not.toBeNull()
    })
  })

  describe('embedded mode', () => {
    it('does not render status bar when embedded', () => {
      const { container } = render(<CliTerminal tmuxSessionId="tmux-1" embedded />)
      // No status bar text
      expect(screen.queryByText('Connected')).not.toBeInTheDocument()
      // No absolute positioned overlay
      const statusBar = container.querySelector('.absolute.top-0')
      expect(statusBar).toBeNull()
    })

    it('renders terminal container without pt-5 padding when embedded', () => {
      const { container } = render(<CliTerminal tmuxSessionId="tmux-1" embedded />)
      const termContainer = container.querySelector('.pt-5')
      expect(termContainer).toBeNull()
    })

    it('still renders disconnect overlay when embedded and disconnected', async () => {
      const { useCliTerminal } = await import('../useCliTerminal')
      ;(useCliTerminal as ReturnType<typeof vi.fn>).mockReturnValue({
        status: { state: 'disconnected', reason: 'Connection lost' },
        sendKeys: vi.fn(),
        reconnect: vi.fn(),
        focus: vi.fn(),
      })

      render(<CliTerminal tmuxSessionId="tmux-1" embedded />)
      expect(screen.getByText('Connection lost')).toBeInTheDocument()
      expect(screen.getByText('Reconnect')).toBeInTheDocument()
    })

    it('renders terminal container as full height with no padding', () => {
      const { container } = render(<CliTerminal tmuxSessionId="tmux-1" embedded />)
      // Terminal mount div should exist and have h-full but no pt-5
      const mountDiv = container.querySelector('[class*="h-full"]')
      expect(mountDiv).not.toBeNull()
      expect(mountDiv?.className).not.toContain('pt-5')
    })
  })
})
