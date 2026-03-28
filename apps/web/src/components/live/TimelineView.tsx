import * as Tooltip from '@radix-ui/react-tooltip'
import { useEffect, useMemo, useState } from 'react'
import { formatCostUsd } from '../../lib/format-utils'
import { cn } from '../../lib/utils'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface TimelineViewProps {
  subAgents: SubAgentInfo[]
  /** Session start time (unix seconds) for calculating offsets */
  sessionStartedAt: number
  /** Total session duration for scaling the time axis */
  sessionDurationMs: number
}

/* ── Helpers ─────────────────────────────────── */

function formatDurationSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)}s`
}

/** Color by agent type; error/running override type color */
function getBarColor(agentType: string, status: string): string {
  if (status === 'error') return 'bg-red-500 dark:bg-red-400'
  if (status === 'running') return 'bg-emerald-500 dark:bg-emerald-400'

  const t = agentType.toLowerCase()
  if (t === 'explore') return 'bg-blue-500 dark:bg-blue-400'
  if (t === 'plan') return 'bg-amber-500 dark:bg-amber-400'
  if (t.includes('review')) return 'bg-violet-500 dark:bg-violet-400'
  if (t.includes('search')) return 'bg-sky-500 dark:bg-sky-400'
  if (t.includes('super')) return 'bg-indigo-500 dark:bg-indigo-400'
  return 'bg-emerald-500 dark:bg-emerald-400'
}

/** Adaptive time-axis intervals, capped to ≤10 labels */
function calculateTimeIntervals(durationMs: number): number[] {
  const durationSec = durationMs / 1000

  let intervalSec: number
  if (durationSec <= 30) intervalSec = 5
  else if (durationSec <= 60) intervalSec = 10
  else if (durationSec <= 300) intervalSec = 30
  else if (durationSec <= 600) intervalSec = 60
  else if (durationSec <= 1800) intervalSec = 300
  else if (durationSec <= 3600) intervalSec = 600
  else if (durationSec <= 7200) intervalSec = 900
  else intervalSec = 1800

  const intervals: number[] = []
  for (let t = 0; t <= durationSec; t += intervalSec) {
    intervals.push(t)
  }
  if (intervals.length > 0 && intervals[intervals.length - 1] < durationSec) {
    intervals.push(durationSec)
  }

  // Thin to ≤10 labels — always keep first & last
  if (intervals.length > 10) {
    const step = Math.ceil(intervals.length / 8)
    return intervals.filter((_, i) => i === 0 || i === intervals.length - 1 || i % step === 0)
  }
  return intervals
}

function formatTimeLabel(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)}s`
  const minutes = Math.floor(seconds / 60)
  const rem = Math.round(seconds % 60)
  if (minutes >= 60) {
    const h = Math.floor(minutes / 60)
    const m = minutes % 60
    return m > 0 ? `${h}h${m}m` : `${h}h`
  }
  return rem > 0 ? `${minutes}m${rem}s` : `${minutes}m`
}

/* ── Component ───────────────────────────────── */

/**
 * TimelineView — Gantt-like timeline showing when sub-agents ran.
 *
 * Each agent is a coloured bar positioned by its start offset,
 * sized by duration. Colour encodes agent type; tooltip shows
 * description, cost, and tool-use count.
 */
export function TimelineView({
  subAgents,
  sessionStartedAt,
  sessionDurationMs,
}: TimelineViewProps) {
  const timeIntervals = useMemo(
    () => calculateTimeIntervals(sessionDurationMs),
    [sessionDurationMs],
  )

  const sortedAgents = useMemo(
    () => [...subAgents].filter((a) => a.startedAt > 0).sort((a, b) => a.startedAt - b.startedAt),
    [subAgents],
  )

  // Tick every 2 s while any agent is still running
  const [, setTick] = useState(0)
  useEffect(() => {
    const hasRunning = sortedAgents.some((a) => a.status === 'running')
    if (!hasRunning) return
    const id = setInterval(() => setTick((p) => p + 1), 2000)
    return () => clearInterval(id)
  }, [sortedAgents])

  if (subAgents.length === 0) return null

  const durationSec = sessionDurationMs / 1000

  return (
    <div className="flex flex-col gap-1.5 overflow-hidden">
      {/* ── Time axis (aligned with bar tracks via matching spacer) ── */}
      <div className="flex items-end gap-2">
        {/* Spacer matching the label column width */}
        <div className="w-24 shrink-0" />

        <div className="relative flex-1 h-5">
          {timeIntervals.map((timeSec, i) => {
            const pct = (timeSec / durationSec) * 100
            const isFirst = i === 0
            const isLast = i === timeIntervals.length - 1
            return (
              <div
                key={timeSec}
                className="absolute bottom-0 flex flex-col items-center"
                style={{ left: `${pct}%` }}
              >
                <div className="w-px h-1.5 bg-gray-300 dark:bg-gray-600" />
                <span
                  className="font-mono text-[10px] leading-none mt-0.5 whitespace-nowrap text-gray-400 dark:text-gray-500"
                  style={{
                    transform: isLast ? 'translateX(-100%)' : isFirst ? 'none' : 'translateX(-50%)',
                  }}
                >
                  {formatTimeLabel(timeSec)}
                </span>
              </div>
            )
          })}
          {/* Baseline */}
          <div className="absolute bottom-2.75 left-0 right-0 h-px bg-gray-200 dark:bg-gray-700/50" />
        </div>
      </div>

      {/* ── Agent rows ── */}
      <div className="flex flex-col gap-0.5">
        {sortedAgents.map((agent) => {
          const startOffsetMs = (agent.startedAt - sessionStartedAt) * 1000
          const startPct = Math.max(0, (startOffsetMs / sessionDurationMs) * 100)

          let widthPct: number
          let isRunning = false

          if (agent.status === 'running') {
            const elapsedMs = Date.now() - agent.startedAt * 1000
            widthPct = Math.min(100 - startPct, (elapsedMs / sessionDurationMs) * 100)
            isRunning = true
          } else if (agent.durationMs != null) {
            widthPct = Math.min(100 - startPct, (agent.durationMs / sessionDurationMs) * 100)
          } else {
            widthPct = 1
          }

          const barColor = getBarColor(agent.agentType, agent.status)
          const label = agent.description || agent.agentType

          const tooltipContent = (
            <div className="flex flex-col gap-1 text-xs">
              <div className="font-semibold text-gray-900 dark:text-white">{agent.agentType}</div>
              <div className="text-gray-600 dark:text-gray-300">{agent.description}</div>
              <div className="flex gap-3 text-gray-500 dark:text-gray-400 mt-1">
                {agent.durationMs != null && <span>{formatDurationSeconds(agent.durationMs)}</span>}
                {agent.costUsd != null && <span>{formatCostUsd(agent.costUsd)}</span>}
                {agent.toolUseCount != null && <span>{agent.toolUseCount} tools</span>}
              </div>
              {agent.status === 'running' && (
                <div className="text-emerald-600 dark:text-emerald-400 mt-1">Running…</div>
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
                    className="flex items-center gap-2 h-5"
                    tabIndex={0}
                    role="button"
                    aria-label={`${agent.agentType}: ${agent.description}`}
                  >
                    {/* Label — description is far more useful than generic "Agent" */}
                    <span className="text-[11px] text-gray-500 dark:text-gray-400 w-24 truncate shrink-0 text-right">
                      {label}
                    </span>

                    {/* Track + bar */}
                    <div className="relative flex-1 h-3 bg-gray-100/60 dark:bg-gray-800/30 rounded-sm">
                      <div
                        className={cn(
                          'absolute h-full rounded-sm',
                          barColor,
                          isRunning && 'timeline-bar-growing',
                        )}
                        style={{
                          left: `${startPct}%`,
                          width: `max(3px, ${widthPct}%)`,
                        }}
                      />
                    </div>
                  </div>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-md px-3 py-2 shadow-lg z-50 max-w-sm"
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
