import { useMemo } from 'react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { cn } from '../../lib/utils'

interface SwimLanesProps {
  subAgents: SubAgentInfo[]
  /** Whether the parent session is still active (reserved for future use) */
  sessionActive?: boolean
  /** Called when a sub-agent row is clicked to drill down into its conversation */
  onDrillDown?: (agentId: string, agentType: string, description: string) => void
}

/** Format cost as $X.XX */
function formatCost(usd: number): string {
  return `$${usd.toFixed(2)}`
}

/** Format duration from milliseconds to human-readable string */
function formatDuration(ms: number): string {
  const seconds = Math.round(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}

/** Status dot component with color based on agent status */
function StatusDot({ status }: { status: SubAgentInfo['status'] }) {
  const colorClass = {
    running: 'bg-green-500',
    complete: 'bg-gray-400 dark:bg-gray-500',
    error: 'bg-red-500',
  }[status]

  return <div className={cn('w-2 h-2 rounded-full flex-shrink-0', colorClass)} />
}

/** Animated indeterminate progress bar for running agents */
function ProgressBar() {
  return (
    <div className="w-full h-1 bg-gray-200 dark:bg-gray-800 rounded-full overflow-hidden">
      <div
        className="h-full bg-blue-500 rounded-full"
        style={{
          animation: 'swimlane-progress 1.5s ease-in-out infinite',
        }}
      />
    </div>
  )
}

/**
 * SwimLanes — flat list of sub-agents with all info visible.
 *
 * Each row shows:
 * - Status dot (green=running, gray=complete, red=error)
 * - Agent type badge
 * - Description text
 * - For running: current activity or progress bar
 * - For finished: cost, duration, tool call count
 *
 * Clicking a row opens the terminal drill-down (when agentId is available).
 * Rows are sorted: running first (by startedAt), then completed (by completedAt desc).
 */
export function SwimLanes({ subAgents, onDrillDown }: SwimLanesProps) {
  // Sort: Running first (by startedAt asc), then Complete/Error (by completedAt desc)
  const sortedAgents = useMemo(() => {
    const running = subAgents
      .filter((a) => a.status === 'running')
      .sort((a, b) => a.startedAt - b.startedAt)

    const finished = subAgents
      .filter((a) => a.status !== 'running')
      .sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0))

    return [...running, ...finished]
  }, [subAgents])

  if (subAgents.length === 0) return null

  return (
    <div
      className={cn(
        'flex flex-col gap-1 bg-white dark:bg-gray-950 border border-gray-200 dark:border-gray-800 rounded-lg p-2',
        subAgents.length > 5 && 'max-h-[360px] overflow-y-auto'
      )}
    >
      {sortedAgents.map((agent) => {
        const canDrillDown = !!agent.agentId && !!onDrillDown

        return (
          <div
            key={agent.toolUseId}
            role={canDrillDown ? 'button' : undefined}
            tabIndex={canDrillDown ? 0 : undefined}
            onClick={canDrillDown ? () => onDrillDown(agent.agentId!, agent.agentType, agent.description) : undefined}
            onKeyDown={canDrillDown ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                onDrillDown!(agent.agentId!, agent.agentType, agent.description)
              }
            } : undefined}
            className={cn(
              'flex flex-col gap-1 rounded-md px-2.5 py-2 transition-colors',
              canDrillDown && 'cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-900/50',
            )}
          >
            {/* Row 1: status + type + description + error tag */}
            <div className="flex items-center gap-2">
              <StatusDot status={agent.status} />
              <span className="text-xs font-mono text-gray-500 dark:text-gray-400 uppercase tracking-wide flex-shrink-0">
                {agent.agentType}
              </span>
              <span className="text-sm text-gray-700 dark:text-gray-300 flex-1 truncate">
                {agent.description}
              </span>
              {agent.status === 'error' && (
                <span className="text-[10px] font-semibold text-red-500 dark:text-red-400 uppercase flex-shrink-0">
                  ERR
                </span>
              )}
            </div>

            {/* Row 2: metrics (finished) or activity (running) */}
            {agent.status === 'running' ? (
              <div className="pl-4 flex items-center gap-2">
                {agent.currentActivity ? (
                  <span className="text-xs font-mono text-blue-600 dark:text-blue-400 flex items-center gap-1.5">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse" />
                    {agent.currentActivity}
                  </span>
                ) : (
                  <ProgressBar />
                )}
                {!agent.agentId && onDrillDown && (
                  <span className="text-[10px] text-gray-400 dark:text-gray-500 italic">
                    awaiting agent ID...
                  </span>
                )}
              </div>
            ) : (
              <div className="flex items-center gap-1.5 pl-4 text-xs font-mono text-gray-400 dark:text-gray-500">
                {agent.costUsd != null && (
                  <span>{formatCost(agent.costUsd)}</span>
                )}
                {agent.costUsd != null && agent.durationMs != null && (
                  <span className="text-gray-300 dark:text-gray-600">&middot;</span>
                )}
                {agent.durationMs != null && (
                  <span>{formatDuration(agent.durationMs)}</span>
                )}
                {agent.durationMs != null && agent.toolUseCount != null && (
                  <span className="text-gray-300 dark:text-gray-600">&middot;</span>
                )}
                {agent.toolUseCount != null && (
                  <span>{agent.toolUseCount} tool call{agent.toolUseCount !== 1 ? 's' : ''}</span>
                )}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
