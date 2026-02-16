import { useRef, useEffect, useState, useCallback, useMemo } from 'react'
import { Monitor } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { LiveSession } from './use-live-sessions'

interface MonitorGridProps {
  sessions: LiveSession[]
  gridOverride: { cols: number; rows: number } | null
  compactHeaders: boolean
  children: React.ReactNode
  onVisibilityChange?: (visiblePanes: Set<string>) => void
}

/**
 * MonitorGrid — CSS Grid container that arranges MonitorPane children responsively.
 *
 * Auto mode: CSS Grid with auto-fill and minmax for responsive breakpoints.
 * Override mode: explicit cols x rows when the user chooses a layout.
 * Mobile (< 640px): horizontal scroll-snap with dot indicators.
 *
 * Tracks which panes are visible via IntersectionObserver so consumers can
 * optimize WebSocket connect/disconnect for off-screen panes.
 */
export function MonitorGrid({ sessions, gridOverride, compactHeaders, children, onVisibilityChange }: MonitorGridProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [visiblePanes, setVisiblePanes] = useState<Set<string>>(new Set())
  const [activeMobileIndex, setActiveMobileIndex] = useState(0)
  const [isMobile, setIsMobile] = useState(false)

  // Detect mobile breakpoint (< 640px)
  useEffect(() => {
    function checkMobile() {
      setIsMobile(window.innerWidth < 640)
    }
    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  // IntersectionObserver to track which panes are visible
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const observer = new IntersectionObserver(
      (entries) => {
        setVisiblePanes((prev) => {
          const next = new Set(prev)
          for (const entry of entries) {
            const el = entry.target as HTMLElement
            const sessionId = el.dataset.paneId
            if (!sessionId) continue
            if (entry.isIntersecting) {
              next.add(sessionId)
            } else {
              next.delete(sessionId)
            }
          }
          return next
        })
      },
      { root: container, threshold: 0.1 }
    )

    // Observe all pane children
    const panes = container.querySelectorAll('[data-pane-id]')
    panes.forEach((pane) => observer.observe(pane))

    return () => observer.disconnect()
  }, [sessions.length, gridOverride, isMobile])

  // Notify parent when visible panes change
  useEffect(() => {
    onVisibilityChange?.(visiblePanes)
  }, [visiblePanes, onVisibilityChange])

  // Mobile scroll handler — track active dot
  const handleMobileScroll = useCallback(() => {
    const container = containerRef.current
    if (!container) return
    const scrollLeft = container.scrollLeft
    const paneWidth = container.clientWidth
    if (paneWidth > 0) {
      setActiveMobileIndex(Math.round(scrollLeft / paneWidth))
    }
  }, [])

  // Grid style for auto and override modes
  const gridStyle = useMemo(() => {
    if (isMobile) return undefined

    if (gridOverride) {
      return {
        display: 'grid' as const,
        gap: '4px',
        gridTemplateColumns: `repeat(${gridOverride.cols}, 1fr)`,
        gridTemplateRows: `repeat(${gridOverride.rows}, 1fr)`,
        height: '100%',
        overflow: 'hidden' as const,
      }
    }

    return {
      display: 'grid' as const,
      gap: '4px',
      gridTemplateColumns: 'repeat(auto-fill, minmax(480px, 1fr))',
      gridAutoRows: 'minmax(300px, 1fr)',
      height: '100%',
      overflow: 'hidden' as const,
    }
  }, [gridOverride, isMobile])

  // Empty state
  if (sessions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-gray-400 dark:text-gray-500">
        <Monitor className="h-10 w-10 mb-3 text-gray-300 dark:text-gray-600" />
        <p className="text-sm font-medium text-gray-500 dark:text-gray-400">
          No active sessions to monitor
        </p>
        <p className="text-xs mt-1">
          Start a Claude Code session in your terminal
        </p>
      </div>
    )
  }

  // Mobile: horizontal scroll-snap layout
  if (isMobile) {
    return (
      <div className="relative h-full">
        <div
          ref={containerRef}
          className="flex overflow-x-auto snap-x snap-mandatory h-full scrollbar-hide"
          style={{ WebkitOverflowScrolling: 'touch' }}
          onScroll={handleMobileScroll}
        >
          {sessions.map((session) => (
            <div
              key={session.id}
              data-pane-id={session.id}
              className="snap-start min-w-full flex-shrink-0 h-full"
            >
              {children}
            </div>
          ))}
        </div>

        {/* Dot indicators */}
        {sessions.length > 1 && (
          <div className="absolute bottom-2 left-0 right-0 flex items-center justify-center gap-1.5">
            {sessions.map((session, idx) => (
              <button
                key={session.id}
                type="button"
                aria-label={`Go to session ${idx + 1}`}
                className={cn(
                  'h-1.5 rounded-full transition-all',
                  idx === activeMobileIndex
                    ? 'w-4 bg-indigo-500'
                    : 'w-1.5 bg-gray-400 dark:bg-gray-600'
                )}
                onClick={() => {
                  const container = containerRef.current
                  if (container) {
                    container.scrollTo({
                      left: idx * container.clientWidth,
                      behavior: 'smooth',
                    })
                  }
                }}
              />
            ))}
          </div>
        )}
      </div>
    )
  }

  // Desktop / Tablet: CSS Grid layout
  return (
    <div
      ref={containerRef}
      style={gridStyle}
      className={cn(
        'w-full',
        compactHeaders && 'monitor-grid--compact'
      )}
    >
      {children}
    </div>
  )
}

/** Context value for pane visibility — consumers can check if their pane is on-screen. */
export type { MonitorGridProps }
