import { createContext, useContext } from 'react'

export interface ConversationActions {
  retryMessage: (localId: string) => void
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
