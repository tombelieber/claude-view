import { TrendingUp, TrendingDown } from 'lucide-react'
import { cn } from '../lib/utils'

export interface MetricCardProps {
  /** Display label for the metric */
  label: string
  /** Formatted value to display */
  value: string
  /** Optional trend information */
  trend?: {
    delta: number
    deltaPercent: number | null
  }
  /** Optional className for additional styling */
  className?: string
}

/**
 * MetricCard displays a single metric with optional trend indicator.
 *
 * Design system:
 * - Value: Fira Code, text-2xl font-semibold text-blue-900
 * - Label: Fira Sans
 * - Trend: Lucide TrendingUp/TrendingDown icon + percent (no color coding, icon direction sufficient)
 */
export function MetricCard({ label, value, trend, className }: MetricCardProps) {
  const hasTrend = trend && trend.deltaPercent !== null
  const isPositive = hasTrend && trend.delta > 0
  const isNegative = hasTrend && trend.delta < 0

  return (
    <div
      className={cn(
        'bg-white rounded-xl border border-gray-200 p-4',
        className
      )}
    >
      <p className="text-xs font-medium text-gray-500 uppercase tracking-wider font-metric-label mb-2">
        {label}
      </p>
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-2xl font-semibold text-blue-900 font-metric-value tabular-nums">
          {value}
        </span>
        {hasTrend && (
          <div className="flex items-center gap-1 text-sm text-gray-500">
            {isPositive && (
              <TrendingUp className="w-4 h-4" aria-hidden="true" />
            )}
            {isNegative && (
              <TrendingDown className="w-4 h-4" aria-hidden="true" />
            )}
            <span className="font-metric-value tabular-nums">
              {trend.deltaPercent !== null
                ? `${trend.deltaPercent > 0 ? '+' : ''}${trend.deltaPercent.toFixed(1)}%`
                : '--'}
            </span>
          </div>
        )}
      </div>
    </div>
  )
}
