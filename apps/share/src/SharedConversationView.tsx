/**
 * SharedConversationView -- renders a shared conversation using the exact same
 * components as the web app's session detail view (compact mode).
 *
 * Imports from @claude-view/shared so shared links have 100% visual parity
 * with the main app, without coupling to apps/web source.
 */

import { ErrorBoundary } from '@claude-view/shared/components/ErrorBoundary'
import { MessageTyped } from '@claude-view/shared/components/MessageTyped'
import { ExpandProvider } from '@claude-view/shared/contexts/ExpandContext'
import { ThreadHighlightProvider } from '@claude-view/shared/contexts/ThreadHighlightContext'
import type { Message } from '@claude-view/shared/types/message'
import { buildThreadMap, getThreadChain } from '@claude-view/shared/utils/thread-map'
import { useCallback, useMemo } from 'react'

interface SharedConversationViewProps {
  messages: Message[]
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

export function SharedConversationView({ messages }: SharedConversationViewProps) {
  const filtered = useMemo(() => filterMessages(messages), [messages])

  const threadMap = useMemo(() => buildThreadMap(filtered), [filtered])

  const getThreadChainForUuid = useCallback(
    (uuid: string) => getThreadChain(uuid, filtered),
    [filtered],
  )

  if (filtered.length === 0) {
    return (
      <div className="text-center text-gray-400 dark:text-gray-500 py-12 text-sm">
        No messages to display.
      </div>
    )
  }

  return (
    <ThreadHighlightProvider>
      <ExpandProvider>
        <div className="space-y-0">
          {filtered.map((message, index) => {
            const thread = message.uuid ? threadMap.get(message.uuid) : undefined
            return (
              <div key={message.uuid || index} className="max-w-4xl mx-auto px-6 pb-4">
                <ErrorBoundary>
                  <MessageTyped
                    message={message}
                    messageIndex={index}
                    messageType={message.role}
                    metadata={message.metadata as Record<string, any>}
                    parentUuid={thread?.parentUuid}
                    indent={thread?.indent ?? 0}
                    isChildMessage={thread?.isChild ?? false}
                    onGetThreadChain={getThreadChainForUuid}
                    showThinking={false}
                  />
                </ErrorBoundary>
              </div>
            )
          })}
          <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
            {messages.length} messages
            {messages.length - filtered.length > 0 && (
              <> &bull; {messages.length - filtered.length} system messages hidden</>
            )}
          </div>
        </div>
      </ExpandProvider>
    </ThreadHighlightProvider>
  )
}
