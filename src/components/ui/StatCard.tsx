import type { LucideIcon } from 'lucide-react'
import { cn } from '../../lib/utils'

export interface StatCardProps {
  /** Display label for the stat */
  label: string
  /** Value to display (formatted) */
  value: string
  /** Optional icon displayed above the label */
  icon?: LucideIcon
  /** Optional className for additional styling */
  className?: string
}

/**
 * StatCard is a simple stat card for storage overview grids.
 *
 * Design tokens:
 * - Primary text: gray-800 / gray-100 (dark)
 * - Secondary text: gray-500 / gray-400 (dark)
 * - Background: gray-50 / frosted glass (dark)
 * - Border: subtle ring for definition
 *
 * This is a simplified version of MetricCard without trend indicators,
 * designed for compact grid layouts in the Settings/Storage section.
 */
export function StatCard({ label, value, icon: Icon, className }: StatCardProps) {
  return (
    <div
      className={cn(
        'bg-gray-50 dark:bg-white/[0.05] rounded-lg p-4 text-center',
        'ring-1 ring-gray-950/[0.05] dark:ring-white/[0.06]',
        className
      )}
      role="group"
      aria-label={`${label}: ${value}`}
    >
      {/* Icon */}
      {Icon && (
        <div className="flex justify-center mb-2" aria-hidden="true">
          <Icon className="w-4 h-4 text-gray-400 dark:text-gray-500" strokeWidth={1.5} />
        </div>
      )}

      {/* Label */}
      <p
        className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1"
        aria-hidden="true"
      >
        {label}
      </p>

      {/* Value */}
      <p
        className="text-lg font-semibold text-gray-800 dark:text-gray-100 tabular-nums leading-tight"
        aria-hidden="true"
      >
        {value}
      </p>
    </div>
  )
}
