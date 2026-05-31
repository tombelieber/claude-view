import { Bot } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { WorkflowAgentSummary } from '../../types/generated/WorkflowAgentSummary'
import { formatNumber } from './run-detail-format'

/** Shared column template so the agents header and rows stay aligned. */
export const AGENT_ROW_GRID = 'grid grid-cols-[1.2fr_120px_100px_100px] gap-3'

export function WorkflowAgentRow({
  agent,
  selected,
  onSelect,
}: {
  agent: WorkflowAgentSummary
  selected: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        AGENT_ROW_GRID,
        'w-full items-center border-b border-gray-200 px-4 py-3 text-left text-sm transition-colors dark:border-gray-800',
        selected ? 'bg-blue-50 dark:bg-blue-950/30' : 'hover:bg-gray-50 dark:hover:bg-gray-900/70',
      )}
    >
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 shrink-0 text-gray-500" />
          <span className="truncate font-medium text-gray-950 dark:text-white">
            {agent.label ?? agent.agentId}
          </span>
        </div>
        <div className="mt-1 truncate text-xs text-gray-500">
          {agent.phaseTitle ?? `Agent ${agent.agentId}`}
        </div>
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">{agent.state}</div>
      <div className="text-xs text-gray-600 dark:text-gray-300">
        {formatNumber(Number(agent.tokens))}
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">{agent.toolCalls}</div>
    </button>
  )
}
