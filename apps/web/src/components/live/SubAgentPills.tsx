import * as Tooltip from '@radix-ui/react-tooltip'
import { CheckCircle, Loader2, XCircle } from 'lucide-react'
import { useCallback } from 'react'
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

const TIP =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs'
const ARROW = 'fill-gray-200 dark:fill-gray-700'

const PILL =
  'inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium cursor-default transition-colors duration-200'

const STATUS_STYLE = {
  running: `${PILL} bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300`,
  complete: `${PILL} bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400`,
  error: `${PILL} bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300`,
} as const

const STATUS_ICON = {
  running: <Loader2 className="h-2.5 w-2.5 animate-spin" />,
  complete: <CheckCircle className="h-2.5 w-2.5" />,
  error: <XCircle className="h-2.5 w-2.5" />,
} as const

export function SubAgentPills({ subAgents, onExpand }: SubAgentPillsProps) {
  const displayAgents = subAgents.slice(0, 4)
  const overflowAgents = subAgents.slice(4)

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (onExpand && (e.key === 'Enter' || e.key === ' ')) {
        e.preventDefault()
        onExpand()
      }
    },
    [onExpand],
  )

  if (subAgents.length === 0) return null

  return (
    <Tooltip.Provider delayDuration={200}>
      <div
        className={`flex items-center gap-1 flex-wrap ${onExpand ? 'cursor-pointer' : ''}`}
        onClick={onExpand}
        role={onExpand ? 'button' : undefined}
        tabIndex={onExpand ? 0 : undefined}
        onKeyDown={handleKeyDown}
      >
        {displayAgents.map((agent) => (
          <AgentPill key={agent.toolUseId} agent={agent} />
        ))}
        {overflowAgents.length > 0 && <OverflowPill agents={overflowAgents} />}
      </div>
    </Tooltip.Provider>
  )
}

function AgentPill({ agent }: { agent: SubAgentInfo }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span
          className={STATUS_STYLE[agent.status]}
          aria-label={`${agent.agentType}: ${agent.description}`}
        >
          {STATUS_ICON[agent.status]}
          {agent.agentType}
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TIP} sideOffset={5}>
          <AgentTooltipContent agent={agent} />
          <Tooltip.Arrow className={ARROW} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function AgentTooltipContent({ agent }: { agent: SubAgentInfo }) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2">
        <span className="font-medium text-gray-900 dark:text-gray-100">{agent.agentType}</span>
        <span className={STATUS_STYLE[agent.status].replace(PILL, '').trim()}>
          {STATUS_ICON[agent.status]}
        </span>
      </div>
      <div className="text-gray-500 dark:text-gray-400">{agent.description}</div>

      {agent.status === 'running' && agent.currentActivity && (
        <div className="flex items-center gap-1.5 text-blue-600 dark:text-blue-400 font-mono">
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse flex-shrink-0" />
          {agent.currentActivity}
        </div>
      )}

      {agent.status !== 'running' &&
        (agent.costUsd != null || agent.durationMs != null || agent.toolUseCount != null) && (
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
              <span>
                {agent.toolUseCount} tool{agent.toolUseCount !== 1 ? 's' : ''}
              </span>
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
        <span className={`${PILL} bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400`}>
          +{agents.length} more
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TIP} sideOffset={5}>
          <div className="space-y-2">
            {agents.map((agent) => (
              <div key={agent.toolUseId} className="flex items-start gap-2">
                <span className="flex-shrink-0 mt-0.5">{STATUS_ICON[agent.status]}</span>
                <div className="min-w-0">
                  <span className="font-medium text-gray-900 dark:text-gray-100">
                    {agent.agentType}
                  </span>
                  <span className="text-gray-400 dark:text-gray-500 ml-1.5">
                    {agent.description}
                  </span>
                </div>
              </div>
            ))}
          </div>
          <Tooltip.Arrow className={ARROW} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}
