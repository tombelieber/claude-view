import { useBlockSocket } from '../../hooks/use-block-socket'
import { useMonitorStore } from '../../store/monitor-store'
import { ConversationThread } from '../conversation/ConversationThread'
import { chatRegistry } from '../conversation/blocks/chat/registry'
import { developerRegistry } from '../conversation/blocks/developer/registry'

interface BlockTerminalPaneProps {
  sessionId: string
  isVisible: boolean
  agentId?: string
  compact?: boolean
}

export function BlockTerminalPane({
  sessionId,
  isVisible,
  agentId,
  compact = true,
}: BlockTerminalPaneProps) {
  const displayMode = useMonitorStore((s) => s.displayMode)
  const registry = displayMode === 'chat' ? chatRegistry : developerRegistry

  const { blocks } = useBlockSocket({
    sessionId,
    enabled: isVisible,
    agentId,
  })

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-gray-400 dark:text-gray-500 p-4">
        No messages yet
      </div>
    )
  }

  return (
    <ConversationThread
      blocks={blocks}
      renderers={registry}
      compact={compact}
      filterBar={!compact && displayMode === 'developer'}
    />
  )
}
