import { useState, useEffect, useRef, useCallback } from 'react'
import { X, Loader2, HardDrive, CheckCircle2 } from 'lucide-react'
import type { IndexingProgress } from '../hooks/use-indexing-progress'

/**
 * Format a byte count into a human-readable string.
 */
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`
  return `${bytes} bytes`
}

/** Minimum time (ms) the banner stays visible so users can actually read it. */
const MIN_DISPLAY_MS = 2000
/** How long the "done" banner lingers before auto-dismissing. */
const DONE_LINGER_MS = 3000
/** Collapse/fade animation duration — matches the CSS transition. */
const ANIMATE_OUT_MS = 300

interface ColdStartOverlayProps {
  progress: IndexingProgress
}

/**
 * Non-blocking banner shown during cold start indexing.
 *
 * UX improvements over a naive show/hide:
 *  1. Shows a green "Done" state with summary before disappearing
 *  2. Animates out smoothly (no layout shift)
 *  3. Enforces a minimum display time so fast indexing doesn't just flash
 *  4. Respects prefers-reduced-motion
 */
export function ColdStartOverlay({ progress }: ColdStartOverlayProps) {
  const [dismissed, setDismissed] = useState(false)
  const [visible, setVisible] = useState(true) // controls the CSS transition
  const [gone, setGone] = useState(false) // true after animation completes — removes from DOM
  const firstShownAt = useRef<number | null>(null)
  const lingerTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Record when the banner first becomes meaningful
  useEffect(() => {
    if (
      firstShownAt.current === null &&
      progress.phase !== 'idle' &&
      progress.phase !== 'done'
    ) {
      firstShownAt.current = Date.now()
    }
  }, [progress.phase])

  // Animate-out helper: fade + collapse, then remove from DOM
  const animateOut = useCallback(() => {
    setVisible(false)
    setTimeout(() => setGone(true), ANIMATE_OUT_MS)
  }, [])

  // When indexing completes (or user dismisses), linger then animate out
  useEffect(() => {
    if (dismissed) {
      animateOut()
      return
    }

    if (progress.phase !== 'done') return

    // Ensure we've been visible for at least MIN_DISPLAY_MS
    const shownFor = firstShownAt.current
      ? Date.now() - firstShownAt.current
      : MIN_DISPLAY_MS
    const remainingMin = Math.max(0, MIN_DISPLAY_MS - shownFor)
    const delay = remainingMin + DONE_LINGER_MS

    lingerTimer.current = setTimeout(animateOut, delay)
    return () => {
      if (lingerTimer.current) clearTimeout(lingerTimer.current)
    }
  }, [progress.phase, dismissed, animateOut])

  // Nothing to show yet, or already fully gone
  if (progress.phase === 'idle' || gone) return null

  const percentage =
    progress.bytesTotal > 0
      ? Math.min(100, Math.round((progress.bytesProcessed / progress.bytesTotal) * 100))
      : 0

  const canDismiss =
    progress.phase === 'ready' ||
    progress.phase === 'deep-indexing' ||
    progress.phase === 'done'

  const isDone = progress.phase === 'done'

  return (
    <div
      className={[
        // Smooth collapse + fade
        'overflow-hidden transition-all ease-out',
        'motion-reduce:transition-none',
        visible
          ? 'max-h-40 opacity-100'
          : 'max-h-0 opacity-0',
      ]
        .join(' ')}
      style={{ transitionDuration: `${ANIMATE_OUT_MS}ms` }}
    >
      <div
        className={[
          'border-b px-4 py-2.5',
          isDone
            ? 'bg-green-50 dark:bg-green-950/40 border-green-200 dark:border-green-800'
            : 'bg-blue-50 dark:bg-blue-950/40 border-blue-200 dark:border-blue-800',
        ].join(' ')}
        role="status"
        aria-live="polite"
        aria-label="Indexing progress"
      >
        <div className="max-w-5xl mx-auto flex items-center gap-3">
          {/* Icon */}
          <div className="flex-shrink-0">
            {progress.phase === 'reading-indexes' && (
              <Loader2
                className="w-4 h-4 text-blue-500 dark:text-blue-400 animate-spin motion-reduce:animate-none"
                aria-hidden="true"
              />
            )}
            {(progress.phase === 'ready' || progress.phase === 'deep-indexing') && (
              <HardDrive
                className="w-4 h-4 text-blue-500 dark:text-blue-400"
                aria-hidden="true"
              />
            )}
            {isDone && (
              <CheckCircle2
                className="w-4 h-4 text-green-500 dark:text-green-400"
                aria-hidden="true"
              />
            )}
            {progress.phase === 'error' && (
              <X className="w-4 h-4 text-red-500 dark:text-red-400" aria-hidden="true" />
            )}
          </div>

          {/* Content */}
          <div className="flex-1 min-w-0">
            {progress.phase === 'reading-indexes' && (
              <p className="text-sm text-blue-700 dark:text-blue-300">
                Scanning your Claude Code history...
              </p>
            )}

            {progress.phase === 'ready' && (
              <p className="text-sm text-blue-700 dark:text-blue-300">
                Found {progress.projects.toLocaleString()} projects and{' '}
                {progress.sessions.toLocaleString()} sessions. Starting deep index...
              </p>
            )}

            {progress.phase === 'deep-indexing' && (
              <div className="space-y-1.5">
                {/* Progress bar */}
                <div className="flex items-center gap-3">
                  <div className="flex-1 h-2 bg-blue-100 dark:bg-blue-900/50 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-blue-500 dark:bg-blue-400 rounded-full transition-all duration-300 ease-out motion-reduce:transition-none"
                      style={{ width: `${percentage}%` }}
                    />
                  </div>
                  <span className="flex-shrink-0 text-xs font-medium text-blue-600 dark:text-blue-300 tabular-nums w-9 text-right">
                    {percentage}%
                  </span>
                </div>

                {/* Stats line */}
                <div className="flex items-center gap-1.5 text-xs text-blue-600 dark:text-blue-400">
                  <span className="tabular-nums">
                    {formatBytes(progress.bytesProcessed)} / {formatBytes(progress.bytesTotal)}
                  </span>
                  {progress.throughputBytesPerSec > 0 && (
                    <>
                      <span aria-hidden="true">&middot;</span>
                      <span className="tabular-nums">
                        {formatBytes(progress.throughputBytesPerSec)}/s
                      </span>
                    </>
                  )}
                  <span aria-hidden="true">&middot;</span>
                  <span className="tabular-nums">
                    {progress.indexed.toLocaleString()} / {progress.total.toLocaleString()} sessions
                  </span>
                </div>

                {/* Helper text */}
                <p className="text-xs text-blue-500 dark:text-blue-500">
                  You can browse sessions while we finish. Analytics will appear as indexing completes.
                </p>
              </div>
            )}

            {isDone && (
              <p className="text-sm text-green-700 dark:text-green-300">
                Ready &mdash; {progress.total.toLocaleString()} sessions indexed
                {progress.bytesTotal > 0 && ` (${formatBytes(progress.bytesTotal)})`}
              </p>
            )}

            {progress.phase === 'error' && (
              <p className="text-sm text-red-600 dark:text-red-400">
                Indexing error: {progress.errorMessage ?? 'Unknown error'}
              </p>
            )}
          </div>

          {/* Dismiss button */}
          {canDismiss && (
            <button
              type="button"
              onClick={() => setDismissed(true)}
              className={[
                'flex-shrink-0 p-1 rounded transition-colors duration-150',
                'focus-visible:outline-none focus-visible:ring-2 cursor-pointer',
                isDone
                  ? 'text-green-400 hover:text-green-600 dark:text-green-500 dark:hover:text-green-300 focus-visible:ring-green-400'
                  : 'text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300 focus-visible:ring-blue-400',
              ].join(' ')}
              aria-label="Dismiss indexing banner"
            >
              <X className="w-4 h-4" aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
