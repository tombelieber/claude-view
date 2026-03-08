import { useQuery } from '@tanstack/react-query'
import type { PlanDocument } from '../types/generated/PlanDocument'

async function fetchPlanDocuments(sessionId: string): Promise<PlanDocument[]> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/plans`)
  if (!response.ok) {
    throw new Error(`Failed to fetch plan documents: ${response.status}`)
  }
  return response.json()
}

export function usePlanDocuments(sessionId: string | null, enabled = true) {
  return useQuery({
    queryKey: ['plan-documents', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchPlanDocuments(sessionId)
    },
    enabled: !!sessionId && enabled,
  })
}
