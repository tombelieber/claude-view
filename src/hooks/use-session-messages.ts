import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

const PAGE_SIZE = 100

async function fetchMessages(
  sessionId: string,
  offset: number,
  limit: number,
): Promise<PaginatedMessages> {
  const response = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages?limit=${limit}&offset=${offset}`
  )
  if (!response.ok) throw new HttpError('Failed to fetch messages', response.status)
  return response.json()
}

export function useSessionMessages(sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', sessionId],
    queryFn: async ({ pageParam }) => {
      if (!sessionId) throw new Error('sessionId is required')

      if (pageParam === -1) {
        // Initial load: probe for total, then fetch the last PAGE_SIZE messages.
        const probe = await fetchMessages(sessionId, 0, 1)
        const tailOffset = Math.max(0, probe.total - PAGE_SIZE)
        return fetchMessages(sessionId, tailOffset, PAGE_SIZE)
      }

      return fetchMessages(sessionId, pageParam, PAGE_SIZE)
    },
    initialPageParam: -1 as number,
    getNextPageParam: () => undefined, // No downward pagination needed — already at the end
    getPreviousPageParam: (firstPage) => {
      // Load older messages (lower offsets) when scrolling up
      if (firstPage.offset === 0) return undefined // Already at the beginning
      const prevOffset = Math.max(0, firstPage.offset - PAGE_SIZE)
      return prevOffset
    },
    enabled: !!sessionId,
    retry: (_, error) => !isNotFoundError(error),
  })
}
