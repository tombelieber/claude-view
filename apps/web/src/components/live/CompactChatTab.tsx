// Compact chat view for the live monitor side panel.
// Receives pre-fetched blocks from SessionDetailPanel (shared useConversation call)
// to avoid opening a second WebSocket connection to the same session.
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { ConversationThread } from '../conversation/ConversationThread'
import { chatRegistry } from '../conversation/blocks/chat/registry'

interface CompactChatTabProps {
  blocks: ConversationBlock[]
}

export function CompactChatTab({ blocks }: CompactChatTabProps) {
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
