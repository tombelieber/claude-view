import { useEffect, useRef, useState } from 'react'
import { sseUrl } from '../lib/sse-url'
import type { ProcessTreeSnapshot } from '../types/generated/ProcessTreeSnapshot'
import type { ResourceSnapshot } from '../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../types/generated/SystemInfo'

export type MonitorStatus = 'connecting' | 'connected' | 'reconnecting' | 'error'

export interface SystemMonitorState {
  status: MonitorStatus
  systemInfo: SystemInfo | null
  snapshot: ResourceSnapshot | null
  /** Classified process tree. null until first `process_tree` SSE event (~10s after connect). */
  processTree: ProcessTreeSnapshot | null
  /** Unix timestamp (ms) when processTree was last updated. For freshness indicator. */
  processTreeFreshAt: number | null
}

const INITIAL_STATE: SystemMonitorState = {
  status: 'connecting',
  systemInfo: null,
  snapshot: null,
  processTree: null,
  processTreeFreshAt: null,
}

const MAX_BACKOFF_MS = 30_000
const BASE_BACKOFF_MS = 1_000

export function useSystemMonitor(): SystemMonitorState {
  const [state, setState] = useState<SystemMonitorState>(INITIAL_STATE)
  const retriesRef = useRef(0)
  const esRef = useRef<EventSource | null>(null)

  useEffect(() => {
    let unmounted = false

    function connect() {
      if (unmounted) return

      const es = new EventSource(sseUrl('/api/monitor/stream'))
      esRef.current = es

      es.addEventListener('init', (e: MessageEvent) => {
        if (unmounted || esRef.current !== es) return
        try {
          const data: { systemInfo: SystemInfo; snapshot: ResourceSnapshot } = JSON.parse(e.data)
          retriesRef.current = 0
          setState((prev) => ({
            ...prev,
            status: 'connected',
            systemInfo: data.systemInfo,
            snapshot: data.snapshot,
          }))
        } catch {
          // ignore malformed data
        }
      })

      es.addEventListener('snapshot', (e: MessageEvent) => {
        if (unmounted || esRef.current !== es) return
        try {
          const snap: ResourceSnapshot = JSON.parse(e.data)
          setState((prev) => ({
            ...prev,
            status: 'connected',
            snapshot: snap,
          }))
        } catch {
          // ignore malformed data
        }
      })

      es.addEventListener('process_tree', (e: MessageEvent) => {
        if (unmounted || esRef.current !== es) return
        try {
          const tree: ProcessTreeSnapshot = JSON.parse(e.data)
          setState((prev) => ({
            ...prev,
            processTree: tree,
            processTreeFreshAt: Date.now(),
          }))
        } catch {
          // ignore malformed data
        }
      })

      es.addEventListener('error', () => {
        if (unmounted || esRef.current !== es) return
        es.close()
        esRef.current = null

        const retries = retriesRef.current
        retriesRef.current = retries + 1
        const backoff = Math.min(BASE_BACKOFF_MS * 2 ** retries, MAX_BACKOFF_MS)

        setState((prev) => ({
          ...prev,
          status: 'reconnecting',
        }))

        setTimeout(connect, backoff)
      })
    }

    connect()

    return () => {
      unmounted = true
      if (esRef.current) {
        esRef.current.close()
        esRef.current = null
      }
    }
  }, [])

  return state
}
