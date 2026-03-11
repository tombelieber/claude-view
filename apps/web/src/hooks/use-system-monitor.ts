import { useEffect, useRef, useState } from 'react'
import { sseUrl } from '../lib/sse-url'
import type { ResourceSnapshot } from '../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../types/generated/SystemInfo'

export type MonitorStatus = 'connecting' | 'connected' | 'reconnecting' | 'error'

export interface SystemMonitorState {
  status: MonitorStatus
  systemInfo: SystemInfo | null
  snapshot: ResourceSnapshot | null
}

const INITIAL_STATE: SystemMonitorState = {
  status: 'connecting',
  systemInfo: null,
  snapshot: null,
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

      es.addEventListener('monitor_connected', (e: MessageEvent) => {
        if (unmounted || esRef.current !== es) return
        try {
          const info: SystemInfo = JSON.parse(e.data)
          retriesRef.current = 0
          setState((prev) => ({
            ...prev,
            status: 'connected',
            systemInfo: info,
          }))
        } catch {
          // ignore malformed data
        }
      })

      es.addEventListener('monitor_snapshot', (e: MessageEvent) => {
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
