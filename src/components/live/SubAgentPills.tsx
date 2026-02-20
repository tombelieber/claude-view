import { useCallback } from 'react'
import * as Tooltip from '@radix-ui/react-tooltip'
import { Loader2, CheckCircle, XCircle } from 'lucide-react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface SubAgentPillsProps {
  subAgents: SubAgentInfo[]
  /** Click handler to expand into swim lane view */
  onExpand?: () => void
}

function formatDuration(ms: number): string {
  const seconds = Math.round(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}

const TOOLTIP_CONTENT_CLASS = 'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs'
const TOOLTIP_ARROW_CLASS = 'fill-gray-200 dark:fill-gray-700'

/**
 * Mini cards for display inside monitor grid panes (Phase C) where space is limited.
 * Each agent rendered as a small pill: [type initial] [status icon].
 * Hover on any pill shows full agent name, description, and metrics breakdown.
 *
 * Layout example: [E ⟳] [C ⟳] [S done]    3 agents (2 active)
 */
export function SubAgentPills({ subAgents, onExpand }: SubAgentPillsProps) {
  // Derive computed values before early return
  const activeCount = subAgents.filter(a => a.status === 'running').length
  const displayAgents = subAgents.slice(0, 3)
  const overflowAgents = subAgents.slice(3)
  const hasMore = overflowAgents.length > 0

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
    <Tooltip.Provider delayDuration={200}>
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
            <OverflowPill agents={overflowAgents} />
          )}
        </div>

        {/* Summary text */}
        <span className="text-xs text-gray-600 dark:text-gray-400 whitespace-nowrap">
          {summaryText}
        </span>
      </div>
    </Tooltip.Provider>
  )
}

function AgentPill({ agent }: { agent: SubAgentInfo }) {
  // Get first letter of agent type for display
  const initial = agent.agentType[0]?.toUpperCase() || 'T'

  // Determine status icon and styling
  const statusConfig = getStatusConfig(agent.status)

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span
          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium border cursor-default ${statusConfig.borderClass} ${statusConfig.bgClass} ${statusConfig.textClass}`}
          aria-label={`${agent.agentType}: ${agent.description}`}
        >
          <span className="font-semibold">{initial}</span>
          {statusConfig.icon}
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <AgentTooltipContent agent={agent} />
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function AgentTooltipContent({ agent }: { agent: SubAgentInfo }) {
  const statusConfig = getStatusConfig(agent.status)

  return (
    <div className="space-y-1.5">
      {/* Full agent name + status */}
      <div className="flex items-center gap-2">
        <span className="font-medium text-gray-900 dark:text-gray-100">
          {agent.agentType}
        </span>
        <span className={`inline-flex items-center gap-0.5 ${statusConfig.textClass}`}>
          {statusConfig.icon}
        </span>
      </div>

      {/* Description */}
      <div className="text-gray-500 dark:text-gray-400">
        {agent.description}
      </div>

      {/* Running: current activity */}
      {agent.status === 'running' && agent.currentActivity && (
        <div className="flex items-center gap-1.5 text-blue-600 dark:text-blue-400 font-mono">
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse flex-shrink-0" />
          {agent.currentActivity}
        </div>
      )}

      {/* Completed/Error: metrics row */}
      {agent.status !== 'running' && (agent.costUsd != null || agent.durationMs != null || agent.toolUseCount != null) && (
        <div className="flex items-center gap-1.5 font-mono text-gray-400 dark:text-gray-500">
          {agent.costUsd != null && <span>${agent.costUsd.toFixed(2)}</span>}
          {agent.costUsd != null && agent.durationMs != null && (
            <span className="text-gray-300 dark:text-gray-600">&middot;</span>
          )}
          {agent.durationMs != null && <span>{formatDuration(agent.durationMs)}</span>}
          {agent.durationMs != null && agent.toolUseCount != null && (
            <span className="text-gray-300 dark:text-gray-600">&middot;</span>
          )}
          {agent.toolUseCount != null && (
            <span>{agent.toolUseCount} tool{agent.toolUseCount !== 1 ? 's' : ''}</span>
          )}
        </div>
      )}
    </div>
  )
}

function OverflowPill({ agents }: { agents: SubAgentInfo[] }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border border-zinc-300 dark:border-zinc-600 bg-zinc-50 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 cursor-default">
          +{agents.length} more
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <div className="space-y-2">
            {agents.map((agent) => {
              const statusConfig = getStatusConfig(agent.status)
              return (
                <div key={agent.toolUseId} className="flex items-start gap-2">
                  <span className={`flex-shrink-0 mt-0.5 ${statusConfig.textClass}`}>
                    {statusConfig.icon}
                  </span>
                  <div className="min-w-0">
                    <span className="font-medium text-gray-900 dark:text-gray-100">
                      {agent.agentType}
                    </span>
                    <span className="text-gray-400 dark:text-gray-500 ml-1.5">
                      {agent.description}
                    </span>
                  </div>
                </div>
              )
            })}
          </div>
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
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
