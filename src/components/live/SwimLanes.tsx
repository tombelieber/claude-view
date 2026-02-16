import { useMemo } from 'react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { cn } from '../../lib/utils'

interface SwimLanesProps {
  subAgents: SubAgentInfo[]
  /** Whether the parent session is still active */
  sessionActive: boolean
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
    complete: 'bg-gray-500',
    error: 'bg-red-500',
  }[status]

  return <div className={cn('w-2 h-2 rounded-full flex-shrink-0', colorClass)} />
}

/** Animated indeterminate progress bar for running agents */
function ProgressBar() {
  return (
    <div className="w-full h-1 bg-gray-800 rounded-full overflow-hidden">
      <div
        className="h-full bg-blue-500 rounded-full animate-progress"
        style={{
          animation: 'progress 1.5s ease-in-out infinite',
        }}
      />
      <style>{`
        @keyframes progress {
          0% { width: 0%; margin-left: 0%; }
          50% { width: 40%; margin-left: 30%; }
          100% { width: 0%; margin-left: 100%; }
        }
      `}</style>
    </div>
  )
}

/**
 * SwimLanes — visualizes sub-agent execution as horizontal swim lanes.
 *
 * Each sub-agent renders as a horizontal row with:
 * - Status dot (green=running, gray=complete, red=error)
 * - Agent type badge
 * - Description text
 * - For running: animated progress bar
 * - For completed: cost, duration, tool call count
 *
 * Rows are sorted: running first (by startedAt), then completed (by completedAt desc).
 * Empty state returns null when no sub-agents exist.
 */
export function SwimLanes({ subAgents, sessionActive }: SwimLanesProps) {
  // Early return if no sub-agents
  if (subAgents.length === 0) return null

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

  return (
    <div
      className={cn(
        'flex flex-col gap-2 bg-gray-950 border border-gray-800 rounded-lg p-3',
        // Max height with scroll if > 5 agents (each lane ~48px + gap)
        subAgents.length > 5 && 'max-h-[280px] overflow-y-auto'
      )}
    >
      {sortedAgents.map((agent) => (
        <div
          key={agent.toolUseId}
          className="flex flex-col gap-1.5 border-b border-gray-800 last:border-b-0 pb-2 last:pb-0"
        >
          {/* Header row: status + type + description */}
          <div className="flex items-center gap-2">
            <StatusDot status={agent.status} />
            <span className="text-xs font-mono text-gray-400 uppercase tracking-wide min-w-[80px]">
              {agent.agentType}
            </span>
            <span className="text-sm text-gray-300 flex-1 truncate">
              {agent.description}
            </span>
            {/* Error indicator */}
            {agent.status === 'error' && (
              <span className="text-xs text-red-400 font-medium">ERROR</span>
            )}
          </div>

          {/* Running: progress bar */}
          {agent.status === 'running' && (
            <div className="pl-4">
              <ProgressBar />
            </div>
          )}

          {/* Complete/Error: metrics row */}
          {agent.status !== 'running' && (
            <div className="flex items-center gap-4 pl-4 text-xs font-mono text-gray-500 dark:text-gray-400">
              {agent.costUsd != null && (
                <span>{formatCost(agent.costUsd)}</span>
              )}
              {agent.durationMs != null && (
                <span>{formatDuration(agent.durationMs)}</span>
              )}
              {agent.toolUseCount != null && (
                <span>{agent.toolUseCount} tool call{agent.toolUseCount !== 1 ? 's' : ''}</span>
              )}
              {agent.agentId && (
                <span className="text-gray-600 dark:text-gray-500">id:{agent.agentId}</span>
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
