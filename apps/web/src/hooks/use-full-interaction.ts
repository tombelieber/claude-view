import { useQuery } from '@tanstack/react-query'
import type { InteractionBlock } from '../types/generated/InteractionBlock'
import type { PendingInteractionMeta } from '../types/generated/PendingInteractionMeta'

// Re-export the ts-rs-generated contract so existing consumers/tests that
// imported these types from this module keep working. The shapes are the
// single source of truth in `../types/generated/` — never re-declare them.
export type { InteractionBlock, PendingInteractionMeta }

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
