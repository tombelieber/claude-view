import { useState, useCallback } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { SyncAcceptedResponse, ErrorResponse } from '../types/generated'

export type SyncStatus = 'idle' | 'running' | 'success' | 'conflict' | 'error'

interface UseGitSyncResult {
  /** Trigger git sync. Returns true if started, false if already running */
  triggerSync: () => Promise<boolean>
  /** Current sync status */
  status: SyncStatus
  /** Whether a sync request is in flight */
  isLoading: boolean
  /** Error message if sync failed (not including 409 conflict) */
  error: string | null
  /** Response from successful sync initiation */
  response: SyncAcceptedResponse | null
  /** Reset to idle state */
  reset: () => void
}

/**
 * Hook for triggering git sync operations.
 *
 * POST /api/sync/git returns:
 * - 202 Accepted: Sync started successfully
 * - 409 Conflict: Sync already in progress
 *
 * After successful sync trigger, poll /api/status to check completion.
 */
export function useGitSync(): UseGitSyncResult {
  const queryClient = useQueryClient()
  const [status, setStatus] = useState<SyncStatus>('idle')
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [response, setResponse] = useState<SyncAcceptedResponse | null>(null)

  const reset = useCallback(() => {
    setStatus('idle')
    setError(null)
    setResponse(null)
  }, [])

  const triggerSync = useCallback(async (): Promise<boolean> => {
    setIsLoading(true)
    setError(null)
    setResponse(null)

    try {
      const res = await fetch('/api/sync/git', {
        method: 'POST',
      })

      if (res.status === 202) {
        // Sync started successfully
        const data: SyncAcceptedResponse = await res.json()
        setResponse(data)
        setStatus('success')

        // Invalidate status query to show updated sync info
        queryClient.invalidateQueries({ queryKey: ['status'] })

        return true
      } else if (res.status === 409) {
        // Sync already in progress
        setStatus('conflict')
        return false
      } else {
        // Other error
        const errorData: ErrorResponse = await res.json().catch(() => ({
          error: 'Unknown error',
          details: null,
        }))
        const message = errorData.details || errorData.error || `Request failed with status ${res.status}`
        setError(message)
        setStatus('error')
        return false
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to trigger sync'
      setError(message)
      setStatus('error')
      return false
    } finally {
      setIsLoading(false)
    }
  }, [queryClient])

  return {
    triggerSync,
    status,
    isLoading,
    error,
    response,
    reset,
  }
}

// Re-export types for convenience
export type { SyncAcceptedResponse } from '../types/generated'
