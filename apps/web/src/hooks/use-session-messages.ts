import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

const PAGE_SIZE = 100

/** Exported for testing — fetches paginated messages from the server.
 *  @param suppressNotFound When true, 404 returns empty data instead of throwing.
 *    Used for brand-new sessions whose JSONL hasn't been created yet. */
export async function fetchMessages(
  sessionId: string,
  offset: number,
  limit: number,
  raw: boolean,
  suppressNotFound = false,
): Promise<PaginatedMessages> {
  const params = new URLSearchParams({ limit: String(limit), offset: String(offset) })
  if (raw) params.set('raw', 'true')
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/messages?${params}`)
  if (response.status === 404 && suppressNotFound) {
    // Session JSONL not created yet (brand-new session still initializing).
    // Return empty data — the WS stream will deliver live events, and future
    // refetches will pick up the JSONL once the sidecar writes it.
    return { messages: [], total: 0, offset: 0, limit, hasMore: false }
  }
  if (!response.ok) throw new HttpError('Failed to fetch messages', response.status)
  return response.json()
}

export function useSessionMessages(
  sessionId: string | null,
  options?: { raw?: boolean; suppressNotFound?: boolean },
) {
  const raw = options?.raw ?? false
  const suppressNotFound = options?.suppressNotFound ?? false
  return useInfiniteQuery({
    queryKey: ['session-messages', sessionId, { raw }],
    queryFn: async ({ pageParam }) => {
      if (!sessionId) throw new Error('sessionId is required')

      if (pageParam === -1) {
        // Initial load: probe for total, then fetch the last PAGE_SIZE messages.
        const probe = await fetchMessages(sessionId, 0, 1, raw, suppressNotFound)
        const tailOffset = Math.max(0, probe.total - PAGE_SIZE)
        return fetchMessages(sessionId, tailOffset, PAGE_SIZE, raw, suppressNotFound)
      }

      return fetchMessages(sessionId, pageParam, PAGE_SIZE, raw, suppressNotFound)
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
    staleTime: 30_000,
    retry: (_, error) => !isNotFoundError(error),
  })
}
