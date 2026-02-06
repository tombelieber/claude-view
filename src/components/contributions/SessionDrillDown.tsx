import { X, Clock, MessageSquare, FileCode2, GitCommit, ArrowLeft, ExternalLink } from 'lucide-react'
import { cn } from '../../lib/utils'
import { InsightLine } from './InsightLine'
import { useSessionContribution } from '../../hooks/use-contributions'
import type { FileImpact, LinkedCommit } from '../../types/generated'

interface SessionDrillDownProps {
  sessionId: string
  branchName?: string
  onClose: () => void
  onOpenFullSession?: (sessionId: string) => void
}

/**
 * SessionDrillDown displays detailed contribution info for a single session.
 *
 * Shows:
 * - Work type classification
 * - Duration, prompts, lines, commits summary
 * - Files impacted with line changes
 * - Linked commits
 * - Effectiveness metrics (commit rate, re-edit rate)
 */
export function SessionDrillDown({
  sessionId,
  branchName,
  onClose,
  onOpenFullSession,
}: SessionDrillDownProps) {
  const { data, isLoading, error } = useSessionContribution(sessionId)

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 shadow-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
        <button
          onClick={onClose}
          className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 transition-colors cursor-pointer"
          aria-label="Go back"
        >
          <ArrowLeft className="w-4 h-4" aria-hidden="true" />
          {branchName || 'Back'}
        </button>
        <button
          onClick={onClose}
          className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-pointer"
          aria-label="Close drill-down"
        >
          <X className="w-5 h-5" aria-hidden="true" />
        </button>
      </div>

      {/* Content */}
      <div className="p-6">
        {isLoading && (
          <div className="flex items-center justify-center py-12">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500" />
          </div>
        )}

        {error && (
          <div className="text-center py-12">
            <p className="text-red-500 dark:text-red-400 mb-2">Failed to load session</p>
            <p className="text-sm text-gray-500">{error.message}</p>
          </div>
        )}

        {data && (
          <div className="space-y-6">
            {/* Work Type & Preview */}
            <div>
              {data.workType && (
                <span className="inline-block px-2 py-1 text-xs font-medium rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 mb-2">
                  {data.workType}
                </span>
              )}
              <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
                Session Details
              </h2>
            </div>

            {/* Summary Stats Grid */}
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
              <StatBox
                icon={<Clock className="w-4 h-4 text-gray-400" />}
                label="Duration"
                value={formatDuration(data.duration)}
              />
              <StatBox
                icon={<MessageSquare className="w-4 h-4 text-gray-400" />}
                label="Prompts"
                value={data.promptCount.toString()}
              />
              <StatBox
                icon={<FileCode2 className="w-4 h-4 text-gray-400" />}
                label="AI Lines"
                value={
                  <span>
                    <span className="text-green-600 dark:text-green-400">+{formatNumber(data.aiLinesAdded)}</span>
                    {' / '}
                    <span className="text-red-500 dark:text-red-400">-{formatNumber(data.aiLinesRemoved)}</span>
                  </span>
                }
              />
              <StatBox
                icon={<GitCommit className="w-4 h-4 text-gray-400" />}
                label="Commits"
                value={data.commits.length.toString()}
              />
            </div>

            {/* Files Impacted */}
            {data.files.length > 0 && (
              <div>
                <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
                  Files Impacted
                </h3>
                <div className="space-y-2 max-h-48 overflow-y-auto">
                  {data.files.map((file) => (
                    <FileImpactRow key={file.path} file={file} />
                  ))}
                </div>
              </div>
            )}

            {/* Linked Commits */}
            {data.commits.length > 0 && (
              <div>
                <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
                  Linked Commits
                </h3>
                <div className="space-y-2">
                  {data.commits.map((commit) => (
                    <CommitRow key={commit.hash} commit={commit} />
                  ))}
                </div>
              </div>
            )}

            {/* Effectiveness */}
            <div>
              <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
                Effectiveness
              </h3>
              <div className="space-y-3">
                {/* Commit Rate Bar */}
                {data.commitRate !== null && (
                  <div>
                    <div className="flex items-center gap-2 mb-1">
                      <div className="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                        <div
                          className="h-full bg-green-500 rounded-full"
                          style={{ width: `${data.commitRate * 100}%` }}
                          role="progressbar"
                          aria-valuenow={Math.round(data.commitRate * 100)}
                          aria-valuemin={0}
                          aria-valuemax={100}
                        />
                      </div>
                      <span className="text-sm font-medium text-gray-700 dark:text-gray-300 tabular-nums w-16 text-right">
                        {(data.commitRate * 100).toFixed(0)}% committed
                      </span>
                    </div>
                  </div>
                )}

                {/* Re-edit Rate */}
                <p className="text-sm text-gray-600 dark:text-gray-400">
                  Re-edit rate:{' '}
                  <span className="font-medium text-gray-900 dark:text-gray-100">
                    {data.reeditRate !== null ? data.reeditRate.toFixed(2) : '--'}
                  </span>
                  {data.reeditRate !== null && data.reeditRate < 0.2 && (
                    <span className="text-green-600 dark:text-green-400 ml-1">
                      (low — good first-attempt accuracy)
                    </span>
                  )}
                </p>
              </div>
            </div>

            {/* Insight */}
            <InsightLine insight={data.insight} />

            {/* Open Full Session */}
            {onOpenFullSession && (
              <div className="pt-4 border-t border-gray-200 dark:border-gray-700">
                <button
                  onClick={() => onOpenFullSession(sessionId)}
                  className={cn(
                    'flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg',
                    'text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30',
                    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
                    'transition-colors cursor-pointer'
                  )}
                >
                  Open Full Session
                  <ExternalLink className="w-4 h-4" aria-hidden="true" />
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

/**
 * Stat box for summary metrics.
 */
function StatBox({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode
  label: string
  value: React.ReactNode
}) {
  return (
    <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-3">
      <div className="flex items-center gap-1.5 mb-1">
        {icon}
        <span className="text-xs text-gray-500 dark:text-gray-400">{label}</span>
      </div>
      <div className="text-lg font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
        {value}
      </div>
    </div>
  )
}

/**
 * File impact row with progress bar.
 */
function FileImpactRow({ file }: { file: FileImpact }) {
  const totalLines = file.linesAdded + file.linesRemoved
  const addedPercent = totalLines > 0 ? (file.linesAdded / totalLines) * 100 : 50

  return (
    <div className="flex items-center gap-3 py-2 px-3 bg-gray-50 dark:bg-gray-800 rounded-lg">
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
          {file.path}
        </p>
        <div className="flex items-center gap-2 mt-1">
          <span className="text-xs text-green-600 dark:text-green-400 tabular-nums">
            +{formatNumber(file.linesAdded)}
          </span>
          <span className="text-xs text-red-500 dark:text-red-400 tabular-nums">
            -{formatNumber(file.linesRemoved)}
          </span>
        </div>
      </div>
      <div className="w-24 flex items-center gap-2">
        <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden flex">
          <div
            className="h-full bg-green-500"
            style={{ width: `${addedPercent}%` }}
          />
          <div
            className="h-full bg-red-500"
            style={{ width: `${100 - addedPercent}%` }}
          />
        </div>
        <span className="text-xs text-gray-500 dark:text-gray-400 w-12 text-right">
          {file.action}
        </span>
      </div>
    </div>
  )
}

/**
 * Commit row with hash and message.
 */
function CommitRow({ commit }: { commit: LinkedCommit }) {
  return (
    <div className="flex items-center gap-3 py-2 px-3 bg-gray-50 dark:bg-gray-800 rounded-lg">
      <GitCommit className="w-4 h-4 text-gray-400 flex-shrink-0" aria-hidden="true" />
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <code className="text-xs font-mono text-gray-500 dark:text-gray-400">
            {commit.hash.slice(0, 7)}
          </code>
          <span className="text-sm text-gray-900 dark:text-gray-100 truncate">
            {commit.message}
          </span>
        </div>
      </div>
      {(commit.insertions !== null || commit.deletions !== null) && (
        <div className="text-xs tabular-nums flex-shrink-0">
          {commit.insertions !== null && (
            <span className="text-green-600 dark:text-green-400">+{commit.insertions}</span>
          )}
          {commit.deletions !== null && (
            <span className="text-red-500 dark:text-red-400 ml-1">-{commit.deletions}</span>
          )}
        </div>
      )}
    </div>
  )
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
