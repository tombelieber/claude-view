/**
 * SharedConversationView -- renders a shared conversation using the same
 * MessageTyped component as the web app (from @claude-view/shared).
 *
 * Code blocks use plain-text rendering (no shiki syntax highlighting) since
 * the share viewer is a lightweight read-only SPA. The web app injects
 * shiki-based renderers via CodeRenderProvider for enhanced highlighting.
 */

import {
  ErrorBoundary,
  ExpandProvider,
  type Message,
  MessageTyped,
  ThreadHighlightProvider,
  buildThreadMap,
  getThreadChain,
} from '@claude-view/shared'
import { useCallback, useMemo } from 'react'

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
                    metadata={
                      typeof message.metadata === 'object' && message.metadata !== null
                        ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
                          (message.metadata as Record<string, any>)
                        : undefined
                    }
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
