import { useQuery } from '@tanstack/react-query'

export interface PendingInteractionMeta {
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string
  preview: string
}

export interface InteractionBlock {
  id: string
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string | null
  resolved: boolean
  historicalSource: string | null
  data: unknown
}

/**
 * Fetches the full interaction payload for a session's pending interaction.
 * Returns `null` when there is no pending interaction or data hasn't loaded yet.
 *
 * The query is only enabled when `pendingMeta` is truthy. Once fetched the
 * data is treated as immutable (staleTime: Infinity) since an interaction's
 * payload never changes while it is pending.
 */
export function useFullInteraction(
  sessionId: string,
  pendingMeta: PendingInteractionMeta | null | undefined,
): InteractionBlock | null {
  const { data } = useQuery({
    queryKey: ['interaction', sessionId, pendingMeta?.requestId],
    queryFn: () =>
      fetch(`/api/sessions/${sessionId}/interaction`).then((r) => {
        if (!r.ok) throw new Error(`${r.status}`)
        return r.json() as Promise<InteractionBlock>
      }),
    enabled: !!pendingMeta,
    staleTime: Number.POSITIVE_INFINITY, // interaction data is immutable while pending
    gcTime: 30_000, // garbage collect 30s after unmount
  })
  return data ?? null
}
