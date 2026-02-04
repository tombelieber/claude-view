import { GitBranch, Clock, ChevronRight, FileCode2, GitCommit } from 'lucide-react'
import { cn } from '../../lib/utils'
import { useBranchSessions, type TimeRange } from '../../hooks/use-contributions'
import type { BranchBreakdown, BranchSession } from '../../types/generated'

interface BranchCardProps {
  branch: BranchBreakdown
  isExpanded: boolean
  onToggle: () => void
  onDrillDown?: (sessionId: string) => void
  timeRange?: TimeRange
}

/**
 * BranchCard displays a single branch with contribution stats.
 *
 * Shows sessions, lines, commits, AI share with progress bar,
 * and an insight about the work pattern.
 *
 * When expanded, fetches and displays sessions for the branch.
 */
export function BranchCard({
  branch,
  isExpanded,
  onToggle,
  onDrillDown,
  timeRange = 'week',
}: BranchCardProps) {
  const {
    branch: branchName,
    sessionsCount,
    linesAdded,
    linesRemoved,
    commitsCount,
    aiShare,
    lastActivity,
  } = branch

  // Fetch sessions when expanded
  const { data, isLoading } = useBranchSessions(branchName, timeRange, isExpanded)

  const aiSharePercent = aiShare !== null ? Math.round(aiShare * 100) : null
  const lastActivityText = lastActivity !== null ? formatRelativeTime(Number(lastActivity)) : null

  // Generate insight based on AI share and commit rate
  const insight = generateBranchInsight(aiShare, Number(sessionsCount), Number(commitsCount))

  return (
    <div
      className={cn(
        'border rounded-lg transition-all',
        isExpanded
          ? 'border-blue-300 dark:border-blue-700 bg-blue-50/50 dark:bg-blue-900/10'
          : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900'
      )}
    >
      <button
        onClick={onToggle}
        className="w-full text-left p-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-inset rounded-t-lg cursor-pointer"
        aria-expanded={isExpanded}
      >
        {/* Branch Name */}
        <div className="flex items-center gap-2 mb-2">
          <GitBranch className="w-4 h-4 text-gray-500 dark:text-gray-400" aria-hidden="true" />
          <span className="font-medium text-gray-900 dark:text-gray-100 truncate">
            {branchName}
          </span>
          <ChevronRight
            className={cn(
              'w-4 h-4 text-gray-400 transition-transform ml-auto',
              isExpanded && 'rotate-90'
            )}
            aria-hidden="true"
          />
        </div>

        {/* Stats Row */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-gray-600 dark:text-gray-400 mb-3">
          <span>{Number(sessionsCount)} sessions</span>
          <span className="text-gray-300 dark:text-gray-600">&bull;</span>
          <span className="tabular-nums">
            <span className="text-green-600 dark:text-green-400">+{formatNumber(Number(linesAdded))}</span>
            {' / '}
            <span className="text-red-500 dark:text-red-400">-{formatNumber(Number(linesRemoved))}</span>
            {' lines'}
          </span>
          <span className="text-gray-300 dark:text-gray-600">&bull;</span>
          <span>{Number(commitsCount)} commits</span>
          {lastActivityText && (
            <>
              <span className="text-gray-300 dark:text-gray-600">&bull;</span>
              <span className="flex items-center gap-1">
                <Clock className="w-3 h-3" aria-hidden="true" />
                Last: {lastActivityText}
              </span>
            </>
          )}
        </div>

        {/* AI Share Progress Bar */}
        {aiSharePercent !== null && (
          <div className="mb-3">
            <div className="flex items-center gap-2">
              <div className="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-blue-500 rounded-full transition-all"
                  style={{ width: `${aiSharePercent}%` }}
                  role="progressbar"
                  aria-valuenow={aiSharePercent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-label={`${aiSharePercent}% AI share`}
                />
              </div>
              <span className="text-xs font-medium text-gray-600 dark:text-gray-400 tabular-nums w-12 text-right">
                {aiSharePercent}% AI
              </span>
            </div>
          </div>
        )}

        {/* Insight */}
        {insight && (
          <p className="text-xs text-gray-500 dark:text-gray-400 italic">
            {insight}
          </p>
        )}
      </button>

      {/* Expanded Sessions List */}
      {isExpanded && (
        <div className="border-t border-gray-200 dark:border-gray-700 px-4 py-3">
          {isLoading ? (
            <div className="flex items-center justify-center py-4">
              <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-blue-500" />
            </div>
          ) : data?.sessions && data.sessions.length > 0 ? (
            <div className="space-y-2">
              <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2">
                Sessions
              </p>
              {data.sessions.map((session) => (
                <SessionRow
                  key={session.sessionId}
                  session={session}
                  onClick={() => onDrillDown?.(session.sessionId)}
                />
              ))}
            </div>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400 py-2">
              No sessions found for this branch.
            </p>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * Session row for the expanded branch view.
 */
function SessionRow({
  session,
  onClick,
}: {
  session: BranchSession
  onClick?: () => void
}) {
  const linesTotal = Number(session.aiLinesAdded) + Number(session.aiLinesRemoved)
  const commitCount = Number(session.commitCount)
  const durationText = formatDuration(Number(session.durationSeconds))
  const timeText = formatRelativeTime(Number(session.lastMessageAt))

  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-3 rounded-lg transition-colors cursor-pointer',
        'bg-gray-50 dark:bg-gray-800 hover:bg-gray-100 dark:hover:bg-gray-700',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-inset'
      )}
    >
      <div className="flex items-center justify-between mb-1">
        <div className="flex items-center gap-2 min-w-0">
          {session.workType && (
            <span className="inline-block px-1.5 py-0.5 text-xs font-medium rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 flex-shrink-0">
              {session.workType}
            </span>
          )}
          <span className="text-xs text-gray-500 dark:text-gray-400 truncate">
            {timeText}
          </span>
        </div>
        <ChevronRight
          className="w-4 h-4 text-gray-400 flex-shrink-0"
          aria-hidden="true"
        />
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-gray-600 dark:text-gray-400">
        <span className="flex items-center gap-1">
          <Clock className="w-3 h-3" aria-hidden="true" />
          {durationText}
        </span>
        <span className="flex items-center gap-1">
          <FileCode2 className="w-3 h-3" aria-hidden="true" />
          <span className="text-green-600 dark:text-green-400">+{formatNumber(Number(session.aiLinesAdded))}</span>
          {' / '}
          <span className="text-red-500 dark:text-red-400">-{formatNumber(Number(session.aiLinesRemoved))}</span>
        </span>
        {commitCount > 0 && (
          <span className="flex items-center gap-1">
            <GitCommit className="w-3 h-3" aria-hidden="true" />
            {commitCount} commit{commitCount !== 1 ? 's' : ''}
          </span>
        )}
        {linesTotal > 0 && commitCount === 0 && (
          <span className="text-amber-600 dark:text-amber-400">(uncommitted)</span>
        )}
      </div>
    </button>
  )
}

/**
 * Generate insight based on AI share and activity.
 */
function generateBranchInsight(
  aiShare: number | null,
  sessions: number,
  commits: number
): string | null {
  if (aiShare === null) return null

  const sharePercent = aiShare * 100
  const commitRate = sessions > 0 ? commits / sessions : 0

  if (sharePercent >= 70 && commitRate >= 0.5) {
    return 'High AI share + high commit rate \u2014 AI doing heavy lifting here'
  }
  if (sharePercent >= 70 && commitRate < 0.5) {
    return 'High AI share but low commit rate \u2014 may need review before committing'
  }
  if (sharePercent < 50 && sharePercent > 0) {
    return 'Lower AI share \u2014 likely more manual investigation/debugging'
  }
  if (sharePercent >= 50) {
    return 'Balanced AI assistance on this branch'
  }
  return null
}

/**
 * Format relative time from Unix timestamp.
 */
function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000
  const diff = now - timestamp

  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  return `${Math.floor(diff / 604800)}w ago`
}

/**
 * Format duration in seconds to human-readable string.
 */
function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min`
  const hours = Math.floor(seconds / 3600)
  const mins = Math.floor((seconds % 3600) / 60)
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`
}

/**
 * Format large numbers with K/M suffixes.
 */
function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toLocaleString()
}
