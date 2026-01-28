import { GitCommit, Clock } from 'lucide-react'
import { cn } from '../lib/utils'
import { truncateMessage, formatRelativeTime } from '../lib/format-utils'
import { TierBadge } from './TierBadge'
import type { CommitWithTier } from '../types/generated'

export interface RecentCommitsProps {
  /** List of commits to display (max 5 shown) */
  commits: CommitWithTier[]
  /** Optional className for additional styling */
  className?: string
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
