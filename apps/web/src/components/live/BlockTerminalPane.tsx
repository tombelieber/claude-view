import { ErrorBoundary } from '@claude-view/shared/components/ErrorBoundary'
import { useSessionChannel } from '../../hooks/use-session-channel'
import { useMonitorStore } from '../../store/monitor-store'
import { ConversationThread } from '@claude-view/shared/components/conversation/ConversationThread'
import { chatRegistry } from '@claude-view/shared/components/conversation/blocks/chat/registry'
import { developerRegistry } from '@claude-view/shared/components/conversation/blocks/developer/registry'

interface BlockTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  compact?: boolean
}

export function BlockTerminalPane({
  sessionId,
  isVisible,
  compact = true,
}: BlockTerminalPaneProps) {
  const displayMode = useMonitorStore((s) => s.displayMode)
  const registry = displayMode === 'chat' ? chatRegistry : developerRegistry

  const { blocks } = useSessionChannel({
    sessionId,
    modes: ['block'],
    enabled: isVisible,
  })

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-gray-400 dark:text-gray-500 p-4">
        No messages yet
      </div>
    )
  }

  return (
    <ErrorBoundary>
      <ConversationThread
        blocks={blocks}
        renderers={registry}
        compact={compact}
        filterBar={!compact && displayMode === 'developer'}
      />
    </ErrorBoundary>
  )
}
