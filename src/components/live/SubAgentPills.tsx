import { useCallback } from 'react'
import { Loader2, CheckCircle, XCircle } from 'lucide-react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface SubAgentPillsProps {
  subAgents: SubAgentInfo[]
  /** Click handler to expand into swim lane view */
  onExpand?: () => void
}

/**
 * Mini cards for display inside monitor grid panes (Phase C) where space is limited.
 * Each agent rendered as a small pill: [type initial] [status icon].
 *
 * Layout example: [E ⟳] [C ⟳] [S done]    3 agents (2 active)
 */
export function SubAgentPills({ subAgents, onExpand }: SubAgentPillsProps) {
  // Derive computed values before early return
  const activeCount = subAgents.filter(a => a.status === 'running').length
  const displayAgents = subAgents.slice(0, 3)
  const hasMore = subAgents.length > 3

  // Memoized keyboard handler for performance in list rendering
  // MUST be called before early return to satisfy Rules of Hooks
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (onExpand && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault()
      onExpand()
    }
  }, [onExpand])

  // Early return if no sub-agents
  if (subAgents.length === 0) {
    return null
  }

  // Calculate summary text
  const summaryText = activeCount > 0
    ? `${subAgents.length} agent${subAgents.length > 1 ? 's' : ''} (${activeCount} active)`
    : `${subAgents.length} agent${subAgents.length > 1 ? 's' : ''} (all done)`

  return (
    <div
      className={`flex items-center gap-2 px-2 py-1 rounded transition-colors ${
        onExpand ? 'cursor-pointer hover:bg-zinc-50 dark:hover:bg-zinc-800/50' : ''
      }`}
      onClick={onExpand}
      role={onExpand ? 'button' : undefined}
      tabIndex={onExpand ? 0 : undefined}
      onKeyDown={handleKeyDown}
    >
      {/* Pills container */}
      <div className="flex items-center gap-1.5">
        {displayAgents.map((agent) => (
          <AgentPill key={agent.toolUseId} agent={agent} />
        ))}
        {hasMore && (
          <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border border-zinc-300 dark:border-zinc-600 bg-zinc-50 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400">
            +{subAgents.length - 3} more
          </span>
        )}
      </div>

      {/* Summary text */}
      <span className="text-xs text-gray-600 dark:text-gray-400 whitespace-nowrap">
        {summaryText}
      </span>
    </div>
  )
}

function AgentPill({ agent }: { agent: SubAgentInfo }) {
  // Get first letter of agent type for display
  const initial = agent.agentType[0]?.toUpperCase() || 'T'

  // Determine status icon and styling
  const statusConfig = getStatusConfig(agent.status)

  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium border ${statusConfig.borderClass} ${statusConfig.bgClass} ${statusConfig.textClass}`}
      title={agent.currentActivity
        ? `${agent.agentType}: ${agent.currentActivity}`
        : `${agent.agentType}: ${agent.description}`}
    >
      <span className="font-semibold">{initial}</span>
      {statusConfig.icon}
    </span>
  )
}

function getStatusConfig(status: SubAgentInfo['status']) {
  switch (status) {
    case 'running':
      return {
        icon: <Loader2 className="h-3 w-3 animate-spin" />,
        borderClass: 'border-green-500 dark:border-green-400',
        bgClass: 'bg-green-50 dark:bg-green-900/20',
        textClass: 'text-green-700 dark:text-green-300',
      }
    case 'complete':
      return {
        icon: <CheckCircle className="h-3 w-3" />,
        borderClass: 'border-zinc-300 dark:border-zinc-600',
        bgClass: 'bg-zinc-50 dark:bg-zinc-800',
        textClass: 'text-zinc-600 dark:text-zinc-400',
      }
    case 'error':
      return {
        icon: <XCircle className="h-3 w-3" />,
        borderClass: 'border-red-500 dark:border-red-400',
        bgClass: 'bg-red-50 dark:bg-red-900/20',
        textClass: 'text-red-700 dark:text-red-300',
      }
  }
}
