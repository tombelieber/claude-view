// Compact chat view for the live monitor side panel.
// Uses useConversation to connect to the session's WS (live) or load history,
// then renders all blocks through chatRegistry in compact mode.
import { useConversation } from '../../hooks/use-conversation'
import { ConversationThread } from '../conversation/ConversationThread'
import { chatRegistry } from '../conversation/blocks/chat/registry'

interface CompactChatTabProps {
  sessionId: string
}

export function CompactChatTab({ sessionId }: CompactChatTabProps) {
  const { blocks } = useConversation(sessionId)

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-gray-400 dark:text-gray-500 p-4">
        No messages yet
      </div>
    )
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto p-3">
      <ConversationThread blocks={blocks} renderers={chatRegistry} compact />
    </div>
  )
}
