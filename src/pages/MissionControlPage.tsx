import { useLiveSessions } from '../hooks/use-live-sessions'
import { SessionCard } from '../components/live/SessionCard'
import type { LiveSummary } from '../hooks/use-live-sessions'

export function MissionControlPage() {
  const { sessions, summary, isConnected, lastUpdate } = useLiveSessions()

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-7xl mx-auto space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Mission Control
          </h1>
          <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
            <span
              className={`inline-block h-2 w-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`}
            />
            {isConnected ? 'Live' : 'Reconnecting...'}
            {lastUpdate && (
              <span className="ml-2">
                Updated {formatRelativeTime(lastUpdate)}
              </span>
            )}
          </div>
        </div>

        {/* Summary bar */}
        <SummaryBar summary={summary} />

        {/* Session grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {sessions.map(session => (
            <SessionCard key={session.id} session={session} />
          ))}
        </div>

        {/* Empty state */}
        {sessions.length === 0 && isConnected && (
          <div className="text-center text-gray-400 dark:text-gray-500 py-16">
            <div className="text-4xl mb-4">~</div>
            <div className="text-sm">
              No active Claude Code sessions detected.
            </div>
            <div className="text-xs mt-1">
              Start a session in your terminal and it will appear here.
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function SummaryBar({ summary }: { summary: LiveSummary | null }) {
  if (!summary) return null

  return (
    <div className="flex flex-wrap gap-x-6 gap-y-2 p-3 rounded-lg bg-gray-100/50 dark:bg-gray-800/50 text-sm">
      <div>
        <span className="text-green-500 font-medium">{summary.activeCount}</span>
        <span className="text-gray-500 dark:text-gray-400 ml-1">active</span>
      </div>
      <div>
        <span className="text-amber-500 font-medium">{summary.waitingCount}</span>
        <span className="text-gray-500 dark:text-gray-400 ml-1">waiting</span>
      </div>
      <div>
        <span className="text-gray-400 font-medium">{summary.idleCount}</span>
        <span className="text-gray-500 dark:text-gray-400 ml-1">idle</span>
      </div>
      <div className="ml-auto flex gap-4">
        <span className="text-gray-600 dark:text-gray-300 font-mono tabular-nums">
          ${summary.totalCostTodayUsd.toFixed(2)}
          <span className="text-gray-400 dark:text-gray-500 font-sans ml-1">today</span>
        </span>
        <span className="text-gray-600 dark:text-gray-300 font-mono tabular-nums">
          {formatTokenCount(summary.totalTokensToday)}
          <span className="text-gray-400 dark:text-gray-500 font-sans ml-1">tokens</span>
        </span>
      </div>
    </div>
  )
}

function formatTokenCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`
  return String(n)
}

function formatRelativeTime(date: Date): string {
  const diff = (Date.now() - date.getTime()) / 1000
  if (diff < 5) return 'just now'
  if (diff < 60) return `${Math.floor(diff)}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  return `${Math.floor(diff / 3600)}h ago`
}
