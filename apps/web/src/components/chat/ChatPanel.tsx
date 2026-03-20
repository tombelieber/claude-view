import type { IDockviewPanelProps } from 'dockview-react'
import { useState } from 'react'
import { useHistoryBlocks } from '../../hooks/use-history-blocks'
import { useSidecarConnection } from '../../hooks/use-sidecar-connection'
import type { PermissionMode } from '../../types/control'
import { ConversationThread } from '../conversation/ConversationThread'
import type { BlockRenderers } from '../conversation/types'
import { ChatInputBar } from './ChatInputBar'
import { ChatPanelHeader } from './ChatPanelHeader'
import { ChatStatusBar } from './ChatStatusBar'

interface ChatPanelParams {
  sessionId: string
  isWatching?: boolean
}

/**
 * Placeholder renderers — each block type just renders its text content.
 * Phase 3 will wire up rich renderers (code blocks, tool use, etc.).
 */
const placeholderRenderers: BlockRenderers = {
  user: ({ block }) => (
    <div className="px-4 py-2 text-sm text-gray-900 dark:text-gray-100">
      {'text' in block ? String(block.text) : ''}
    </div>
  ),
  assistant: ({ block }) => (
    <div className="px-4 py-2 text-sm text-gray-700 dark:text-gray-300">
      {'text' in block ? String(block.text) : ''}
    </div>
  ),
}

type DisplayMode = 'chat' | 'developer'

export function ChatPanel({ params }: IDockviewPanelProps<ChatPanelParams>) {
  const { sessionId, isWatching } = params
  const [displayMode, setDisplayMode] = useState<DisplayMode>('chat')

  // Live connection to sidecar (skipped for watching-only sessions)
  const connection = useSidecarConnection(sessionId, { skip: isWatching })

  // History blocks from Rust server REST API (for ended/non-live sessions)
  const history = useHistoryBlocks(sessionId || null, {
    enabled: !connection.isLive && !!sessionId,
  })

  // Single source of truth: live blocks if connected, else history
  const blocks = connection.isLive ? connection.committedBlocks : history.blocks

  const handleSend = (message: string) => {
    connection.send({ type: 'user_message', text: message })
  }

  const handleModeChange = (mode: PermissionMode) => {
    connection.send({ type: 'set_permission_mode', mode })
  }

  return (
    <div className="flex flex-col h-full">
      <ChatPanelHeader
        status={connection.status}
        isLive={connection.isLive}
        displayMode={displayMode}
        onDisplayModeChange={setDisplayMode}
        permissionMode={connection.permissionMode}
        onPermissionModeChange={handleModeChange}
      />
      <div className="flex-1 overflow-y-auto">
        {history.isLoading && !connection.isLive ? (
          <div className="flex items-center justify-center h-32 text-sm text-gray-400">
            Loading conversation...
          </div>
        ) : (
          <ConversationThread blocks={blocks} renderers={placeholderRenderers} />
        )}
      </div>
      <ChatStatusBar
        model={connection.model}
        contextTokens={connection.contextTokens}
        contextLimit={connection.contextLimit}
        contextPercent={connection.contextPercent}
        totalCost={connection.totalCost}
      />
      <ChatInputBar
        onSend={handleSend}
        state={connection.isLive ? 'active' : 'dormant'}
        mode={connection.permissionMode}
        onModeChange={handleModeChange}
        model={connection.model}
        contextPercent={connection.contextPercent}
      />
    </div>
  )
}
