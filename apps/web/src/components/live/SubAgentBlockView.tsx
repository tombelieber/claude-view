import { X } from 'lucide-react'
import { cn } from '../../lib/utils'
import { useBlockSocket } from '../../hooks/use-block-socket'
import { useMonitorStore } from '../../store/monitor-store'
import { ConversationThread } from '@claude-view/shared/components/conversation/ConversationThread'
import { chatRegistry } from '@claude-view/shared/components/conversation/blocks/chat/registry'
import { developerRegistry } from '@claude-view/shared/components/conversation/blocks/developer/registry'
import { DisplayModeToggle } from './DisplayModeToggle'

interface SubAgentBlockViewProps {
  sessionId: string
  agentId: string
  agentType: string
  description: string
  onClose: () => void
}

export function SubAgentBlockView({
  sessionId,
  agentId,
  agentType,
  description,
  onClose,
}: SubAgentBlockViewProps) {
  const displayMode = useMonitorStore((s) => s.displayMode)
  const registry = displayMode === 'chat' ? chatRegistry : developerRegistry

  const { blocks, connectionState, error } = useBlockSocket({
    sessionId,
    agentId,
    enabled: true,
    scrollback: 100_000,
  })

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 flex-shrink-0">
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 uppercase tracking-wide flex-shrink-0">
          {agentType}
        </span>
        <span className="text-xs text-gray-400 dark:text-gray-500 font-mono flex-shrink-0">
          {agentId}
        </span>
        <span className="text-xs text-gray-700 dark:text-gray-300 truncate min-w-0">
          {description}
        </span>
        <div className="flex-1" />
        <span
          className={cn(
            'text-xs font-mono flex-shrink-0',
            connectionState === 'connected' && 'text-green-600 dark:text-green-400',
            connectionState === 'connecting' && 'text-yellow-600 dark:text-yellow-400',
            connectionState === 'disconnected' && 'text-gray-400 dark:text-gray-500',
            connectionState === 'error' && 'text-red-500 dark:text-red-400',
          )}
        >
          {connectionState}
        </span>
        <DisplayModeToggle />
        <button
          type="button"
          onClick={onClose}
          aria-label="Close sub-agent view"
          className="text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-0.5 flex-shrink-0"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
      <div className="flex-1 min-h-0">
        {error ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 p-4">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
            <p className="text-xs text-gray-400 dark:text-gray-500">
              Sub-agent JSONL may not exist yet or session has ended
            </p>
          </div>
        ) : blocks.length === 0 && connectionState !== 'connecting' ? (
          <div className="flex items-center justify-center h-full text-sm text-gray-400 dark:text-gray-500 p-4">
            {connectionState === 'error' ? 'Failed to load sub-agent content' : 'No messages yet'}
          </div>
        ) : (
          <ConversationThread
            blocks={blocks}
            renderers={registry}
            filterBar={displayMode === 'developer'}
          />
        )}
      </div>
    </div>
  )
}
