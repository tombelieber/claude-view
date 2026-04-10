import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

/**
 * xterm.js Terminal requires a real DOM with canvas support,
 * which happy-dom cannot provide. We mock the module so the
 * hook's state-management logic can be tested in isolation.
 */
vi.mock('@xterm/xterm', () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    loadAddon: vi.fn(),
    open: vi.fn(),
    onData: vi.fn(() => ({ dispose: vi.fn() })),
    write: vi.fn(),
    dispose: vi.fn(),
    cols: 80,
    rows: 24,
  })),
}))

vi.mock('@xterm/addon-fit', () => ({
  FitAddon: vi.fn().mockImplementation(() => ({
    fit: vi.fn(),
    dispose: vi.fn(),
  })),
}))

vi.mock('@xterm/addon-webgl', () => ({
  WebglAddon: vi.fn().mockImplementation(() => ({
    onContextLoss: vi.fn(),
    dispose: vi.fn(),
  })),
}))

// Must import after mocks are set up
const { useCliTerminal } = await import('../useCliTerminal')

describe('useCliTerminal', () => {
  it('returns disconnected state when tmuxSessionId is null', () => {
    const containerRef = { current: document.createElement('div') }
    const { result } = renderHook(() => useCliTerminal({ tmuxSessionId: null, containerRef }))

    expect(result.current.isConnected).toBe(false)
    expect(result.current.error).toBeNull()
  })

  it('returns a callable sendKeys function even when disconnected', () => {
    const containerRef = { current: document.createElement('div') }
    const { result } = renderHook(() => useCliTerminal({ tmuxSessionId: null, containerRef }))

    expect(typeof result.current.sendKeys).toBe('function')
    // Should not throw when called while disconnected
    expect(() => result.current.sendKeys('hello')).not.toThrow()
  })

  it('stays disconnected when container ref is null', () => {
    const containerRef = { current: null }
    const { result } = renderHook(() =>
      useCliTerminal({ tmuxSessionId: 'test-session', containerRef }),
    )

    expect(result.current.isConnected).toBe(false)
    expect(result.current.error).toBeNull()
  })
})
