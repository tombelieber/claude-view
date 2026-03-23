import { useCallback, useState } from 'react'
import { useConversationActions } from '../../../../contexts/conversation-actions-context'

/**
 * Local resolution state after the user responds to an interactive card.
 * Tracks what the user chose so the UI can immediately show resolved state
 * without waiting for a server round-trip.
 */
type LocalResponse =
  | { variant: 'permission'; allowed: boolean }
  | { variant: 'question' }
  | { variant: 'plan'; approved: boolean }
  | { variant: 'elicitation' }

/**
 * Module-level cache of responded requestIds.
 * Survives component unmount/remount (e.g. Chat ↔ Developer mode switch)
 * so the user's Allow/Deny choice persists across display mode changes.
 */
const respondedCache = new Map<string, LocalResponse>()

/** Clear the responded cache. Called on WS disconnect and in tests. */
export function clearRespondedCache(): void {
  respondedCache.clear()
}

/** @deprecated Use clearRespondedCache() instead */
export const _clearRespondedCacheForTesting = clearRespondedCache

export function useInteractionHandlers(requestId: string) {
  const ctx = useConversationActions()
  const [localResponse, setLocalResponse] = useState<LocalResponse | null>(
    () => respondedCache.get(requestId) ?? null,
  )
  const [isPending, setIsPending] = useState(false)

  const respondPermission = useCallback(
    (reqId: string, allowed: boolean) => {
      setIsPending(true)
      ctx?.respondPermission?.(reqId, allowed)
      const response: LocalResponse = { variant: 'permission', allowed }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
      // Clear pending once local state is committed (server ack comes via blocks_update)
      setIsPending(false)
    },
    [ctx],
  )

  const alwaysAllow = useCallback(
    (reqId: string, allowed: boolean, updatedPermissions: unknown[]) => {
      setIsPending(true)
      ctx?.respondPermission?.(reqId, allowed, updatedPermissions)
      const response: LocalResponse = { variant: 'permission', allowed }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
      setIsPending(false)
    },
    [ctx],
  )

  const answerQuestion = useCallback(
    (reqId: string, answers: Record<string, string>) => {
      setIsPending(true)
      ctx?.answerQuestion?.(reqId, answers)
      const response: LocalResponse = { variant: 'question' }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
      setIsPending(false)
    },
    [ctx],
  )

  const approvePlan = useCallback(
    (reqId: string, approved: boolean, feedback?: string) => {
      setIsPending(true)
      ctx?.approvePlan?.(reqId, approved, feedback)
      const response: LocalResponse = { variant: 'plan', approved }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
      setIsPending(false)
    },
    [ctx],
  )

  const submitElicitation = useCallback(
    (reqId: string, response: string) => {
      setIsPending(true)
      ctx?.submitElicitation?.(reqId, response)
      const resp: LocalResponse = { variant: 'elicitation' }
      setLocalResponse(resp)
      respondedCache.set(reqId, resp)
      setIsPending(false)
    },
    [ctx],
  )

  return {
    localResponse,
    isPending,
    respondPermission: ctx?.respondPermission ? respondPermission : undefined,
    alwaysAllow: ctx?.respondPermission ? alwaysAllow : undefined,
    answerQuestion: ctx?.answerQuestion ? answerQuestion : undefined,
    approvePlan: ctx?.approvePlan ? approvePlan : undefined,
    submitElicitation: ctx?.submitElicitation ? submitElicitation : undefined,
  }
}
