import { useState, useEffect, useCallback, useRef } from 'react'

export interface FacetIngestProgress {
  status: 'idle' | 'scanning' | 'ingesting' | 'complete' | 'error' | 'no_cache_found'
  total: number
  ingested: number
  newFacets: number
}

export interface UseFacetIngestResult {
  progress: FacetIngestProgress | null
  isRunning: boolean
  trigger: () => Promise<void>
}

/**
 * SSE endpoint URL.
 * In dev mode (Vite on :5173), bypass the proxy and hit the Rust server directly —
 * Vite's http-proxy buffers SSE, defeating real-time feedback.
 */
function sseUrl(): string {
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'http://localhost:47892/api/facets/ingest/stream'
  }
  return '/api/facets/ingest/stream'
}

function apiBase(): string {
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'http://localhost:47892'
  }
  return ''
}

/**
 * Hook that triggers facet ingestion and streams progress via SSE.
 *
 * Call `trigger()` to POST to the ingest endpoint, then automatically
 * connects to the SSE stream for real-time progress updates.
 * Automatically closes on completion, error, or unmount.
 */
export function useFacetIngest(): UseFacetIngestResult {
  const [progress, setProgress] = useState<FacetIngestProgress | null>(null)
  const esRef = useRef<EventSource | null>(null)

  const connectStream = useCallback(() => {
    esRef.current?.close()
    const es = new EventSource(sseUrl())
    esRef.current = es

    es.addEventListener('progress', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        es.close()
        return
      }
      setProgress(data)
    })

    es.addEventListener('done', (e: MessageEvent) => {
      let data
      try {
        data = JSON.parse(e.data)
      } catch {
        es.close()
        return
      }
      setProgress(data)
      es.close()
    })

    es.onerror = () => es.close()
  }, [])

  const trigger = useCallback(async () => {
    await fetch(`${apiBase()}/api/facets/ingest/trigger`, { method: 'POST' })
    connectStream()
  }, [connectStream])

  useEffect(() => {
    return () => esRef.current?.close()
  }, [])

  const isRunning = progress !== null &&
    !['complete', 'error', 'no_cache_found', 'idle'].includes(progress.status)

  return { progress, isRunning, trigger }
}
