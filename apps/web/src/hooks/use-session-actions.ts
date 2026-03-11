import { useMemo } from 'react'

export interface SessionActions {
  sendMessage: (text: string) => void
  respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => void
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation: (requestId: string, response: string) => void
  setPermissionMode: (mode: string) => void
}

const NOOP_ACTIONS: SessionActions = {
  sendMessage: () => {},
  respondPermission: () => {},
  answerQuestion: () => {},
  approvePlan: () => {},
  submitElicitation: () => {},
  setPermissionMode: () => {},
}

export function useSessionActions(
  send: ((msg: Record<string, unknown>) => void) | null,
): SessionActions {
  return useMemo(() => {
    if (!send) return NOOP_ACTIONS

    return {
      sendMessage: (text: string) => {
        send({ type: 'user_message', content: text })
      },
      respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => {
        send({ type: 'permission_response', requestId, allowed, updatedPermissions })
      },
      answerQuestion: (requestId: string, answers: Record<string, string>) => {
        send({ type: 'question_response', requestId, answers })
      },
      approvePlan: (requestId: string, approved: boolean, feedback?: string) => {
        send({ type: 'plan_response', requestId, approved, feedback })
      },
      submitElicitation: (requestId: string, response: string) => {
        send({ type: 'elicitation_response', requestId, response })
      },
      setPermissionMode: (mode: string) => {
        send({ type: 'set_mode', mode })
      },
    }
  }, [send])
}
