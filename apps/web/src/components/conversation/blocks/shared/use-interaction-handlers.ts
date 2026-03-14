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

export function useInteractionHandlers() {
  const ctx = useConversationActions()
  const [localResponse, setLocalResponse] = useState<LocalResponse | null>(null)

  const respondPermission = useCallback(
    (requestId: string, allowed: boolean) => {
      ctx?.respondPermission?.(requestId, allowed)
      setLocalResponse({ variant: 'permission', allowed })
    },
    [ctx],
  )

  const answerQuestion = useCallback(
    (requestId: string, answers: Record<string, string>) => {
      ctx?.answerQuestion?.(requestId, answers)
      setLocalResponse({ variant: 'question' })
    },
    [ctx],
  )

  const approvePlan = useCallback(
    (requestId: string, approved: boolean, feedback?: string) => {
      ctx?.approvePlan?.(requestId, approved, feedback)
      setLocalResponse({ variant: 'plan', approved })
    },
    [ctx],
  )

  const submitElicitation = useCallback(
    (requestId: string, response: string) => {
      ctx?.submitElicitation?.(requestId, response)
      setLocalResponse({ variant: 'elicitation' })
    },
    [ctx],
  )

  return {
    localResponse,
    // Only expose handlers when context provides them — otherwise undefined → read-only cards
    respondPermission: ctx?.respondPermission ? respondPermission : undefined,
    answerQuestion: ctx?.answerQuestion ? answerQuestion : undefined,
    approvePlan: ctx?.approvePlan ? approvePlan : undefined,
    submitElicitation: ctx?.submitElicitation ? submitElicitation : undefined,
  }
}
