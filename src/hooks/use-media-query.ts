import { useState, useEffect, useCallback } from 'react'

/**
 * Standard breakpoints for responsive design.
 * Matches Tailwind CSS defaults.
 */
export const BREAKPOINTS = {
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  '2xl': 1536,
} as const

/**
 * useMediaQuery hook for responsive component behavior.
 *
 * Detects viewport size changes and returns a boolean indicating
 * whether the media query matches.
 *
 * @param query - CSS media query string (e.g., '(max-width: 640px)')
 * @returns boolean indicating if the media query matches
 *
 * @example
 * ```tsx
 * const isMobile = useMediaQuery('(max-width: 640px)')
 * const isTablet = useMediaQuery('(min-width: 640px) and (max-width: 1024px)')
 * const isDesktop = useMediaQuery('(min-width: 1024px)')
 * ```
 */
export function useMediaQuery(query: string): boolean {
  // Initialize with a function to avoid SSR issues
  const getMatches = useCallback(() => {
    // Check if window is available (browser environment)
    if (typeof window !== 'undefined') {
      return window.matchMedia(query).matches
    }
    return false
  }, [query])

  const [matches, setMatches] = useState(getMatches)

  useEffect(() => {
    // Create MediaQueryList for the query
    const mediaQueryList = window.matchMedia(query)

    // Handler for media query changes
    const handleChange = (event: MediaQueryListEvent) => {
      setMatches(event.matches)
    }

    // Set initial value from media query (may differ from initial useState)
    // This is safe because it only runs on mount/query change
    if (mediaQueryList.matches !== matches) {
      setMatches(mediaQueryList.matches)
    }

    // Add listener (using the modern API with fallback)
    if (mediaQueryList.addEventListener) {
      mediaQueryList.addEventListener('change', handleChange)
    } else {
      // Fallback for older browsers
      mediaQueryList.addListener(handleChange)
    }

    // Cleanup
    return () => {
      if (mediaQueryList.removeEventListener) {
        mediaQueryList.removeEventListener('change', handleChange)
      } else {
        // Fallback for older browsers
        mediaQueryList.removeListener(handleChange)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- matches is intentionally excluded to prevent infinite loops
  }, [query])

  return matches
}

/**
 * Convenience hook for detecting mobile viewport.
 * Mobile: < 640px (below sm breakpoint)
 */
export function useIsMobile(): boolean {
  return useMediaQuery(`(max-width: ${BREAKPOINTS.sm - 1}px)`)
}

/**
 * Convenience hook for detecting tablet viewport.
 * Tablet: 640px - 1023px (sm to below lg)
 */
export function useIsTablet(): boolean {
  return useMediaQuery(
    `(min-width: ${BREAKPOINTS.sm}px) and (max-width: ${BREAKPOINTS.lg - 1}px)`
  )
}

/**
 * Convenience hook for detecting desktop viewport.
 * Desktop: >= 1024px (lg and above)
 */
export function useIsDesktop(): boolean {
  return useMediaQuery(`(min-width: ${BREAKPOINTS.lg}px)`)
}

/**
 * Hook that returns current breakpoint name.
 * Useful for conditional rendering based on screen size.
 *
 * @returns 'mobile' | 'tablet' | 'desktop' | 'wide'
 */
export function useBreakpoint(): 'mobile' | 'tablet' | 'desktop' | 'wide' {
  const isMobile = useMediaQuery(`(max-width: ${BREAKPOINTS.sm - 1}px)`)
  const isTablet = useMediaQuery(
    `(min-width: ${BREAKPOINTS.sm}px) and (max-width: ${BREAKPOINTS.lg - 1}px)`
  )
  const isDesktop = useMediaQuery(
    `(min-width: ${BREAKPOINTS.lg}px) and (max-width: ${BREAKPOINTS.xl - 1}px)`
  )

  if (isMobile) return 'mobile'
  if (isTablet) return 'tablet'
  if (isDesktop) return 'desktop'
  return 'wide'
}
