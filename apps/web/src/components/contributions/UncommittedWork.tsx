import { AlertTriangle, RefreshCw, ExternalLink, X, Clock, FileCode2 } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatNumber } from '../../lib/format-utils'
import type { UncommittedWork as UncommittedWorkType } from '../../types/generated'

interface UncommittedWorkSectionProps {
  uncommitted: UncommittedWorkType[]
  uncommittedInsight: string
  onRefresh?: () => void
  onDismiss?: (projectId: string) => void
  onViewSession?: (sessionId: string) => void
}

/**
 * UncommittedWorkSection displays alerts for uncommitted AI work.
 *
 * Shows projects with uncommitted lines, time since last activity,
 * and helpful prompts to commit or review the work.
 */
export function UncommittedWorkSection({
  uncommitted,
  uncommittedInsight,
  onRefresh,
  onDismiss,
  onViewSession,
}: UncommittedWorkSectionProps) {
  if (uncommitted.length === 0) {
    return null // Don't render if nothing to show
  }

  const totalLines = uncommitted.reduce((sum, u) => sum + u.linesAdded, 0)
  const projectCount = uncommitted.length

  return (
    <div className="bg-amber-50 dark:bg-amber-900/20 rounded-xl border border-amber-200 dark:border-amber-800 p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-400" aria-hidden="true" />
          <h2 className="text-xs font-medium text-amber-800 dark:text-amber-300 uppercase tracking-wider">
            Uncommitted AI Work
          </h2>
        </div>
        {onRefresh && (
          <button
            onClick={onRefresh}
            className={cn(
              'flex items-center gap-1 px-2 py-1 text-xs font-medium rounded',
              'text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-800/50',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400',
              'transition-colors cursor-pointer'
            )}
            aria-label="Refresh uncommitted work"
          >
            <RefreshCw className="w-3 h-3" aria-hidden="true" />
            Refresh
          </button>
        )}
      </div>

      {/* Uncommitted Items */}
      <div className="space-y-3 mb-4">
        {uncommitted.map((item) => (
          <UncommittedItem
            key={item.projectId}
            item={item}
            onDismiss={onDismiss ? () => onDismiss(item.projectId) : undefined}
            onView={onViewSession ? () => onViewSession(item.lastSessionId) : undefined}
          />
        ))}
      </div>

      {/* Summary Insight */}
      <p className="text-sm text-amber-800 dark:text-amber-200">
        {uncommittedInsight ||
          `You have ${formatNumber(totalLines)} uncommitted AI lines across ${projectCount} project${projectCount > 1 ? 's' : ''}. Commit often to avoid losing work.`}
      </p>
    </div>
  )
}

interface UncommittedItemProps {
  item: UncommittedWorkType
  onDismiss?: () => void
  onView?: () => void
}

/**
 * Individual uncommitted work item card.
 */
function UncommittedItem({ item, onDismiss, onView }: UncommittedItemProps) {
  const {
    projectName,
    branch,
    linesAdded,
    filesCount,
    lastSessionPreview,
    lastActivityAt,
    insight,
  } = item

  const ageText = formatRelativeTime(lastActivityAt)

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-amber-200 dark:border-amber-700 p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <FileCode2 className="w-4 h-4 text-gray-500 dark:text-gray-400" aria-hidden="true" />
          <span className="font-medium text-gray-900 dark:text-gray-100">
            {projectName}
          </span>
          {branch && (
            <span className="text-sm text-gray-500 dark:text-gray-400">
              ({branch})
            </span>
          )}
        </div>
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-pointer"
            aria-label={`Dismiss ${projectName}`}
          >
            <X className="w-4 h-4" aria-hidden="true" />
          </button>
        )}
      </div>

      {/* Stats */}
      <div className="flex items-center gap-3 text-sm text-gray-600 dark:text-gray-400 mb-2">
        <span className="text-green-600 dark:text-green-400 font-medium tabular-nums">
          +{formatNumber(linesAdded)} lines
        </span>
        <span>in {filesCount} files</span>
        <span className="text-gray-300 dark:text-gray-600">&bull;</span>
        <span className="flex items-center gap-1">
          <Clock className="w-3 h-3" aria-hidden="true" />
          Last session: {ageText}
        </span>
      </div>

      {/* Session Preview */}
      {lastSessionPreview && (
        <p className="text-sm text-gray-700 dark:text-gray-300 mb-2 truncate">
          Session: "{lastSessionPreview}"
        </p>
      )}

      {/* Insight */}
      {insight && (
        <p className="text-xs text-amber-700 dark:text-amber-400 mb-3 italic">
          {insight}
        </p>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2">
        {onDismiss && (
          <button
            onClick={onDismiss}
            className={cn(
              'px-3 py-1.5 text-xs font-medium rounded',
              'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-400',
              'transition-colors cursor-pointer'
            )}
          >
            Dismiss
          </button>
        )}
        {onView && (
          <button
            onClick={onView}
            className={cn(
              'flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded',
              'text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
              'transition-colors cursor-pointer'
            )}
          >
            View
            <ExternalLink className="w-3 h-3" aria-hidden="true" />
          </button>
        )}
      </div>
    </div>
  )
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

