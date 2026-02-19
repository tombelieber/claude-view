import { useCallback, useState } from 'react'
import { X } from 'lucide-react'
import { RichPane } from './RichPane'
import { useSubAgentStream } from './use-subagent-stream'
import { cn } from '../../lib/utils'

interface SubAgentDrillDownProps {
  sessionId: string
  agentId: string
  agentType: string
  description: string
  onClose: () => void
}

export function SubAgentDrillDown({
  sessionId,
  agentId,
  agentType,
  description,
  onClose,
}: SubAgentDrillDownProps) {
  const [verboseMode, setVerboseMode] = useState(false)

  const noop = useCallback(() => {}, [])

  const { connectionState, messages: streamMessages, bufferDone } = useSubAgentStream({
    sessionId,
    agentId,
    enabled: true,
    onMessage: noop,
  })

  return (
    <div className="flex flex-col h-full bg-white dark:bg-gray-950 border border-gray-200 dark:border-gray-800 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          {agentType}
        </span>
        <span className="text-xs text-gray-400 dark:text-gray-500">id:{agentId}</span>
        <span className="text-sm text-gray-700 dark:text-gray-300 flex-1 truncate">{description}</span>

        {/* Connection status */}
        <span className={cn(
          'text-[10px] font-mono',
          connectionState === 'connected' && 'text-green-600 dark:text-green-400',
          connectionState === 'connecting' && 'text-yellow-600 dark:text-yellow-400',
          connectionState === 'disconnected' && 'text-gray-400 dark:text-gray-500',
          connectionState === 'error' && 'text-red-500 dark:text-red-400',
        )}>
          {connectionState}
        </span>

        {/* Verbose toggle */}
        <button
          onClick={() => setVerboseMode(!verboseMode)}
          className={cn(
            'text-[10px] px-1.5 py-0.5 rounded border',
            verboseMode
              ? 'border-blue-500 text-blue-600 dark:text-blue-400'
              : 'border-gray-300 dark:border-gray-700 text-gray-500 hover:text-gray-700 dark:hover:text-gray-400',
          )}
        >
          {verboseMode ? 'verbose' : 'compact'}
        </button>

        {/* Close button */}
        <button
          onClick={onClose}
          aria-label="Close sub-agent drill-down"
          className="text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Conversation */}
      <div className="flex-1 min-h-0">
        <RichPane
          messages={streamMessages}
          isVisible={true}
          verboseMode={verboseMode}
          bufferDone={bufferDone}
        />
      </div>
    </div>
  )
}
