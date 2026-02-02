import { useInfiniteQuery } from '@tanstack/react-query'
import type { PaginatedMessages } from '../types/generated'

const PAGE_SIZE = 100

async function fetchMessages(
  projectDir: string,
  sessionId: string,
  offset: number,
  limit: number,
): Promise<PaginatedMessages> {
  const response = await fetch(
    `/api/session/${encodeURIComponent(projectDir)}/${encodeURIComponent(sessionId)}/messages?limit=${limit}&offset=${offset}`
  )
  if (!response.ok) throw new Error('Failed to fetch messages')
  return response.json()
}

export function useSessionMessages(projectDir: string | null, sessionId: string | null) {
  return useInfiniteQuery({
    queryKey: ['session-messages', projectDir, sessionId],
    queryFn: ({ pageParam = 0 }) => {
      if (!projectDir || !sessionId) throw new Error('projectDir and sessionId are required')
      return fetchMessages(projectDir, sessionId, pageParam, PAGE_SIZE)
    },
    initialPageParam: 0,
    getNextPageParam: (lastPage) => {
      if (!lastPage.hasMore) return undefined
      return lastPage.offset + lastPage.limit
    },
    enabled: !!projectDir && !!sessionId,
  })
}
