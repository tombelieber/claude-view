import { cn } from '../../../../utils/cn'

interface DurationBadgeProps {
  ms: number
  className?: string
}

/**
 * Color-coded duration badge with consistent formatting.
 * - Sub-second: "245ms"
 * - Seconds: "4.2s"
 * - Minutes: "2.1m"
 * Thresholds: >5s amber, >30s red.
 */
export function DurationBadge({ ms, className }: DurationBadgeProps) {
  const secs = ms / 1000
  const text =
    secs >= 60 ? `${(secs / 60).toFixed(1)}m` : secs >= 1 ? `${secs.toFixed(1)}s` : `${ms}ms`

  const color =
    secs > 30
      ? 'text-red-600 dark:text-red-400 bg-red-500/10 dark:bg-red-500/20'
      : secs > 5
        ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
        : 'text-gray-500 dark:text-gray-400 bg-gray-500/10 dark:bg-gray-500/20'

  return (
    <span className={cn('text-xs font-mono tabular-nums px-1.5 py-0.5 rounded', color, className)}>
      {text}
    </span>
  )
}
