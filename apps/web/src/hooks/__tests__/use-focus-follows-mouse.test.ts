import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useFocusFollowsMouse } from '../use-focus-follows-mouse'

describe('useFocusFollowsMouse', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('returns a getHandlers function', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: true }))
    expect(typeof result.current.getHandlers).toBe('function')
  })

  it('fires activation callback after delay on pointer enter', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: true, delayMs: 150 }))
    const activate = vi.fn()
    const handlers = result.current.getHandlers(activate)

    act(() => {
      handlers.onPointerEnter()
    })

    // Not yet fired — still within delay
    expect(activate).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(150)
    })

    expect(activate).toHaveBeenCalledTimes(1)
  })

  it('does not fire if pointer leaves before delay expires', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: true, delayMs: 150 }))
    const activate = vi.fn()
    const handlers = result.current.getHandlers(activate)

    act(() => {
      handlers.onPointerEnter()
    })

    act(() => {
      vi.advanceTimersByTime(100) // only 100ms of 150ms
      handlers.onPointerLeave()
    })

    act(() => {
      vi.advanceTimersByTime(100) // past the original 150ms
    })

    expect(activate).not.toHaveBeenCalled()
  })

  it('does nothing when disabled', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: false }))
    const activate = vi.fn()
    const handlers = result.current.getHandlers(activate)

    act(() => {
      handlers.onPointerEnter()
    })

    act(() => {
      vi.advanceTimersByTime(500)
    })

    expect(activate).not.toHaveBeenCalled()
  })

  it('uses default delay of 150ms when delayMs not specified', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: true }))
    const activate = vi.fn()
    const handlers = result.current.getHandlers(activate)

    act(() => {
      handlers.onPointerEnter()
    })

    act(() => {
      vi.advanceTimersByTime(149)
    })
    expect(activate).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(activate).toHaveBeenCalledTimes(1)
  })

  it('cancels pending timer on unmount', () => {
    const { result, unmount } = renderHook(() => useFocusFollowsMouse({ enabled: true }))
    const activate = vi.fn()
    const handlers = result.current.getHandlers(activate)

    act(() => {
      handlers.onPointerEnter()
    })

    unmount()

    act(() => {
      vi.advanceTimersByTime(300)
    })

    expect(activate).not.toHaveBeenCalled()
  })

  it('replaces pending timer if pointer enters again before first fires', () => {
    const { result } = renderHook(() => useFocusFollowsMouse({ enabled: true, delayMs: 100 }))
    const activate1 = vi.fn()
    const activate2 = vi.fn()
    const handlers1 = result.current.getHandlers(activate1)
    const handlers2 = result.current.getHandlers(activate2)

    act(() => {
      handlers1.onPointerEnter()
    })

    act(() => {
      vi.advanceTimersByTime(50)
    })

    // Enter a different pane — cancels first timer
    act(() => {
      handlers2.onPointerEnter()
    })

    act(() => {
      vi.advanceTimersByTime(100)
    })

    expect(activate1).not.toHaveBeenCalled()
    expect(activate2).toHaveBeenCalledTimes(1)
  })

  it('getHandlers is referentially stable across rerenders', () => {
    const { result, rerender } = renderHook(() => useFocusFollowsMouse({ enabled: true }))
    const first = result.current.getHandlers
    rerender()
    expect(result.current.getHandlers).toBe(first)
  })
})
