import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { usePaneZoom } from '../use-pane-zoom'

function createMockPanel(overrides: Record<string, unknown> = {}) {
  return {
    id: 'panel-1',
    api: {
      maximize: vi.fn(),
      isMaximized: vi.fn(() => false),
      exitMaximized: vi.fn(),
    },
    ...overrides,
  }
}

type MaxChangeCallback = (e: { isMaximized: boolean }) => void

function createMockApi(overrides: Record<string, unknown> = {}) {
  const listeners: MaxChangeCallback[] = []
  // Dockview Event<T> is a callable: (listener) => IDisposable
  const onDidMaximizedGroupChange = vi.fn((cb: MaxChangeCallback) => {
    listeners.push(cb)
    return { dispose: vi.fn(() => listeners.splice(listeners.indexOf(cb), 1)) }
  })
  return {
    activePanel: createMockPanel(),
    hasMaximizedGroup: vi.fn(() => false),
    exitMaximizedGroup: vi.fn(),
    onDidMaximizedGroupChange,
    // Helper to trigger event in tests
    _triggerMaxChange: (isMaximized: boolean) => {
      for (const cb of listeners) cb({ isMaximized })
    },
    ...overrides,
  }
}

describe('usePaneZoom', () => {
  it('returns isZoomed=false initially when no group is maximized', () => {
    const api = createMockApi()
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))
    expect(result.current.isZoomed).toBe(false)
  })

  it('returns isZoomed=true when api reports maximized group', () => {
    const api = createMockApi({ hasMaximizedGroup: vi.fn(() => true) })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))
    expect(result.current.isZoomed).toBe(true)
  })

  it('toggleZoom maximizes active panel when not zoomed', () => {
    const panel = createMockPanel()
    const api = createMockApi({ activePanel: panel })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    act(() => {
      result.current.toggleZoom()
    })

    expect(panel.api.maximize).toHaveBeenCalledTimes(1)
  })

  it('toggleZoom exits maximize when already zoomed', () => {
    const api = createMockApi({ hasMaximizedGroup: vi.fn(() => true) })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    act(() => {
      result.current.toggleZoom()
    })

    expect(api.exitMaximizedGroup).toHaveBeenCalledTimes(1)
  })

  it('zoomPanel maximizes a specific panel by ID', () => {
    const panel = createMockPanel({ id: 'target-panel' })
    const getPanel = vi.fn(() => panel)
    const api = createMockApi({ getPanel })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    act(() => {
      result.current.zoomPanel('target-panel')
    })

    expect(getPanel).toHaveBeenCalledWith('target-panel')
    expect(panel.api.maximize).toHaveBeenCalledTimes(1)
  })

  it('zoomPanel is a no-op if panel not found', () => {
    const api = createMockApi({ getPanel: vi.fn(() => undefined) })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    // Should not throw
    act(() => {
      result.current.zoomPanel('nonexistent')
    })
  })

  it('exitZoom calls exitMaximizedGroup', () => {
    const api = createMockApi({ hasMaximizedGroup: vi.fn(() => true) })
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    act(() => {
      result.current.exitZoom()
    })

    expect(api.exitMaximizedGroup).toHaveBeenCalledTimes(1)
  })

  it('updates isZoomed when onDidMaximizedGroupChange fires', () => {
    const api = createMockApi()
    const { result } = renderHook(() => usePaneZoom({ api: api as never }))

    expect(result.current.isZoomed).toBe(false)

    act(() => {
      api._triggerMaxChange(true)
    })

    expect(result.current.isZoomed).toBe(true)

    act(() => {
      api._triggerMaxChange(false)
    })

    expect(result.current.isZoomed).toBe(false)
  })

  it('returns no-op functions when api is null', () => {
    const { result } = renderHook(() => usePaneZoom({ api: null }))

    expect(result.current.isZoomed).toBe(false)

    // Should not throw
    act(() => {
      result.current.toggleZoom()
      result.current.zoomPanel('any')
      result.current.exitZoom()
    })
  })

  it('disposes event listener on unmount', () => {
    const dispose = vi.fn()
    const api = createMockApi()
    ;(api.onDidMaximizedGroupChange as ReturnType<typeof vi.fn>).mockReturnValue({ dispose })

    const { unmount } = renderHook(() => usePaneZoom({ api: api as never }))
    unmount()

    expect(dispose).toHaveBeenCalledTimes(1)
  })

  it('re-subscribes when api changes', () => {
    const api1 = createMockApi()
    const api2 = createMockApi()

    const { rerender } = renderHook(({ api }) => usePaneZoom({ api: api as never }), {
      initialProps: { api: api1 },
    })

    expect(api1.onDidMaximizedGroupChange).toHaveBeenCalledTimes(1)

    rerender({ api: api2 })

    expect(api2.onDidMaximizedGroupChange).toHaveBeenCalledTimes(1)
  })
})
