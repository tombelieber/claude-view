import { useState, useEffect } from 'react'

export type IndexingPhase =
  | 'idle'
  | 'reading-indexes'
  | 'deep-indexing'
  | 'done'
  | 'error'

export interface IndexingProgress {
  phase: IndexingPhase
  indexed: number
  total: number
  errorMessage?: string
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
 * Only connects when `enabled` is true (after the user clicks Rebuild).
 * Automatically closes on completion, error, or unmount.
 */
export function useIndexingProgress(enabled: boolean): IndexingProgress {
  const [progress, setProgress] = useState<IndexingProgress>({
    phase: 'idle',
    indexed: 0,
    total: 0,
  })

  useEffect(() => {
    if (!enabled) return

    setProgress({ phase: 'idle', indexed: 0, total: 0 })

    const es = new EventSource(sseUrl())

    es.addEventListener('status', (e: MessageEvent) => {
      const data = JSON.parse(e.data)
      if (data.status === 'reading-indexes') {
        setProgress((prev) => ({ ...prev, phase: 'reading-indexes' }))
      }
    })

    es.addEventListener('deep-progress', (e: MessageEvent) => {
      const data = JSON.parse(e.data)
      setProgress({
        phase: 'deep-indexing',
        indexed: data.indexed ?? 0,
        total: data.total ?? 0,
      })
    })

    es.addEventListener('done', (e: MessageEvent) => {
      const data = JSON.parse(e.data)
      setProgress({
        phase: 'done',
        indexed: data.indexed ?? 0,
        total: data.total ?? 0,
      })
      es.close()
    })

    // Server-sent error events (event: error\ndata: {...}) arrive as MessageEvents
    // with data. Browser connection errors arrive as plain Events without data.
    es.addEventListener('error', (e: Event) => {
      if ('data' in e && (e as MessageEvent).data) {
        const data = JSON.parse((e as MessageEvent).data)
        setProgress({
          phase: 'error',
          indexed: 0,
          total: 0,
          errorMessage: data.message ?? 'Unknown error',
        })
      } else if (es.readyState === EventSource.CLOSED) {
        setProgress({
          phase: 'error',
          indexed: 0,
          total: 0,
          errorMessage: 'Lost connection to server',
        })
      }
      es.close()
    })

    return () => es.close()
  }, [enabled])

  return progress
}
