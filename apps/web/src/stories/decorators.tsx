/**
 * Shared Storybook decorators for conversation components.
 */
import { ConversationActionsProvider } from '../contexts/conversation-actions-context'
import type { ConversationActions } from '../contexts/conversation-actions-context'

/** No-op actions for stories that need ConversationActionsProvider but don't test actions. */
const noopActions: ConversationActions = {
  retryMessage: () => {},
  stopTask: () => {},
  respondPermission: () => {},
  answerQuestion: () => {},
  approvePlan: () => {},
  submitElicitation: () => {},
}

/** Wraps a story in ConversationActionsProvider with no-op handlers. */
export function withConversationActions(Story: React.ComponentType) {
  return (
    <ConversationActionsProvider actions={noopActions}>
      <Story />
    </ConversationActionsProvider>
  )
}

/** Wraps story in a container that mimics the chat panel width. */
export function withChatContainer(Story: React.ComponentType) {
  return (
    <div className="w-[640px] max-w-full">
      <Story />
    </div>
  )
}

/** Combines conversation actions + chat container. */
export function withChatContext(Story: React.ComponentType) {
  return (
    <ConversationActionsProvider actions={noopActions}>
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    </ConversationActionsProvider>
  )
}
