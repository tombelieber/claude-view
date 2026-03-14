import { createContext, useContext } from 'react'

export interface ConversationActions {
  retryMessage: (localId: string) => void
  stopTask?: (taskId: string) => void
  // Interactive card handlers (permission, question, plan, elicitation)
  respondPermission?: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => void
  answerQuestion?: (requestId: string, answers: Record<string, string>) => void
  approvePlan?: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation?: (requestId: string, response: string) => void
}

const ConversationActionsContext = createContext<ConversationActions | null>(null)

export function ConversationActionsProvider({
  children,
  actions,
}: {
  children: React.ReactNode
  actions: ConversationActions
}) {
  return (
    <ConversationActionsContext.Provider value={actions}>
      {children}
    </ConversationActionsContext.Provider>
  )
}

export function useConversationActions(): ConversationActions | null {
  return useContext(ConversationActionsContext)
}
