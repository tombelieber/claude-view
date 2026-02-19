import { useQuery } from '@tanstack/react-query'
import type { RichSessionData } from '../types/generated/RichSessionData'

async function fetchRichSessionData(sessionId: string): Promise<RichSessionData> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/rich`)
  if (!response.ok) {
    throw new Error(`Failed to fetch rich session data: ${response.status}`)
  }
  return response.json()
}

export function useRichSessionData(sessionId: string | null) {
  return useQuery({
    queryKey: ['session-rich', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchRichSessionData(sessionId)
    },
    enabled: !!sessionId,
    staleTime: 60_000, // JSONL doesn't change for completed sessions
  })
}
