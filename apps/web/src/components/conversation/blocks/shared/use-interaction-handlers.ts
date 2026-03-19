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

  const respondPermission = useCallback(
    (reqId: string, allowed: boolean) => {
      ctx?.respondPermission?.(reqId, allowed)
      const response: LocalResponse = { variant: 'permission', allowed }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
    },
    [ctx],
  )

  const answerQuestion = useCallback(
    (reqId: string, answers: Record<string, string>) => {
      ctx?.answerQuestion?.(reqId, answers)
      const response: LocalResponse = { variant: 'question' }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
    },
    [ctx],
  )

  const approvePlan = useCallback(
    (reqId: string, approved: boolean, feedback?: string) => {
      ctx?.approvePlan?.(reqId, approved, feedback)
      const response: LocalResponse = { variant: 'plan', approved }
      setLocalResponse(response)
      respondedCache.set(reqId, response)
    },
    [ctx],
  )

  const submitElicitation = useCallback(
    (reqId: string, response: string) => {
      ctx?.submitElicitation?.(reqId, response)
      const resp: LocalResponse = { variant: 'elicitation' }
      setLocalResponse(resp)
      respondedCache.set(reqId, resp)
    },
    [ctx],
  )

  return {
    localResponse,
    respondPermission: ctx?.respondPermission ? respondPermission : undefined,
    answerQuestion: ctx?.answerQuestion ? answerQuestion : undefined,
    approvePlan: ctx?.approvePlan ? approvePlan : undefined,
    submitElicitation: ctx?.submitElicitation ? submitElicitation : undefined,
  }
}
