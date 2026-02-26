import { TrendingUp, TrendingDown, Minus } from 'lucide-react'
import { cn } from '../../lib/utils'

export interface MetricCardProps {
  /** Display label for the metric */
  label: string
  /** Primary value to display (formatted) */
  value: string
  /** Optional secondary value displayed below main value */
  subValue?: string
  /** Optional footer text */
  footer?: string
  /** Optional trend information (+/- percentage) */
  trend?: {
    /** The raw delta value */
    delta: number
    /** The percentage change (null if not calculable) */
    deltaPercent: number | null
  }
  /** Optional className for additional styling */
  className?: string
}

/**
 * MetricCard displays a single metric with optional sub-value, footer, and trend indicator.
 *
 * Design tokens:
 * - Primary text: #1F2937 (gray-800)
 * - Secondary text: #6B7280 (gray-500)
 * - Metric value: #1E40AF (blue-800)
 * - Positive trend: Green (#22C55E)
 * - Negative trend: Red (#EF4444)
 *
 * Accessibility:
 * - Uses role="group" with aria-label combining all visible information
 * - Trend direction is conveyed through icon and text
 */
export function MetricCard({
  label,
  value,
  subValue,
  footer,
  trend,
  className,
}: MetricCardProps) {
  const hasTrend = trend && trend.deltaPercent !== null
  const isPositive = hasTrend && trend.delta > 0
  const isNegative = hasTrend && trend.delta < 0
  const isNeutral = hasTrend && trend.delta === 0

  // Build screen reader text
  const trendText = hasTrend
    ? isPositive
      ? `up ${trend.deltaPercent?.toFixed(1)}%`
      : isNegative
        ? `down ${Math.abs(trend.deltaPercent ?? 0).toFixed(1)}%`
        : 'no change'
    : ''

  const ariaLabel = [
    `${label}: ${value}`,
    subValue,
    footer,
    trendText,
  ]
    .filter(Boolean)
    .join(', ')

  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4',
        className
      )}
      role="group"
      aria-label={ariaLabel}
    >
      {/* Label */}
      <p
        className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2"
        aria-hidden="true"
      >
        {label}
      </p>

      {/* Value and trend row */}
      <div className="flex items-baseline justify-between gap-2">
        <span
          className="text-2xl font-semibold text-blue-800 dark:text-blue-300 tabular-nums"
          aria-hidden="true"
        >
          {value}
        </span>

        {hasTrend && (
          <div
            className={cn(
              'flex items-center gap-1 text-sm',
              isPositive && 'text-green-600 dark:text-green-400',
              isNegative && 'text-red-600 dark:text-red-400',
              isNeutral && 'text-gray-500 dark:text-gray-400'
            )}
            aria-hidden="true"
          >
            {isPositive && <TrendingUp className="w-4 h-4" />}
            {isNegative && <TrendingDown className="w-4 h-4" />}
            {isNeutral && <Minus className="w-4 h-4" />}
            <span className="tabular-nums">
              {trend.deltaPercent !== null
                ? `${trend.deltaPercent > 0 ? '+' : ''}${trend.deltaPercent.toFixed(1)}%`
                : '--'}
            </span>
          </div>
        )}
      </div>

      {/* Sub-value */}
      {subValue && (
        <p
          className="text-sm text-gray-500 dark:text-gray-400 mt-1"
          aria-hidden="true"
        >
          {subValue}
        </p>
      )}

      {/* Footer */}
      {footer && (
        <p
          className="text-xs text-gray-400 dark:text-gray-500 mt-2 pt-2 border-t border-gray-100 dark:border-gray-800"
          aria-hidden="true"
        >
          {footer}
        </p>
      )}
    </div>
  )
}
