import { X } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { computeCategoryCounts } from '../../lib/compute-category-counts'
import { cn } from '../../lib/utils'
import { useMonitorStore } from '../../store/monitor-store'
import { RichPane } from './RichPane'
import { ViewModeControls } from './ViewModeControls'
import { useSubAgentStream } from './use-subagent-stream'

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
  const verboseMode = useMonitorStore((s) => s.verboseMode)

  const noop = useCallback(() => {}, [])

  const {
    connectionState,
    messages: streamMessages,
    bufferDone,
  } = useSubAgentStream({
    sessionId,
    agentId,
    enabled: true,
    onMessage: noop,
  })

  const categoryCounts = useMemo(() => computeCategoryCounts(streamMessages), [streamMessages])

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header — same density as SessionDetailPanel tab bar */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 flex-shrink-0">
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 uppercase tracking-wide flex-shrink-0">
          {agentType}
        </span>
        <span className="text-[10px] text-gray-400 dark:text-gray-500 font-mono flex-shrink-0">
          {agentId}
        </span>
        <span className="text-xs text-gray-700 dark:text-gray-300 truncate min-w-0">
          {description}
        </span>

        <div className="flex-1" />

        {/* Connection status */}
        <span
          className={cn(
            'text-[10px] font-mono flex-shrink-0',
            connectionState === 'connected' && 'text-green-600 dark:text-green-400',
            connectionState === 'connecting' && 'text-yellow-600 dark:text-yellow-400',
            connectionState === 'disconnected' && 'text-gray-400 dark:text-gray-500',
            connectionState === 'error' && 'text-red-500 dark:text-red-400',
          )}
        >
          {connectionState}
        </span>

        {/* Same Chat/Debug + Rich/JSON controls as Terminal tab */}
        <ViewModeControls />

        {/* Close button */}
        <button
          type="button"
          onClick={onClose}
          aria-label="Close sub-agent drill-down"
          className="text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors p-0.5 flex-shrink-0"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Conversation — identical to Terminal tab */}
      <div className="flex-1 min-h-0">
        <RichPane
          messages={streamMessages}
          isVisible={true}
          verboseMode={verboseMode}
          bufferDone={bufferDone}
          categoryCounts={categoryCounts}
        />
      </div>
    </div>
  )
}
