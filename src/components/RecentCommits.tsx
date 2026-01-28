import { GitCommit, Clock } from 'lucide-react'
import { cn } from '../lib/utils'
import type { CommitWithTier } from '../types/generated'

export interface RecentCommitsProps {
  /** List of commits to display (max 5 shown) */
  commits: CommitWithTier[]
  /** Optional className for additional styling */
  className?: string
}

/** Truncate commit message to specified length */
function truncateMessage(message: string, maxLength: number = 60): string {
  const firstLine = message.split('\n')[0]
  if (firstLine.length <= maxLength) return firstLine
  return firstLine.slice(0, maxLength - 3) + '...'
}

/** Format timestamp as relative time */
function formatRelativeTime(timestamp: bigint): string {
  const date = new Date(Number(timestamp) * 1000)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 7) return `${diffDays}d ago`
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** Tier badge component */
function TierBadge({ tier }: { tier: number }) {
  return (
    <span
      className={cn(
        'inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded',
        tier === 1
          ? 'bg-blue-100 text-blue-700'
          : 'bg-gray-100 text-gray-600'
      )}
      title={tier === 1 ? 'High confidence (commit skill)' : 'Medium confidence (during session)'}
    >
      T{tier}
    </span>
  )
}

/**
 * RecentCommits displays the last 5 linked commits.
 *
 * Each commit shows:
 * - Short hash (7 chars)
 * - Message (truncated)
 * - Tier badge (1 or 2)
 * - Relative timestamp
 *
 * Empty state handled gracefully.
 */
export function RecentCommits({ commits, className }: RecentCommitsProps) {
  const displayCommits = commits.slice(0, 5)

  if (displayCommits.length === 0) {
    return (
      <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
          <GitCommit className="w-4 h-4" />
          Recent Commits
        </h2>
        <div className="flex flex-col items-center justify-center py-8 text-gray-400">
          <GitCommit className="w-8 h-8 mb-2 opacity-50" />
          <p className="text-sm">No commits linked yet</p>
          <p className="text-xs mt-1">Commits made during sessions will appear here</p>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
      <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
        <GitCommit className="w-4 h-4" />
        Recent Commits
      </h2>
      <div className="space-y-3">
        {displayCommits.map((commit) => (
          <div
            key={commit.hash}
            className="flex items-start gap-3 p-2 -mx-2 rounded-lg hover:bg-gray-50 transition-colors"
          >
            <code className="text-xs font-mono text-gray-500 bg-gray-100 px-1.5 py-0.5 rounded flex-shrink-0">
              {commit.hash.slice(0, 7)}
            </code>
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-900 truncate">
                {truncateMessage(commit.message)}
              </p>
            </div>
            <div className="flex items-center gap-2 flex-shrink-0">
              <TierBadge tier={commit.tier} />
              <span className="flex items-center gap-1 text-xs text-gray-400">
                <Clock className="w-3 h-3" />
                {formatRelativeTime(commit.timestamp)}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
