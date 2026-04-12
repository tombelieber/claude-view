import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { usePaneZoom } from '../use-pane-zoom'

function createMockPanel(id = 'panel-1') {
  return {
    id,
    api: {
      maximize: vi.fn(),
      isMaximized: vi.fn(() => false),
      exitMaximized: vi.fn(),
    },
  }
}

type MaxChangeCallback = (e: { isMaximized: boolean }) => void

function createMockApi(overrides: Record<string, unknown> = {}) {
  const listeners: MaxChangeCallback[] = []
  return {
    activePanel: createMockPanel(),
    hasMaximizedGroup: vi.fn(() => false),
    exitMaximizedGroup: vi.fn(),
    getPanel: vi.fn(() => undefined),
    onDidMaximizedGroupChange: vi.fn((cb: MaxChangeCallback) => {
      listeners.push(cb)
      return { dispose: vi.fn() }
    }),
    _triggerMaxChange: (isMaximized: boolean) => {
      for (const cb of listeners) cb({ isMaximized })
    },
    ...overrides,
  }
}

describe('usePaneZoom — edge cases', () => {
  it('toggleZoom does nothing when api has no activePanel', () => {
    const api = createMockApi({ activePanel: null })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    // Should not throw
    act(() => {
      result.current.toggleZoom()
    })

    expect(api.exitMaximizedGroup).not.toHaveBeenCalled()
  })

  it('exitZoom is idempotent — safe to call when not zoomed', () => {
    const api = createMockApi()
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    act(() => {
      result.current.exitZoom()
      result.current.exitZoom()
    })

    expect(api.exitMaximizedGroup).toHaveBeenCalledTimes(2)
  })

  it('isZoomed stays synced through rapid toggle', () => {
    const api = createMockApi()
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    expect(result.current.isZoomed).toBe(false)

    act(() => api._triggerMaxChange(true))
    expect(result.current.isZoomed).toBe(true)

    act(() => api._triggerMaxChange(false))
    expect(result.current.isZoomed).toBe(false)

    act(() => api._triggerMaxChange(true))
    expect(result.current.isZoomed).toBe(true)
  })

  it('handles api going from valid to null', () => {
    const api = createMockApi()
    const { result, rerender } = renderHook(({ api }) => usePaneZoom({ api: api as never }), {
      initialProps: { api },
    })

    act(() => api._triggerMaxChange(true))
    expect(result.current.isZoomed).toBe(true)

    // API goes away
    rerender({ api: null as never })
    expect(result.current.isZoomed).toBe(false)
  })
})
