import { useState, useEffect, useRef } from 'react'

export type IndexingPhase =
  | 'idle'
  | 'reading-indexes'
  | 'ready'
  | 'deep-indexing'
  | 'done'
  | 'error'

export interface IndexingProgress {
  phase: IndexingPhase
  /** Pass 1 results */
  projects: number
  sessions: number
  /** Pass 2 progress */
  indexed: number
  total: number
  /** Bandwidth tracking */
  bytesProcessed: number
  bytesTotal: number
  throughputBytesPerSec: number
  /** True until first indexing completes */
  isFirstRun: boolean
  errorMessage?: string
}

const INITIAL_STATE: IndexingProgress = {
  phase: 'idle',
  projects: 0,
  sessions: 0,
  indexed: 0,
  total: 0,
  bytesProcessed: 0,
  bytesTotal: 0,
  throughputBytesPerSec: 0,
  isFirstRun: true,
}

/**
 * SSE endpoint URL.
 * In dev mode (Vite on :5173), bypass the proxy and hit the Rust server directly —
 * Vite's http-proxy buffers SSE, defeating real-time feedback.
 */
function sseUrl(): string {
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'http://localhost:47892/api/indexing/progress'
  }
  return '/api/indexing/progress'
}

/**
 * Hook that streams rebuild progress via SSE from `GET /api/indexing/progress`.
 *
 * When `enabled` is omitted or true, connects immediately.
 * When `enabled` is false, stays idle (for on-demand usage like StorageOverview).
 * Automatically closes on completion, error, or unmount.
 */
export function useIndexingProgress(enabled: boolean = true): IndexingProgress {
  const [progress, setProgress] = useState<IndexingProgress>(INITIAL_STATE)
  const startTimeRef = useRef<number | null>(null)

  useEffect(() => {
    if (!enabled) return

    setProgress(INITIAL_STATE)
    startTimeRef.current = null

    const es = new EventSource(sseUrl())

    es.addEventListener('status', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      if (data.status === 'reading-indexes') {
        setProgress((prev) => ({ ...prev, phase: 'reading-indexes' }))
      }
    })

    es.addEventListener('ready', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      startTimeRef.current = Date.now()
      setProgress((prev) => ({
        ...prev,
        phase: 'ready',
        projects: data.projects ?? prev.projects,
        sessions: data.sessions ?? prev.sessions,
      }))
    })

    es.addEventListener('deep-progress', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      const elapsed = startTimeRef.current
        ? (Date.now() - startTimeRef.current) / 1000
        : 1
      const bytesProcessed = data.bytes_processed ?? 0
      const throughput = elapsed > 0 ? bytesProcessed / elapsed : 0

      setProgress((prev) => ({
        ...prev,
        phase: 'deep-indexing',
        indexed: data.indexed ?? 0,
        total: data.total ?? 0,
        bytesProcessed,
        bytesTotal: data.bytes_total ?? 0,
        throughputBytesPerSec: throughput,
      }))
    })

    es.addEventListener('done', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Received malformed progress data from server',
        })
        es.close()
        return
      }
      setProgress((prev) => ({
        ...prev,
        phase: 'done',
        indexed: data.indexed ?? 0,
        total: data.total ?? 0,
        bytesProcessed: data.bytes_processed ?? prev.bytesTotal,
        bytesTotal: data.bytes_total ?? prev.bytesTotal,
        isFirstRun: false,
      }))
      es.close()
    })

    // Server-sent error events (event: error\ndata: {...}) arrive as MessageEvents
    // with data. Browser connection errors arrive as plain Events without data.
    es.addEventListener('error', (e: Event) => {
      if ('data' in e && (e as MessageEvent).data) {
        let data
        try {
          data = JSON.parse((e as MessageEvent).data)
        } catch {
          setProgress({
            ...INITIAL_STATE,
            phase: 'error',
            errorMessage: 'Received malformed progress data from server',
          })
          es.close()
          return
        }
        setProgress((prev) => ({
          ...prev,
          phase: 'error',
          errorMessage: data.message ?? 'Unknown error',
        }))
      } else if (es.readyState === EventSource.CLOSED) {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Lost connection to server',
        })
      } else {
        setProgress({
          ...INITIAL_STATE,
          phase: 'error',
          errorMessage: 'Connection to server failed',
        })
      }
      es.close()
    })

    return () => es.close()
  }, [enabled])

  return progress
}
