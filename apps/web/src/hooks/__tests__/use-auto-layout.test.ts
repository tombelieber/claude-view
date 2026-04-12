import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MIN_PANE_H, MIN_PANE_W, computeMaxVisibleCols, useAutoLayout } from '../use-auto-layout'

// --- Pure function tests ---

describe('computeMaxVisibleCols', () => {
  it('returns 4 for 1600px viewport', () => {
    expect(computeMaxVisibleCols(1600)).toBe(4)
  })

  it('returns 3 for 1200px viewport', () => {
    expect(computeMaxVisibleCols(1200)).toBe(3)
  })

  it('returns 2 for 800px viewport', () => {
    expect(computeMaxVisibleCols(800)).toBe(2)
  })

  it('returns 1 for 399px viewport', () => {
    expect(computeMaxVisibleCols(399)).toBe(0)
  })

  it('returns 1 for exactly MIN_PANE_W', () => {
    expect(computeMaxVisibleCols(MIN_PANE_W)).toBe(1)
  })

  it('returns 2 for 2 * MIN_PANE_W', () => {
    expect(computeMaxVisibleCols(MIN_PANE_W * 2)).toBe(2)
  })

  it('returns 0 for zero width', () => {
    expect(computeMaxVisibleCols(0)).toBe(0)
  })
})

describe('constants', () => {
  it('MIN_PANE_W is 400', () => {
    expect(MIN_PANE_W).toBe(400)
  })

  it('MIN_PANE_H is 200', () => {
    expect(MIN_PANE_H).toBe(200)
  })
})

// --- Hook tests ---

function createMockApi(options: { panelCount?: number; groups?: unknown[] } = {}) {
  const panelCount = options.panelCount ?? 4
  const panels = Array.from({ length: panelCount }, (_, i) => ({
    id: `panel-${i}`,
    api: {
      setActive: vi.fn(),
      maximize: vi.fn(),
      isMaximized: vi.fn(() => false),
      exitMaximized: vi.fn(),
      isVisible: true,
    },
    group: { id: `group-${i}`, panels: [] as unknown[] },
    title: `Panel ${i}`,
    params: { sessionId: `session-${i}` },
  }))
  // Default: each panel in its own group (all visible)
  for (const p of panels) {
    p.group.panels = [p]
  }
  return {
    panels,
    groups: options.groups ?? panels.map((p) => p.group),
    addPanel: vi.fn(),
    removePanel: vi.fn(),
    moveGroupOrPanel: vi.fn(),
    hasMaximizedGroup: vi.fn(() => false),
    getPanel: vi.fn((id: string) => panels.find((p) => p.id === id)),
  }
}

describe('useAutoLayout', () => {
  type ResizeCb = (entries: { contentRect: { width: number } }[]) => void
  let resizeCallbacks: ResizeCb[]
  let lastDisconnect: ReturnType<typeof vi.fn>

  beforeEach(() => {
    resizeCallbacks = []
    lastDisconnect = vi.fn()
    const localDisconnect = lastDisconnect
    // Class-based mock so `new ResizeObserver(...)` works
    class MockResizeObserver {
      constructor(cb: ResizeCb) {
        resizeCallbacks.push(cb)
      }
      observe = vi.fn()
      disconnect = localDisconnect
      unobserve = vi.fn()
    }
    vi.stubGlobal('ResizeObserver', MockResizeObserver)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('returns engine object with maxVisibleCols', () => {
    const { result } = renderHook(() =>
      useAutoLayout({ api: null, containerRef: { current: null }, enabled: true }),
    )
    expect(result.current.maxVisibleCols).toBe(0)
  })

  it('computes maxVisibleCols from container width', () => {
    const container = document.createElement('div')
    const api = createMockApi({ panelCount: 4 })

    const { result } = renderHook(() =>
      useAutoLayout({
        api: api as never,
        containerRef: { current: container },
        enabled: true,
      }),
    )

    // Simulate resize to 1600px
    act(() => {
      for (const cb of resizeCallbacks) {
        cb([{ contentRect: { width: 1600 } }])
      }
    })

    expect(result.current.maxVisibleCols).toBe(4)
  })

  it('updates maxVisibleCols when container resizes', () => {
    const container = document.createElement('div')
    const api = createMockApi({ panelCount: 4 })

    const { result } = renderHook(() =>
      useAutoLayout({
        api: api as never,
        containerRef: { current: container },
        enabled: true,
      }),
    )

    act(() => {
      for (const cb of resizeCallbacks) {
        cb([{ contentRect: { width: 1600 } }])
      }
    })
    expect(result.current.maxVisibleCols).toBe(4)

    act(() => {
      for (const cb of resizeCallbacks) {
        cb([{ contentRect: { width: 800 } }])
      }
    })
    expect(result.current.maxVisibleCols).toBe(2)
  })

  it('does not observe when disabled', () => {
    const container = document.createElement('div')
    renderHook(() =>
      useAutoLayout({ api: null, containerRef: { current: container }, enabled: false }),
    )

    // ResizeObserver should not have been called with observe
    expect(resizeCallbacks.length).toBe(0)
  })

  it('disconnects ResizeObserver on unmount', () => {
    const container = document.createElement('div')

    const { unmount } = renderHook(() =>
      useAutoLayout({ api: null, containerRef: { current: container }, enabled: true }),
    )

    unmount()
    expect(lastDisconnect).toHaveBeenCalled()
  })

  it('returns maxVisibleCols=0 when container is null', () => {
    const { result } = renderHook(() =>
      useAutoLayout({ api: null, containerRef: { current: null }, enabled: true }),
    )
    expect(result.current.maxVisibleCols).toBe(0)
  })
})
