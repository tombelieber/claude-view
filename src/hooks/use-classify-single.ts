import { useState, useCallback } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { ClassifySingleResponse } from '../types/generated/ClassifySingleResponse'

export interface UseClassifySingleResult {
  /** ID of the session currently being classified, or null */
  classifyingId: string | null
  /** Classify a single session. Returns the result or null on error. */
  classifySession: (sessionId: string) => Promise<ClassifySingleResponse | null>
  /** Last error message, or null */
  error: string | null
}

/**
 * Hook for classifying a single session via POST /api/classify/single/:id.
 *
 * Lightweight — no SSE, no job tracking. Just request→response.
 * Optimistically updates the React Query cache so the badge appears instantly.
 */
export function useClassifySingle(): UseClassifySingleResult {
  const queryClient = useQueryClient()
  const [classifyingId, setClassifyingId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const classifySession = useCallback(
    async (sessionId: string): Promise<ClassifySingleResponse | null> => {
      setClassifyingId(sessionId)
      setError(null)

      try {
        const url = `/api/classify/single/${encodeURIComponent(sessionId)}`
        console.log('[useClassifySingle] fetching', url)

        const res = await fetch(url, { method: 'POST' })
        console.log('[useClassifySingle] response', res.status, res.statusText)

        if (!res.ok) {
          const errData = await res.json().catch(() => ({ error: 'Unknown error' }))
          const msg = errData.details || errData.error || `Failed: ${res.status}`
          console.error('[useClassifySingle] error response', msg)
          setError(msg)
          return null
        }

        const data: ClassifySingleResponse = await res.json()
        console.log('[useClassifySingle] success', data)

        // Optimistically update session in React Query cache.
        // The server already persisted the result, so this IS the truth —
        // no need for invalidateQueries (which would cause a redundant refetch + flicker).
        queryClient.setQueriesData<{ sessions: Array<Record<string, unknown>> }>(
          { queryKey: ['project-sessions'] },
          (old) => {
            if (!old?.sessions) return old
            return {
              ...old,
              sessions: old.sessions.map((s) =>
                s.id === sessionId
                  ? {
                      ...s,
                      categoryL1: data.categoryL1,
                      categoryL2: data.categoryL2,
                      categoryL3: data.categoryL3,
                      categoryConfidence: data.confidence,
                      categorySource: 'claude-cli',
                      classifiedAt: new Date().toISOString(),
                    }
                  : s,
              ),
            }
          },
        )

        // Track classify count and notify banner via CustomEvent (instant, same-tab)
        const countKey = 'classify-single-count'
        const prev = parseInt(localStorage.getItem(countKey) || '0', 10)
        const newCount = prev + 1
        localStorage.setItem(countKey, String(newCount))
        window.dispatchEvent(new CustomEvent('classify-single-done', { detail: newCount }))

        return data
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Classification failed'
        setError(msg)
        return null
      } finally {
        setClassifyingId(null)
      }
    },
    [queryClient],
  )

  return { classifyingId, classifySession, error }
}
