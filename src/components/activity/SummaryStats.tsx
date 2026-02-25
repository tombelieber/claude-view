import { Clock, Hash, TrendingUp, CalendarDays, Wrench, Bot, Plug, Sparkles } from 'lucide-react'
import { formatHumanDuration } from '../../lib/format-utils'
import type { ActivitySummary } from '../../lib/activity-utils'

/** Format YYYY-MM-DD as readable day name */
function formatDayName(dateStr: string): string {
  const date = new Date(dateStr + 'T12:00:00') // Noon to avoid TZ issues
  return date.toLocaleDateString('en-US', { weekday: 'long' })
}

/** Format large counts with K/M suffixes */
function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

interface SummaryStatsProps {
  summary: ActivitySummary
  label: string // e.g. "This Week", "Today"
}

// V2 deferred: Week-over-week comparison card (design doc Section 1).
// useTimeRange already exposes comparisonLabel; needs a second parallel query
// for the previous period and delta computation. Not included in V1.
export function SummaryStats({ summary, label }: SummaryStatsProps) {
  if (summary.sessionCount === 0) {
    return (
      <div className="text-center py-8">
        <p className="text-sm text-gray-500 dark:text-gray-400">No activity for {label.toLowerCase()}</p>
        <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">Start a Claude Code session and it will show up here</p>
      </div>
    )
  }

  const cards = [
    {
      icon: Clock,
      label: 'Total Time',
      value: formatHumanDuration(summary.totalSeconds),
      color: 'text-blue-400 dark:text-blue-500',
    },
    {
      icon: Hash,
      label: 'Sessions',
      value: String(summary.sessionCount),
      color: 'text-violet-400 dark:text-violet-500',
    },
    {
      icon: TrendingUp,
      label: 'Avg Session',
      value: formatHumanDuration(summary.avgSessionSeconds),
      color: 'text-emerald-400 dark:text-emerald-500',
    },
    {
      icon: CalendarDays,
      label: 'Busiest Day',
      value: summary.busiestDay ? formatDayName(summary.busiestDay.date) : '--',
      color: 'text-amber-400 dark:text-amber-500',
    },
    {
      icon: Wrench,
      label: 'Tool Calls',
      value: formatCount(summary.totalToolCalls),
      color: 'text-cyan-400 dark:text-cyan-500',
    },
    {
      icon: Bot,
      label: 'Agents',
      value: formatCount(summary.totalAgentSpawns),
      color: 'text-rose-400 dark:text-rose-500',
    },
    {
      icon: Plug,
      label: 'MCP Calls',
      value: formatCount(summary.totalMcpCalls),
      color: 'text-orange-400 dark:text-orange-500',
    },
    {
      icon: Sparkles,
      label: 'Skills',
      value: String(summary.uniqueSkills),
      color: 'text-indigo-400 dark:text-indigo-500',
    },
  ]

  return (
    <div>
      <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-200 mb-3">{label}</h2>
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3" role="list" aria-label="Activity summary">
        {cards.map((card) => (
          <div
            key={card.label}
            className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-lg px-4 py-3"
          >
            <div className="flex items-center gap-2 mb-1">
              <card.icon className={`w-4 h-4 ${card.color}`} />
              <span className="text-xs text-gray-500 dark:text-gray-400">{card.label}</span>
            </div>
            <div className="text-xl font-semibold text-gray-900 dark:text-gray-100">{card.value}</div>
          </div>
        ))}
      </div>
      {summary.longestSession && (
        <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
          Longest: {formatHumanDuration(summary.longestSession.seconds)} on {summary.longestSession.project}
        </p>
      )}
    </div>
  )
}
