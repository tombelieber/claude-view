import { AlertCircle, Info, RefreshCw } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { ContributionWarning } from '../../types/generated'

interface WarningBannerProps {
  warnings: ContributionWarning[]
  onSync?: () => void
  className?: string
}

/**
 * WarningBanner displays data quality warnings.
 *
 * Shows different messages based on warning type:
 * - GitSyncIncomplete: Commit data may be stale
 * - CostUnavailable: Token/cost data missing
 * - PartialData: Some sessions still indexing
 */
export function WarningBanner({ warnings, onSync, className }: WarningBannerProps) {
  if (warnings.length === 0) return null

  // Determine severity (any GitSyncIncomplete or PartialData = warning, else info)
  const hasActionableWarning = warnings.some(
    (w) => w.code === 'GitSyncIncomplete' || w.code === 'PartialData'
  )

  const bgColor = hasActionableWarning
    ? 'bg-amber-50 dark:bg-amber-900/20 border-amber-200 dark:border-amber-800'
    : 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800'

  const iconColor = hasActionableWarning
    ? 'text-amber-600 dark:text-amber-400'
    : 'text-blue-600 dark:text-blue-400'

  const textColor = hasActionableWarning
    ? 'text-amber-800 dark:text-amber-200'
    : 'text-blue-800 dark:text-blue-200'

  const Icon = hasActionableWarning ? AlertCircle : Info

  // Check if sync action is relevant
  const showSyncAction = onSync && warnings.some((w) => w.code === 'GitSyncIncomplete')

  return (
    <div
      className={cn('border rounded-xl p-4', bgColor, className)}
      role="alert"
    >
      <div className="flex items-start gap-3">
        <Icon className={cn('w-5 h-5 flex-shrink-0 mt-0.5', iconColor)} aria-hidden="true" />
        <div className="flex-1 space-y-1">
          {warnings.map((warning, i) => (
            <p key={i} className={cn('text-sm', textColor)}>
              {warning.message}
            </p>
          ))}
        </div>

        {/* Sync action button */}
        {showSyncAction && (
          <button
            onClick={onSync}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg transition-colors cursor-pointer',
              'bg-amber-100 dark:bg-amber-800/50 text-amber-800 dark:text-amber-200',
              'hover:bg-amber-200 dark:hover:bg-amber-800',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-400'
            )}
          >
            <RefreshCw className="w-4 h-4" aria-hidden="true" />
            Sync
          </button>
        )}
      </div>
    </div>
  )
}

/**
 * Get user-friendly warning message.
 * Uses the message from the warning object, with fallback for unknown codes.
 */
function getWarningMessage(warning: ContributionWarning): string {
  // Use the server-provided message if available
  if (warning.message) {
    return warning.message
  }
  // Fallback for unknown codes
  return 'Some data may be incomplete'
}

/**
 * Compact warning indicator for use in cards.
 */
export function WarningIndicator({
  warning,
  className,
}: {
  warning: ContributionWarning
  className?: string
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400',
        className
      )}
      title={getWarningMessage(warning)}
    >
      <AlertCircle className="w-3 h-3" aria-hidden="true" />
      <span className="sr-only">{getWarningMessage(warning)}</span>
    </span>
  )
}
