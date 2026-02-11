import { useQuery } from '@tanstack/react-query'
import type { SessionDetail } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

async function fetchSessionDetail(sessionId: string): Promise<SessionDetail> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}`)
  if (!response.ok) {
    throw new HttpError('Failed to fetch session detail', response.status)
  }
  return response.json()
}

/**
 * Hook to fetch extended session detail including commits and derived metrics.
 *
 * Uses GET /api/sessions/:id which returns:
 * - All atomic unit fields (filesRead, filesEdited arrays)
 * - Linked commits with tier
 * - Derived metrics (tokensPerPrompt, reeditRate, etc.)
 */
export function useSessionDetail(sessionId: string | null) {
  return useQuery({
    queryKey: ['session-detail', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchSessionDetail(sessionId)
    },
    enabled: !!sessionId,
    retry: (failureCount, error) => {
      if (isNotFoundError(error)) return false
      return failureCount < 3
    },
  })
}
