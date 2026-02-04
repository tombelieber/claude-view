import { cn } from '../../lib/utils'

export interface ProgressBarProps {
  /** Label for the progress bar */
  label: string
  /** Current value */
  value: number
  /** Maximum value */
  max: number
  /** Optional suffix text (e.g., "3.5M tokens") */
  suffix?: string
  /** Optional className for additional styling */
  className?: string
  /** Stacked layout: label above bar instead of inline (for mobile) */
  stacked?: boolean
}

/**
 * ProgressBar displays a horizontal bar with label, percentage fill, and optional suffix.
 *
 * Design tokens:
 * - Primary text: #1F2937 (gray-800)
 * - Secondary text: #6B7280 (gray-500)
 * - Progress fill: Blue gradient (#3B82F6 -> #1E40AF)
 *
 * Accessibility:
 * - Uses progressbar role with aria-valuenow, aria-valuemin, aria-valuemax
 * - Label is associated via aria-label
 */
export function ProgressBar({
  label,
  value,
  max,
  suffix,
  className,
  stacked = false,
}: ProgressBarProps) {
  // Ensure we don't divide by zero and clamp percentage between 0-100
  const percentage = max > 0 ? Math.min(Math.max((value / max) * 100, 0), 100) : 0
  const roundedPercentage = Math.round(percentage)

  // Stacked layout: label and stats on separate lines above the bar
  if (stacked) {
    return (
      <div className={cn('mb-3', className)}>
        <div className="mb-1.5">
          <span className="text-sm font-medium text-gray-800 dark:text-gray-200 block truncate">
            {label}
          </span>
          <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400 tabular-nums mt-0.5">
            <span>{roundedPercentage}%</span>
            {suffix && <span className="font-medium">{suffix}</span>}
          </div>
        </div>
        <div
          className="h-2 w-full bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden"
          role="progressbar"
          aria-valuenow={value}
          aria-valuemin={0}
          aria-valuemax={max}
          aria-label={`${label}: ${roundedPercentage}%${suffix ? ` (${suffix})` : ''}`}
        >
          <div
            className="h-full rounded-full bg-gradient-to-r from-blue-500 to-blue-800 transition-all duration-300 ease-out"
            style={{ width: `${percentage}%` }}
          />
        </div>
      </div>
    )
  }

  // Default inline layout
  return (
    <div className={cn('mb-2', className)}>
      <div className="flex items-center justify-between mb-1">
        <span className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">
          {label}
        </span>
        <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 tabular-nums shrink-0">
          <span>{roundedPercentage}%</span>
          {suffix && <span className="font-medium">{suffix}</span>}
        </div>
      </div>
      <div
        className="h-2 w-full bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden"
        role="progressbar"
        aria-valuenow={value}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-label={`${label}: ${roundedPercentage}%${suffix ? ` (${suffix})` : ''}`}
      >
        <div
          className="h-full rounded-full bg-gradient-to-r from-blue-500 to-blue-800 transition-all duration-300 ease-out"
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  )
}
