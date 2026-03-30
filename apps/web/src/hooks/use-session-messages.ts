import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'
import { HttpError, isNotFoundError } from './use-session'

export const PAGE_SIZE = 50

/** Compute the initial page size: load all if small, cap to 60% for large sessions.
 *  Exported for testing. */
export function computeInitialPage(total: number): { offset: number; size: number } {
  const maxInitial = Math.floor(total * 0.6)
  const size = total <= PAGE_SIZE ? total : Math.min(PAGE_SIZE, maxInitial)
  const offset = Math.max(0, total - size)
  return { offset, size }
}

/** Compute the previous page params with clamped limit to prevent overlap.
 *  Returns undefined when already at the beginning. Exported for testing. */
export function computePreviousPage(
  currentOffset: number,
): { offset: number; limit: number } | undefined {
  if (currentOffset === 0) return undefined
  const prevOffset = Math.max(0, currentOffset - PAGE_SIZE)
  const limit = currentOffset - prevOffset
  return { offset: prevOffset, limit }
}

export interface PaginatedBlocks {
  blocks: ConversationBlock[]
  total: number
  offset: number
  limit: number
  hasMore: boolean
}

/** Exported for testing — fetches paginated messages from the server.
 *  @param suppressNotFound When true, 404 returns empty data instead of throwing.
 *    Used for brand-new sessions whose JSONL hasn't been created yet.
 *  @param format When 'block', requests pre-built ConversationBlock[] from the server. */
export async function fetchMessages(
  sessionId: string,
  offset: number,
  limit: number,
  raw: boolean,
  suppressNotFound = false,
  format?: 'block',
): Promise<PaginatedMessages | PaginatedBlocks> {
  const params = new URLSearchParams({ limit: String(limit), offset: String(offset) })
  if (raw) params.set('raw', 'true')
  if (format) params.set('format', format)
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/messages?${params}`)
  if (response.status === 404 && suppressNotFound) {
    // Session JSONL not created yet (brand-new session still initializing).
    // Return empty data — the WS stream will deliver live events, and future
    // refetches will pick up the JSONL once the sidecar writes it.
    return format === 'block'
      ? { blocks: [], total: 0, offset: 0, limit, hasMore: false }
      : { messages: [], total: 0, offset: 0, limit, hasMore: false }
  }
  if (!response.ok) throw new HttpError('Failed to fetch messages', response.status)
  return response.json()
}

export interface UseSessionMessagesOptions {
  raw?: boolean
  format?: 'block'
  suppressNotFound?: boolean
  enabled?: boolean
  retry?: number | ((failureCount: number, error: Error) => boolean)
  retryDelay?: number
}

export function useSessionMessages(sessionId: string | null, options?: UseSessionMessagesOptions) {
  const raw = options?.raw ?? false
  const format = options?.format
  const suppressNotFound = options?.suppressNotFound ?? false
  return useInfiniteQuery({
    queryKey: ['session-messages', sessionId, { raw, format }],
    queryFn: async ({ pageParam }) => {
      if (!sessionId) throw new Error('sessionId is required')

      if (pageParam === -1) {
        // Initial load: probe for total, then fetch the tail.
        const probe = await fetchMessages(sessionId, 0, 1, raw, suppressNotFound, format)
        const total = 'total' in probe ? probe.total : 0
        const { offset: tailOffset, size: initialSize } = computeInitialPage(total)
        return fetchMessages(sessionId, tailOffset, initialSize, raw, suppressNotFound, format)
      }

      // Previous page: { offset, limit } with clamped limit to avoid overlap
      if (typeof pageParam === 'object' && pageParam !== null) {
        return fetchMessages(
          sessionId,
          pageParam.offset,
          pageParam.limit,
          raw,
          suppressNotFound,
          format,
        )
      }

      return fetchMessages(sessionId, pageParam, PAGE_SIZE, raw, suppressNotFound, format)
    },
    initialPageParam: -1 as number | { offset: number; limit: number },
    getNextPageParam: () => undefined, // No downward pagination needed — already at the end
    getPreviousPageParam: (firstPage) => computePreviousPage(firstPage.offset),
    enabled: options?.enabled ?? !!sessionId,
    staleTime: 30_000,
    retry: options?.retry ?? ((_, error) => !isNotFoundError(error)),
    ...(options?.retryDelay !== undefined ? { retryDelay: options.retryDelay } : {}),
  })
}
