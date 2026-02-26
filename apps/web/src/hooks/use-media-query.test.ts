import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useMediaQuery, useIsMobile, useIsTablet, useIsDesktop, useBreakpoint, BREAKPOINTS } from './use-media-query'

// Mock matchMedia
interface MockMediaQueryList {
  matches: boolean
  media: string
  onchange: null
  addEventListener: ReturnType<typeof vi.fn>
  removeEventListener: ReturnType<typeof vi.fn>
  addListener: ReturnType<typeof vi.fn>
  removeListener: ReturnType<typeof vi.fn>
  dispatchEvent: ReturnType<typeof vi.fn>
}

function createMockMediaQueryList(matches: boolean): MockMediaQueryList {
  return {
    matches,
    media: '',
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(), // Deprecated but needed for fallback
    removeListener: vi.fn(), // Deprecated but needed for fallback
    dispatchEvent: vi.fn(),
  }
}

describe('useMediaQuery', () => {
  let mockMatchMedia: ReturnType<typeof vi.fn>
  let mockMediaQueryList: MockMediaQueryList

  beforeEach(() => {
    mockMediaQueryList = createMockMediaQueryList(false)
    mockMatchMedia = vi.fn(() => mockMediaQueryList)
    window.matchMedia = mockMatchMedia
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('basic functionality', () => {
    it('returns false when media query does not match', () => {
      mockMediaQueryList.matches = false
      const { result } = renderHook(() => useMediaQuery('(max-width: 640px)'))
      expect(result.current).toBe(false)
    })

    it('returns true when media query matches', () => {
      mockMediaQueryList.matches = true
      const { result } = renderHook(() => useMediaQuery('(max-width: 640px)'))
      expect(result.current).toBe(true)
    })

    it('calls matchMedia with the provided query', () => {
      renderHook(() => useMediaQuery('(max-width: 640px)'))
      expect(mockMatchMedia).toHaveBeenCalledWith('(max-width: 640px)')
    })

    it('adds event listener on mount', () => {
      renderHook(() => useMediaQuery('(max-width: 640px)'))
      expect(mockMediaQueryList.addEventListener).toHaveBeenCalledWith(
        'change',
        expect.any(Function)
      )
    })

    it('removes event listener on unmount', () => {
      const { unmount } = renderHook(() => useMediaQuery('(max-width: 640px)'))
      unmount()
      expect(mockMediaQueryList.removeEventListener).toHaveBeenCalledWith(
        'change',
        expect.any(Function)
      )
    })
  })

  describe('dynamic updates', () => {
    it('updates when media query changes', () => {
      mockMediaQueryList.matches = false
      const { result } = renderHook(() => useMediaQuery('(max-width: 640px)'))

      expect(result.current).toBe(false)

      // Simulate a media query change
      act(() => {
        const changeHandler = mockMediaQueryList.addEventListener.mock.calls[0][1]
        changeHandler({ matches: true } as MediaQueryListEvent)
      })

      expect(result.current).toBe(true)
    })

    it('updates when query prop changes', () => {
      mockMediaQueryList.matches = false
      const { result, rerender } = renderHook(
        ({ query }) => useMediaQuery(query),
        { initialProps: { query: '(max-width: 640px)' } }
      )

      expect(result.current).toBe(false)

      // Change to a matching query
      mockMediaQueryList.matches = true
      rerender({ query: '(max-width: 1024px)' })

      expect(result.current).toBe(true)
    })
  })
})

describe('useIsMobile', () => {
  beforeEach(() => {
    window.matchMedia = vi.fn((query) =>
      createMockMediaQueryList(query === `(max-width: ${BREAKPOINTS.sm - 1}px)`)
    )
  })

  it('returns true for mobile viewport', () => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(true))
    const { result } = renderHook(() => useIsMobile())
    expect(result.current).toBe(true)
  })

  it('returns false for non-mobile viewport', () => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(false))
    const { result } = renderHook(() => useIsMobile())
    expect(result.current).toBe(false)
  })

  it('uses correct breakpoint query', () => {
    const mockMatchMedia = vi.fn(() => createMockMediaQueryList(false))
    window.matchMedia = mockMatchMedia
    renderHook(() => useIsMobile())
    expect(mockMatchMedia).toHaveBeenCalledWith('(max-width: 639px)')
  })
})

describe('useIsTablet', () => {
  it('uses correct breakpoint query range', () => {
    const mockMatchMedia = vi.fn(() => createMockMediaQueryList(false))
    window.matchMedia = mockMatchMedia
    renderHook(() => useIsTablet())
    expect(mockMatchMedia).toHaveBeenCalledWith(
      `(min-width: ${BREAKPOINTS.sm}px) and (max-width: ${BREAKPOINTS.lg - 1}px)`
    )
  })

  it('returns true when in tablet range', () => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(true))
    const { result } = renderHook(() => useIsTablet())
    expect(result.current).toBe(true)
  })
})

describe('useIsDesktop', () => {
  it('uses correct breakpoint query', () => {
    const mockMatchMedia = vi.fn(() => createMockMediaQueryList(false))
    window.matchMedia = mockMatchMedia
    renderHook(() => useIsDesktop())
    expect(mockMatchMedia).toHaveBeenCalledWith(`(min-width: ${BREAKPOINTS.lg}px)`)
  })

  it('returns true when desktop or wider', () => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(true))
    const { result } = renderHook(() => useIsDesktop())
    expect(result.current).toBe(true)
  })
})

describe('useBreakpoint', () => {
  it('returns "mobile" for mobile viewport', () => {
    window.matchMedia = vi.fn((query: string) => {
      // Mobile query matches
      if (query === `(max-width: ${BREAKPOINTS.sm - 1}px)`) {
        return createMockMediaQueryList(true)
      }
      return createMockMediaQueryList(false)
    })
    const { result } = renderHook(() => useBreakpoint())
    expect(result.current).toBe('mobile')
  })

  it('returns "tablet" for tablet viewport', () => {
    window.matchMedia = vi.fn((query: string) => {
      // Tablet query matches
      if (query === `(min-width: ${BREAKPOINTS.sm}px) and (max-width: ${BREAKPOINTS.lg - 1}px)`) {
        return createMockMediaQueryList(true)
      }
      return createMockMediaQueryList(false)
    })
    const { result } = renderHook(() => useBreakpoint())
    expect(result.current).toBe('tablet')
  })

  it('returns "desktop" for desktop viewport', () => {
    window.matchMedia = vi.fn((query: string) => {
      // Desktop query matches
      if (query === `(min-width: ${BREAKPOINTS.lg}px) and (max-width: ${BREAKPOINTS.xl - 1}px)`) {
        return createMockMediaQueryList(true)
      }
      return createMockMediaQueryList(false)
    })
    const { result } = renderHook(() => useBreakpoint())
    expect(result.current).toBe('desktop')
  })

  it('returns "wide" for wide viewport', () => {
    window.matchMedia = vi.fn(() => createMockMediaQueryList(false))
    const { result } = renderHook(() => useBreakpoint())
    expect(result.current).toBe('wide')
  })
})

describe('BREAKPOINTS', () => {
  it('exports standard Tailwind breakpoints', () => {
    expect(BREAKPOINTS.sm).toBe(640)
    expect(BREAKPOINTS.md).toBe(768)
    expect(BREAKPOINTS.lg).toBe(1024)
    expect(BREAKPOINTS.xl).toBe(1280)
    expect(BREAKPOINTS['2xl']).toBe(1536)
  })
})
