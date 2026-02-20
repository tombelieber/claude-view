import { cn } from '../../lib/utils'

export interface StackedBarSegment {
  /** Segment label (shown in legend) */
  label: string
  /** Raw value */
  value: number
  /** Tailwind bg color class for light mode */
  color: string
  /** Tailwind bg color class for dark mode */
  darkColor: string
}

interface StackedBarProps {
  segments: StackedBarSegment[]
  className?: string
}

/**
 * Horizontal stacked bar showing proportional segments.
 * Each segment width = percentage of total. Segments < 1% get min-width for visibility.
 */
export function StackedBar({ segments, className }: StackedBarProps) {
  const total = segments.reduce((sum, s) => sum + s.value, 0)
  if (total === 0) return null

  return (
    <div className={cn('space-y-2', className)}>
      {/* Bar */}
      <div className="flex h-3 w-full rounded-full overflow-hidden bg-gray-100 dark:bg-gray-800">
        {segments.map((seg) => {
          const pct = (seg.value / total) * 100
          if (pct === 0) return null
          return (
            <div
              key={seg.label}
              className={cn(seg.color, seg.darkColor, 'transition-all duration-300')}
              style={{ width: `${Math.max(pct, 0.5)}%` }}
              title={`${seg.label}: ${pct.toFixed(1)}%`}
            />
          )
        })}
      </div>

      {/* Legend */}
      <div className="flex flex-wrap gap-x-4 gap-y-1">
        {segments.map((seg) => {
          const pct = (seg.value / total) * 100
          if (pct === 0) return null
          return (
            <div key={seg.label} className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400">
              <span className={cn('w-2 h-2 rounded-full', seg.color, seg.darkColor)} />
              <span>{seg.label}</span>
              <span className="tabular-nums font-medium">{pct.toFixed(1)}%</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
