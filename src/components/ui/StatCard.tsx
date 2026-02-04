import { cn } from '../../lib/utils'

export interface StatCardProps {
  /** Display label for the stat */
  label: string
  /** Value to display (formatted) */
  value: string
  /** Optional className for additional styling */
  className?: string
}

/**
 * StatCard is a simple stat card for storage overview grids.
 *
 * Design tokens:
 * - Primary text: #1F2937 (gray-800)
 * - Secondary text: #6B7280 (gray-500)
 *
 * This is a simplified version of MetricCard without trend indicators,
 * designed for compact grid layouts in the Settings/Storage section.
 */
export function StatCard({ label, value, className }: StatCardProps) {
  return (
    <div
      className={cn(
        'bg-gray-50 dark:bg-gray-800 rounded-lg p-4 text-center',
        className
      )}
      role="group"
      aria-label={`${label}: ${value}`}
    >
      {/* Label */}
      <p
        className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1"
        aria-hidden="true"
      >
        {label}
      </p>

      {/* Value */}
      <p
        className="text-lg font-semibold text-gray-800 dark:text-gray-200 tabular-nums"
        aria-hidden="true"
      >
        {value}
      </p>
    </div>
  )
}
