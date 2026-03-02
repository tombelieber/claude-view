/**
 * Lightweight hook that polls GET /api/live/sessions to build a Set<string>
 * of currently-live session IDs. Used by the history page to show a "LIVE"
 * badge on sessions that are still running.
 *
 * Polls every 10 seconds — much cheaper than opening a second SSE connection.
 */
import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'

interface LiveSessionsResponse {
  sessions: { id: string }[]
  total: number
  processCount: number
}

export function useLiveSessionIds(): Set<string> {
  const { data } = useQuery<LiveSessionsResponse>({
    queryKey: ['live-session-ids'],
    queryFn: async () => {
      const res = await fetch('/api/live/sessions')
      if (!res.ok) return { sessions: [], total: 0, processCount: 0 }
      return res.json()
    },
    refetchInterval: 10_000,
    staleTime: 5_000,
  })

  return useMemo(() => {
    const sessions = data?.sessions
    if (!sessions || sessions.length === 0) return new Set<string>()
    return new Set(sessions.map((s) => s.id))
  }, [data])
}
