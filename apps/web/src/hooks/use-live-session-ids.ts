/**
 * Lightweight hook that polls GET /api/live/sessions to build:
 * - a Set<string> of currently-live session IDs
 * - a Map<sessionId, totalCostUsd> for live cost override on active history cards
 *
 * Polls every 10 seconds — much cheaper than opening a second SSE connection.
 */
import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'

interface LiveSubAgentSnapshot {
  costUsd?: number | null
}

interface LiveCostSnapshot {
  totalUsd?: number
}

interface LiveSessionSnapshot {
  id: string
  cost?: LiveCostSnapshot | null
  subAgents?: LiveSubAgentSnapshot[] | null
}

interface LiveSessionsResponse {
  sessions: LiveSessionSnapshot[]
  total: number
  processCount: number
}

export interface LiveSessionPresence {
  ids: Set<string>
  costById: Map<string, number>
}

export function useLiveSessionPresence(): LiveSessionPresence {
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
    if (!sessions || sessions.length === 0) {
      return { ids: new Set<string>(), costById: new Map<string, number>() }
    }

    const ids = new Set<string>()
    const costById = new Map<string, number>()
    for (const s of sessions) {
      ids.add(s.id)
      const subAgentTotal = s.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
      const totalCost = (s.cost?.totalUsd ?? 0) + subAgentTotal
      if (totalCost > 0) {
        costById.set(s.id, totalCost)
      }
    }

    return { ids, costById }
  }, [data])
}

export function useLiveSessionIds(): Set<string> {
  return useLiveSessionPresence().ids
}
