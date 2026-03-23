/**
 * SharedConversationView -- renders a shared conversation using the
 * ConversationThread + ConversationBlock pipeline from @claude-view/shared.
 *
 * The share viewer is read-only — no interactive actions (permissions,
 * questions, etc.) are provided. ConversationActionsContext defaults to null
 * which disables interactive UI elements in the block renderers.
 *
 * Code blocks use plain-text rendering (no shiki syntax highlighting) since
 * the share viewer is a lightweight read-only SPA. The web app injects
 * shiki-based renderers via CodeRenderProvider for enhanced highlighting.
 */

import type { Message } from '@claude-view/shared'
import { ConversationThread } from '@claude-view/shared/components/conversation/ConversationThread'
import { chatRegistry } from '@claude-view/shared/components/conversation/blocks/chat/registry'
import { historyToBlocks } from '@claude-view/shared/lib/history-to-blocks'
import { useMemo } from 'react'

interface SharedConversationViewProps {
  messages: Message[]
  verboseMode?: boolean
}

/** Strings that Claude Code emits as placeholder content (no real text) */
const EMPTY_CONTENT = new Set(['(no content)', ''])

function filterMessages(messages: Message[]): Message[] {
  return messages.filter((msg) => {
    if (msg.role === 'user') return true
    if (msg.role === 'assistant') {
      if (EMPTY_CONTENT.has(msg.content.trim()) && !msg.thinking) return false
      return true
    }
    return false
  })
}

export function SharedConversationView({ messages, verboseMode }: SharedConversationViewProps) {
  const filtered = useMemo(
    () => (verboseMode ? messages : filterMessages(messages)),
    [messages, verboseMode],
  )

  const blocks = useMemo(() => historyToBlocks(filtered), [filtered])

  if (blocks.length === 0) {
    return (
      <div className="text-center text-gray-400 dark:text-gray-500 py-12 text-sm">
        No messages to display.
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 min-h-0">
        <ConversationThread blocks={blocks} renderers={chatRegistry} />
      </div>
      <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400 dark:text-gray-500 flex-shrink-0">
        {messages.length} messages
        {messages.length - filtered.length > 0 && (
          <> &bull; {messages.length - filtered.length} system messages hidden</>
        )}
      </div>
    </div>
  )
}
