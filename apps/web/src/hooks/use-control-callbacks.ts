import { useMemo } from 'react'
import type { ControlCallbacks } from '../types/control-callbacks'

export function useControlCallbacks(
  sendRaw: ((msg: Record<string, unknown>) => void) | undefined,
  respondPermission: ((requestId: string, allowed: boolean) => void) | undefined,
): ControlCallbacks | undefined {
  return useMemo(() => {
    if (!sendRaw) return undefined
    return {
      answerQuestion: (requestId, answers) =>
        sendRaw({ type: 'question_response', requestId, answers }),
      respondPermission: (requestId, allowed) => respondPermission?.(requestId, allowed),
      approvePlan: (requestId, approved, feedback) =>
        sendRaw({ type: 'plan_response', requestId, approved, feedback }),
      submitElicitation: (requestId, response) =>
        sendRaw({ type: 'elicitation_response', requestId, response }),
    }
  }, [sendRaw, respondPermission])
}
