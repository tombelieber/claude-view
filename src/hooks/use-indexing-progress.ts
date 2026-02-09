import { useState, useEffect, useCallback, useRef } from 'react'

export type IndexingPhase =
  | 'idle'
  | 'reading-indexes'
  | 'deep-indexing'
  | 'done'
  | 'error'

export interface IndexingProgress {
  /** Current phase of the indexing process */
  phase: IndexingPhase
  /** Number of sessions indexed so far */
  indexed: number
  /** Total sessions to index */
  total: number
  /** Error message if phase is 'error' */
  errorMessage?: string
}

const POLL_INTERVAL_MS = 250

/**
 * Hook that polls `GET /api/indexing/status` for real-time indexing progress.
 *
 * Uses simple HTTP polling instead of SSE because Vite's dev proxy
 * buffers streaming responses, causing SSE events to arrive in a burst
 * only after the stream closes — defeating the purpose of real-time feedback.
 *
 * Only polls when `enabled` is true (i.e., after the user clicks Rebuild).
 * Automatically stops on completion, error, or unmount.
 */
export function useIndexingProgress(enabled: boolean): IndexingProgress {
  const [progress, setProgress] = useState<IndexingProgress>({
    phase: 'idle',
    indexed: 0,
    total: 0,
  })
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const stopPolling = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  useEffect(() => {
    if (!enabled) {
      stopPolling()
      return
    }

    // Reset progress when starting a new poll cycle
    setProgress({ phase: 'idle', indexed: 0, total: 0 })

    let cancelled = false

    const poll = async () => {
      try {
        const res = await fetch('/api/indexing/status')
        if (cancelled) return
        if (!res.ok) return

        const data = await res.json()
        if (cancelled) return

        const phase = data.phase as IndexingPhase

        setProgress({
          phase,
          indexed: data.indexed ?? 0,
          total: data.total ?? 0,
          errorMessage: data.errorMessage,
        })

        // Stop polling on terminal states
        if (phase === 'done' || phase === 'error') {
          stopPolling()
        }
      } catch {
        // Network error — keep polling, don't set error state
        // (transient failures are expected during server restart)
      }
    }

    // Poll immediately, then on interval
    poll()
    timerRef.current = setInterval(poll, POLL_INTERVAL_MS)

    return () => {
      cancelled = true
      stopPolling()
    }
  }, [enabled, stopPolling])

  return progress
}
