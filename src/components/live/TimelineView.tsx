import { useMemo, useEffect, useState } from 'react'
import * as Tooltip from '@radix-ui/react-tooltip'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { cn } from '../../lib/utils'

interface TimelineViewProps {
  subAgents: SubAgentInfo[]
  /** Session start time (unix seconds) for calculating offsets */
  sessionStartedAt: number
  /** Total session duration for scaling the time axis */
  sessionDurationMs: number
}

/** Format cost as $X.XX */
function formatCost(usd: number): string {
  return `$${usd.toFixed(2)}`
}

/** Format duration in seconds with 1 decimal (e.g., "2.1s") */
function formatDurationSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`
}

/** Calculate adaptive time axis intervals based on total duration */
function calculateTimeIntervals(durationMs: number): number[] {
  const durationSec = durationMs / 1000

  // Determine interval size based on total duration
  let intervalSec: number
  if (durationSec <= 30) {
    intervalSec = 5 // 5s intervals for <30s
  } else if (durationSec <= 60) {
    intervalSec = 10 // 10s intervals for <1m
  } else if (durationSec <= 300) {
    intervalSec = 30 // 30s intervals for <5m
  } else if (durationSec <= 600) {
    intervalSec = 60 // 1m intervals for <10m
  } else if (durationSec <= 1800) {
    intervalSec = 300 // 5m intervals for <30m
  } else {
    intervalSec = 600 // 10m intervals for >=30m
  }

  // Generate intervals from 0 to durationSec
  const intervals: number[] = []
  for (let t = 0; t <= durationSec; t += intervalSec) {
    intervals.push(t)
  }

  // Always include the end time if not already present
  if (intervals[intervals.length - 1] < durationSec) {
    intervals.push(durationSec)
  }

  return intervals
}

/** Format time label (e.g., "0s", "15s", "1m 30s", "5m") */
function formatTimeLabel(seconds: number): string {
  if (seconds < 60) {
    return `${Math.round(seconds)}s`
  }
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = Math.round(seconds % 60)
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}

/**
 * TimelineView — Gantt-like timeline showing when sub-agents ran.
 *
 * Layout:
 * ```
 * Time:  0s     5s     10s    15s    20s    25s
 *        |------|------|------|------|------|
 * Main   ███████████████████████████████████████
 * Explore       ████████████████
 * code-rev            ██████████████
 * search  ████████
 * ```
 *
 * - Horizontal time axis at top, scaled to session duration
 * - Each agent as a horizontal bar positioned by startedAt offset, width by durationMs
 * - Overlapping bars clearly show parallel execution
 * - Hover tooltip: agent type, description, duration, cost
 * - Color: green for complete, red for error
 * - Running agents show animated right edge (growing bar)
 * - Time axis labels adapt based on total duration
 * - Min bar width of 2px so very short agents are still visible
 */
export function TimelineView({
  subAgents,
  sessionStartedAt,
  sessionDurationMs,
}: TimelineViewProps) {
  // Calculate time intervals for the axis
  const timeIntervals = useMemo(
    () => calculateTimeIntervals(sessionDurationMs),
    [sessionDurationMs]
  )

  // Sort agents by startedAt (chronological order)
  const sortedAgents = useMemo(() => {
    return [...subAgents].sort((a, b) => a.startedAt - b.startedAt)
  }, [subAgents])

  // Force re-render every 2 seconds when any agent is running
  // This allows running agent bars to grow as time progresses
  const [, setTick] = useState(0)
  useEffect(() => {
    const hasRunning = sortedAgents.some((a) => a.status === 'running')
    if (!hasRunning) return

    const interval = setInterval(() => {
      setTick((prev) => prev + 1)
    }, 2000)

    return () => clearInterval(interval)
  }, [sortedAgents])

  // Early return if no sub-agents
  if (subAgents.length === 0) return null

  const durationSec = sessionDurationMs / 1000

  return (
    <div className="flex flex-col gap-3 bg-white dark:bg-gray-950 border border-gray-200 dark:border-gray-800 rounded-lg p-4 font-mono">
      {/* Time axis */}
      <div className="relative h-8 border-b border-gray-300 dark:border-gray-700">
        <div className="absolute inset-0 flex items-end">
          {timeIntervals.map((timeSec) => {
            const positionPct = (timeSec / durationSec) * 100
            return (
              <div
                key={timeSec}
                className="absolute bottom-0 flex flex-col items-center"
                style={{ left: `${positionPct}%` }}
              >
                {/* Tick mark */}
                <div className="w-px h-2 bg-gray-400 dark:bg-gray-600" />
                {/* Time label */}
                <span className="text-xs text-gray-500 mt-1 -translate-x-1/2 whitespace-nowrap">
                  {formatTimeLabel(timeSec)}
                </span>
              </div>
            )
          })}
        </div>
      </div>

      {/* Agent timeline bars */}
      <div className="flex flex-col gap-1">
        {sortedAgents.map((agent) => {
          // Calculate bar position and width as percentages
          const startOffsetMs = (agent.startedAt - sessionStartedAt) * 1000
          const startPct = Math.max(0, (startOffsetMs / sessionDurationMs) * 100)

          let widthPct: number
          let isRunning = false

          if (agent.status === 'running') {
            // Running agents: bar extends to current time (now)
            const nowMs = Date.now()
            const elapsedMs = nowMs - agent.startedAt * 1000
            widthPct = Math.min(100 - startPct, (elapsedMs / sessionDurationMs) * 100)
            isRunning = true
          } else if (agent.durationMs != null) {
            // Completed agents: use actual duration
            widthPct = Math.min(100 - startPct, (agent.durationMs / sessionDurationMs) * 100)
          } else {
            // Fallback: 1% width for visibility
            widthPct = 1
          }

          // Bar color based on status
          const barColorClass =
            agent.status === 'error'
              ? 'bg-red-500'
              : agent.status === 'running'
                ? 'bg-green-500'
                : 'bg-green-600'

          // Tooltip content
          const tooltipContent = (
            <div className="flex flex-col gap-1 text-xs">
              <div className="font-semibold text-gray-900 dark:text-white">{agent.agentType}</div>
              <div className="text-gray-600 dark:text-gray-300">{agent.description}</div>
              <div className="flex gap-3 text-gray-500 dark:text-gray-400 mt-1">
                {agent.durationMs != null && (
                  <span>{formatDurationSeconds(agent.durationMs)}</span>
                )}
                {agent.costUsd != null && <span>{formatCost(agent.costUsd)}</span>}
                {agent.toolUseCount != null && (
                  <span>{agent.toolUseCount} tool calls</span>
                )}
              </div>
              {agent.status === 'running' && (
                <div className="text-green-600 dark:text-green-400 mt-1">Running...</div>
              )}
              {agent.status === 'error' && (
                <div className="text-red-500 dark:text-red-400 mt-1">Error</div>
              )}
            </div>
          )

          return (
            <Tooltip.Provider key={agent.toolUseId} delayDuration={200}>
              <Tooltip.Root>
                <Tooltip.Trigger asChild>
                  <div
                    className="flex items-center gap-2 h-6"
                    tabIndex={0}
                    role="button"
                    aria-label={`${agent.agentType} agent: ${agent.description}`}
                  >
                    {/* Agent type label */}
                    <span className="text-xs text-gray-500 dark:text-gray-400 w-20 truncate flex-shrink-0">
                      {agent.agentType}
                    </span>

                    {/* Timeline bar container */}
                    <div className="relative flex-1 h-4 bg-gray-100 dark:bg-gray-900 rounded">
                      {/* The actual bar */}
                      <div
                        className={cn(
                          'absolute h-full rounded transition-all',
                          barColorClass,
                          isRunning && 'timeline-bar-growing'
                        )}
                        style={{
                          left: `${startPct}%`,
                          width: `max(2px, ${widthPct}%)`, // Min 2px width
                        }}
                      />
                    </div>
                  </div>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded px-3 py-2 shadow-lg z-50 max-w-sm"
                    sideOffset={5}
                  >
                    {tooltipContent}
                    <Tooltip.Arrow className="fill-gray-200 dark:fill-gray-700" />
                  </Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </Tooltip.Provider>
          )
        })}
      </div>
    </div>
  )
}
