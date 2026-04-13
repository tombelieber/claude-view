/**
 * Selector that derives live session presence from the SSE-driven store.
 *
 * Returns a narrow projection — just IDs and cost — so consumers (HistoryView)
 * don't depend on the full LiveSession type. Same pattern as LiveMonitorPage:
 * SSE drives the store, selectors project what each consumer needs.
 *
 * Previously this polled GET /api/live/sessions every 10s. Eliminated in favour
 * of the single-writer event-driven architecture (SSE → store → selector).
 */
import { useMemo } from 'react'
import { useLiveSessionStore } from '../store/live-session-store'

export interface LiveSessionPresence {
  ids: Set<string>
  costById: Map<string, number>
}

export function useLiveSessionPresence(): LiveSessionPresence {
  const sessionsById = useLiveSessionStore((s) => s.sessionsById)
  return useMemo(() => {
    const ids = new Set<string>()
    const costById = new Map<string, number>()
    for (const [id, s] of sessionsById) {
      ids.add(id)
      const subAgentTotal = s.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
      const total = (s.cost?.totalUsd ?? 0) + subAgentTotal
      if (total > 0) costById.set(id, total)
    }
    return { ids, costById }
  }, [sessionsById])
}

export function useLiveSessionIds(): Set<string> {
  return useLiveSessionPresence().ids
}
