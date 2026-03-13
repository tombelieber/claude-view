import { useCallback, useEffect, useLayoutEffect, useRef } from 'react'

interface UseScrollAnchorOptions {
  /** Call when user scrolls to top sentinel */
  onReachTop?: () => void
  /** Whether older content is being loaded (suppresses auto-scroll and re-fetch) */
  isFetchingOlder?: boolean
  /** Total block count — used to detect new content at bottom */
  blockCount: number
}

export function useScrollAnchor({
  onReachTop,
  isFetchingOlder,
  blockCount,
}: UseScrollAnchorOptions) {
  const scrollContainerRef = useRef<HTMLDivElement>(null)
  const topSentinelRef = useRef<HTMLDivElement>(null)
  const bottomRef = useRef<HTMLDivElement>(null)
  const prevBlockCountRef = useRef(0)
  const isNearBottomRef = useRef(true)
  const prevScrollHeightRef = useRef(0)
  const wasFetchingRef = useRef(false)

  // Keep isFetchingOlder in a ref so the IntersectionObserver callback always
  // reads the latest value without recreating the observer on every change.
  // Pattern: react-infinite-scroll-component uses the same approach.
  const isFetchingOlderRef = useRef(isFetchingOlder)
  isFetchingOlderRef.current = isFetchingOlder

  // Track whether user is near bottom (within 100px)
  const handleScroll = useCallback(() => {
    const el = scrollContainerRef.current
    if (!el) return
    isNearBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 100
  }, [])

  // Auto-scroll to bottom on initial load + session navigation reset.
  // Uses useLayoutEffect (not useEffect) to prevent visible flash before scroll.
  // The prevBlockCountRef reset is inside the effect (not in the render body) to avoid
  // double-firing under React Strict Mode, which re-renders components in development.
  useLayoutEffect(() => {
    // Reset when blockCount drops to 0 (session change).
    // Without this, navigating from a 100-msg session to a new session that loads
    // 50 msgs would skip the initial auto-scroll (50 > 0 but prevBlockCountRef = 100).
    if (blockCount === 0) {
      prevBlockCountRef.current = 0
      return
    }

    if (blockCount > 0 && prevBlockCountRef.current === 0) {
      // Initial load — scroll to bottom
      requestAnimationFrame(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'instant' })
      })
    } else if (blockCount > prevBlockCountRef.current && isNearBottomRef.current) {
      // New messages added while near bottom — follow
      requestAnimationFrame(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
      })
    }
    prevBlockCountRef.current = blockCount
  }, [blockCount])

  // Scroll position preservation for upward pagination.
  // Both save and restore happen in useLayoutEffect — this guarantees the scrollHeight
  // is captured and adjusted synchronously before the browser paints, eliminating the
  // race condition where useEffect (post-paint) would capture the WRONG scrollHeight
  // after React has already rendered the prepended blocks.
  //
  // Pattern proven at scale: Slack and Discord both use synchronous DOM measurement
  // in their scroll anchoring logic (layout-phase, not post-paint).
  useLayoutEffect(() => {
    const el = scrollContainerRef.current
    if (!el) return

    if (isFetchingOlder && !wasFetchingRef.current) {
      // Transition: not-fetching → fetching — save current scroll height NOW,
      // before any new blocks are rendered. useLayoutEffect runs synchronously
      // after React commits DOM changes but BEFORE the browser paints.
      prevScrollHeightRef.current = el.scrollHeight
    } else if (!isFetchingOlder && wasFetchingRef.current && prevScrollHeightRef.current > 0) {
      // Transition: fetching → done — new blocks are in the DOM.
      // Adjust scrollTop by the height delta to keep the viewport anchored.
      const heightDiff = el.scrollHeight - prevScrollHeightRef.current
      if (heightDiff > 0) {
        el.scrollTop += heightDiff
      }
      prevScrollHeightRef.current = 0
    }

    wasFetchingRef.current = !!isFetchingOlder
  }, [blockCount, isFetchingOlder])

  // IntersectionObserver on top sentinel — triggers loading older messages.
  // The observer is created ONCE per onReachTop change (not on isFetchingOlder change).
  // isFetchingOlder is read from a ref inside the callback to avoid recreating the
  // observer, which would fire immediately on re-observe when sentinel is still visible.
  const fetchCooldownRef = useRef(false)

  useEffect(() => {
    const sentinel = topSentinelRef.current
    const container = scrollContainerRef.current
    if (!sentinel || !container || !onReachTop) return

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !isFetchingOlderRef.current && !fetchCooldownRef.current) {
          fetchCooldownRef.current = true
          onReachTop()
          // 200ms cooldown prevents rapid-fire when sentinel re-appears after prepend
          setTimeout(() => {
            fetchCooldownRef.current = false
          }, 200)
        }
      },
      { root: container, threshold: 0.1 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [onReachTop]) // Only recreate on onReachTop change — NOT on isFetchingOlder

  return { scrollContainerRef, topSentinelRef, bottomRef, handleScroll }
}
