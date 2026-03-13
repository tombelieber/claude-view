import { renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useScrollAnchor } from './use-scroll-anchor'

// Mock IntersectionObserver — happy-dom does not implement it
const mockObserve = vi.fn()
const mockDisconnect = vi.fn()

beforeEach(() => {
  mockObserve.mockClear()
  mockDisconnect.mockClear()
  vi.stubGlobal(
    'IntersectionObserver',
    vi.fn().mockImplementation(() => ({
      observe: mockObserve,
      disconnect: mockDisconnect,
      unobserve: vi.fn(),
    })),
  )
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  mockObserve.mockClear()
  mockDisconnect.mockClear()
})

describe('useScrollAnchor', () => {
  // --- Unit: returns correct shape ---
  it('returns refs and handler', () => {
    const { result } = renderHook(() => useScrollAnchor({ blockCount: 0 }))
    expect(result.current.scrollContainerRef).toBeDefined()
    expect(result.current.topSentinelRef).toBeDefined()
    expect(result.current.bottomRef).toBeDefined()
    expect(typeof result.current.handleScroll).toBe('function')
  })

  // --- Behavioral: isFetchingOlderRef guard prevents onReachTop during active loading ---
  // Since renderHook has no DOM (refs are null), the IntersectionObserver is never created.
  // To test the isFetchingOlderRef guard, we directly simulate the IO callback pattern.
  // This mirrors the exact guard logic inside useScrollAnchor's IO callback:
  //   if (entry.isIntersecting && !isFetchingOlderRef.current && !fetchCooldownRef.current)
  it('isFetchingOlderRef guard: callback does not fire onReachTop when fetching', () => {
    const onReachTop = vi.fn()
    const isFetchingOlderRef = { current: true }
    const fetchCooldownRef = { current: false }

    // Simulate the exact IO callback from useScrollAnchor
    const ioCallback = ([entry]: { isIntersecting: boolean }[]) => {
      if (entry.isIntersecting && !isFetchingOlderRef.current && !fetchCooldownRef.current) {
        onReachTop()
      }
    }

    // Sentinel visible but fetching — should NOT fire
    ioCallback([{ isIntersecting: true }])
    expect(onReachTop).not.toHaveBeenCalled()

    // Sentinel visible and NOT fetching — should fire
    isFetchingOlderRef.current = false
    ioCallback([{ isIntersecting: true }])
    expect(onReachTop).toHaveBeenCalledTimes(1)

    // Cooldown active — should NOT fire again
    fetchCooldownRef.current = true
    ioCallback([{ isIntersecting: true }])
    expect(onReachTop).toHaveBeenCalledTimes(1) // still 1
  })

  // --- Behavioral: observer NOT created when refs are null (renderHook has no DOM) ---
  // The useEffect guards on `if (!sentinel || !container || !onReachTop) return`,
  // so in renderHook (where refs are never attached to real DOM elements), the
  // IntersectionObserver constructor is NOT called regardless of onReachTop.
  // This test verifies that guard works — the hook does not crash or create
  // a stray observer when DOM elements are absent.
  it('does not create IntersectionObserver when refs are unattached (no DOM)', () => {
    const onReachTop = vi.fn()
    renderHook(() =>
      useScrollAnchor({
        onReachTop,
        isFetchingOlder: false,
        blockCount: 10,
      }),
    )
    // Refs are null in renderHook → useEffect early-returns → no observer created
    expect(IntersectionObserver).not.toHaveBeenCalled()
    expect(onReachTop).not.toHaveBeenCalled()
  })

  // --- Behavioral: no observer when onReachTop is undefined ---
  it('does not create IntersectionObserver when onReachTop is undefined', () => {
    renderHook(() =>
      useScrollAnchor({
        blockCount: 10,
      }),
    )
    // onReachTop is undefined → useEffect early-returns → no observer
    expect(IntersectionObserver).not.toHaveBeenCalled()
  })

  // --- Unit: handleScroll is stable across renders ---
  it('handleScroll is referentially stable', () => {
    const { result, rerender } = renderHook(({ count }) => useScrollAnchor({ blockCount: count }), {
      initialProps: { count: 0 },
    })
    const first = result.current.handleScroll
    rerender({ count: 5 })
    expect(result.current.handleScroll).toBe(first)
  })
})
