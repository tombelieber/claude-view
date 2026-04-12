import type { DockviewApi } from 'dockview-react'
import { type RefObject, useEffect, useState } from 'react'

/** Minimum pane width in pixels. 50 cols × ~7.8px/char + padding. */
export const MIN_PANE_W = 400
/** Minimum pane height in pixels. ~9 rows at 13px line-height + 40px tab. */
export const MIN_PANE_H = 200

/** Pure function: how many panes fit side-by-side at this width. */
export function computeMaxVisibleCols(viewportWidth: number): number {
  return Math.floor(viewportWidth / MIN_PANE_W)
}

export interface AutoLayoutEngine {
  maxVisibleCols: number
}

interface UseAutoLayoutOptions {
  api: DockviewApi | null
  containerRef: RefObject<HTMLElement | null>
  enabled: boolean
}

export function useAutoLayout({
  api: _api,
  containerRef,
  enabled,
}: UseAutoLayoutOptions): AutoLayoutEngine {
  const [maxVisibleCols, setMaxVisibleCols] = useState(0)

  useEffect(() => {
    if (!enabled || !containerRef.current) return

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0]
      if (entry) {
        setMaxVisibleCols(computeMaxVisibleCols(entry.contentRect.width))
      }
    })

    observer.observe(containerRef.current)
    return () => observer.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- api is read but not a reactive dep
  }, [enabled, containerRef])

  return { maxVisibleCols }
}
