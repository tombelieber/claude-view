import { useState } from 'react'
import { GitCommit, Copy, Check, Clock } from 'lucide-react'
import { cn } from '../lib/utils'
import { truncateMessage, formatRelativeTime } from '../lib/format-utils'
import { TierBadge } from './TierBadge'
import type { CommitWithTier } from '../types/generated'

export interface CommitsPanelProps {
  /** List of commits to display */
  commits: CommitWithTier[]
  /** Optional className for additional styling */
  className?: string
}

interface CommitRowProps {
  commit: CommitWithTier
}

function CommitRow({ commit }: CommitRowProps) {
  const [copied, setCopied] = useState(false)

  const handleCopyHash = async () => {
    try {
      await navigator.clipboard.writeText(commit.hash)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Clipboard API not available, ignore
    }
  }

  return (
    <div className="flex items-start gap-3 p-2 -mx-2 rounded-lg hover:bg-gray-50 transition-colors group">
      {/* Hash with copy button */}
      <button
        onClick={handleCopyHash}
        className={cn(
          'flex items-center gap-1 text-xs font-mono px-1.5 py-0.5 rounded transition-colors flex-shrink-0',
          copied
            ? 'bg-green-100 text-green-700'
            : 'bg-gray-100 text-gray-500 hover:bg-gray-200 hover:text-gray-700'
        )}
        title={copied ? 'Copied!' : 'Click to copy full hash'}
      >
        {commit.hash.slice(0, 7)}
        {copied ? (
          <Check className="w-3 h-3" />
        ) : (
          <Copy className="w-3 h-3 opacity-0 group-hover:opacity-100 transition-opacity" />
        )}
      </button>

      {/* Message */}
      <div className="flex-1 min-w-0">
        <p className="text-sm text-gray-900 truncate" title={commit.message}>
          {truncateMessage(commit.message, 50)}
        </p>
        {commit.branch && (
          <p className="text-xs text-gray-400 truncate">
            on {commit.branch}
          </p>
        )}
      </div>

      {/* Tier badge and time */}
      <div className="flex items-center gap-2 flex-shrink-0">
        <TierBadge tier={commit.tier} />
        <span className="flex items-center gap-1 text-xs text-gray-400">
          <Clock className="w-3 h-3" />
          {formatRelativeTime(commit.timestamp)}
        </span>
      </div>
    </div>
  )
}

/**
 * CommitsPanel displays a list of commits linked to a session.
 *
 * Features:
 * - Shows hash (clickable to copy), message (truncated), tier badge, timestamp
 * - Click hash to copy full commit hash to clipboard
 * - Tier badges: T1 (high confidence - commit skill), T2 (medium - during session)
 * - Empty state handled gracefully
 */
export function CommitsPanel({ commits, className }: CommitsPanelProps) {
  if (commits.length === 0) {
    return (
      <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
        <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
          <GitCommit className="w-4 h-4" />
          Linked Commits
        </h2>
        <div className="flex flex-col items-center justify-center py-6 text-gray-400">
          <GitCommit className="w-8 h-8 mb-2 opacity-50" />
          <p className="text-sm">No commits linked</p>
          <p className="text-xs mt-1">Commits made during this session will appear here</p>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('bg-white rounded-xl border border-gray-200 p-6', className)}>
      <h2 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 flex items-center gap-1.5 font-metric-label">
        <GitCommit className="w-4 h-4" />
        Linked Commits
        <span className="ml-auto text-gray-400 normal-case font-normal">
          {commits.length} {commits.length === 1 ? 'commit' : 'commits'}
        </span>
      </h2>
      <div className="space-y-1">
        {commits.map((commit) => (
          <CommitRow key={commit.hash} commit={commit} />
        ))}
      </div>
    </div>
  )
}
