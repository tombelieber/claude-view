import { useState, useCallback, useEffect, useRef } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type {
  ClassifyProgressInfo,
  ClassifyLastRun,
  ClassifyStatusResponse,
  ClassifyResponse,
  CancelResponse,
} from '../types/generated'

// Re-export generated types for consumers
export type { ClassifyProgressInfo, ClassifyLastRun, ClassifyStatusResponse, ClassifyResponse, CancelResponse }

// Alias for backward compat — components reference ClassifyProgress
export type ClassifyProgress = ClassifyProgressInfo

export type ClassifyMode = 'unclassified' | 'all'

export interface UseClassificationResult {
  // Status
  status: ClassifyStatusResponse | null
  isLoading: boolean
  error: string | null

  // Actions
  startClassification: (mode: ClassifyMode) => Promise<ClassifyResponse | null>
  cancelClassification: () => Promise<boolean>
  dryRun: (mode: ClassifyMode) => Promise<ClassifyResponse | null>

  // SSE
  isStreaming: boolean
  sseProgress: ClassifyProgressInfo | null

  // Refresh
  refreshStatus: () => void
}

/**
 * Hook for managing session classification.
 *
 * Connects to:
 * - GET /api/classify/status (polling)
 * - POST /api/classify (trigger)
 * - POST /api/classify/cancel (cancel)
 * - GET /api/classify/stream (SSE for real-time progress)
 */
export function useClassification(): UseClassificationResult {
  const queryClient = useQueryClient()
  const [status, setStatus] = useState<ClassifyStatusResponse | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [isStreaming, setIsStreaming] = useState(false)
  const [sseProgress, setSseProgress] = useState<ClassifyProgressInfo | null>(null)
  const eventSourceRef = useRef<EventSource | null>(null)

  // Fetch status on mount and periodically
  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch('/api/classify/status')
      if (!res.ok) {
        throw new Error(`Status fetch failed: ${res.status}`)
      }
      const data: ClassifyStatusResponse = await res.json()
      setStatus(data)

      // Auto-connect SSE when classification is running
      if (data.status === 'running' && !eventSourceRef.current) {
        connectStream()
      }

      return data
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to fetch status'
      setError(msg)
      return null
    }
  }, [])

  // Initial fetch
  useEffect(() => {
    fetchStatus()
  }, [fetchStatus])

  // Polling interval when running
  useEffect(() => {
    if (status?.status !== 'running') return

    const interval = setInterval(() => {
      fetchStatus()
    }, 3000)

    return () => clearInterval(interval)
  }, [status?.status, fetchStatus])

  // Connect to SSE stream
  const connectStream = useCallback(() => {
    if (eventSourceRef.current) return

    const es = new EventSource('/api/classify/stream')
    eventSourceRef.current = es
    setIsStreaming(true)

    es.addEventListener('progress', (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data) as ClassifyProgressInfo
        setSseProgress(data)
      } catch {
        // ignore parse errors
      }
    })

    es.addEventListener('complete', (_event: MessageEvent) => {
      setSseProgress(null)
      setIsStreaming(false)
      es.close()
      eventSourceRef.current = null
      fetchStatus()
      queryClient.invalidateQueries({ queryKey: ['sessions'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    })

    es.addEventListener('cancelled', (_event: MessageEvent) => {
      setSseProgress(null)
      setIsStreaming(false)
      es.close()
      eventSourceRef.current = null
      fetchStatus()
    })

    es.addEventListener('error', (event: MessageEvent) => {
      // SSE errors can be connection drops, not necessarily job errors
      if (event.data) {
        try {
          const data = JSON.parse(event.data)
          setError(data.message || 'Classification error')
        } catch {
          // ignore
        }
      }
      setSseProgress(null)
      setIsStreaming(false)
      es.close()
      eventSourceRef.current = null
      fetchStatus()
    })

    es.addEventListener('idle', () => {
      setSseProgress(null)
      setIsStreaming(false)
      es.close()
      eventSourceRef.current = null
    })

    es.onerror = () => {
      // EventSource reconnects automatically, but if it fails repeatedly
      // we'll clean up
      setIsStreaming(false)
      es.close()
      eventSourceRef.current = null
    }
  }, [fetchStatus, queryClient])

  // Cleanup SSE on unmount
  useEffect(() => {
    return () => {
      if (eventSourceRef.current) {
        eventSourceRef.current.close()
        eventSourceRef.current = null
      }
    }
  }, [])

  // Start classification
  const startClassification = useCallback(async (mode: ClassifyMode): Promise<ClassifyResponse | null> => {
    setIsLoading(true)
    setError(null)

    try {
      const res = await fetch('/api/classify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode }),
      })

      if (res.status === 202 || res.status === 200) {
        const data: ClassifyResponse = await res.json()
        await fetchStatus()
        connectStream()
        return data
      } else if (res.status === 409) {
        setError('A classification job is already running')
        return null
      } else {
        const errorData = await res.json().catch(() => ({ error: 'Unknown error' }))
        const message = errorData.details || errorData.error || `Request failed: ${res.status}`
        setError(message)
        return null
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to start classification'
      setError(msg)
      return null
    } finally {
      setIsLoading(false)
    }
  }, [fetchStatus, connectStream])

  // Cancel classification
  const cancelClassification = useCallback(async (): Promise<boolean> => {
    setError(null)

    try {
      const res = await fetch('/api/classify/cancel', { method: 'POST' })

      if (res.ok) {
        await fetchStatus()
        return true
      } else {
        const errorData = await res.json().catch(() => ({ error: 'Unknown error' }))
        setError(errorData.details || errorData.error || 'Failed to cancel')
        return false
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to cancel classification'
      setError(msg)
      return false
    }
  }, [fetchStatus])

  // Dry run
  const dryRun = useCallback(async (mode: ClassifyMode): Promise<ClassifyResponse | null> => {
    try {
      const res = await fetch('/api/classify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode, dryRun: true }),
      })

      if (res.ok) {
        return await res.json()
      }
      return null
    } catch {
      return null
    }
  }, [])

  return {
    status,
    isLoading,
    error,
    startClassification,
    cancelClassification,
    dryRun,
    isStreaming,
    sseProgress,
    refreshStatus: fetchStatus,
  }
}
