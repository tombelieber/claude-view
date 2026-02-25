import { useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatHumanDuration } from '../../lib/format-utils'
import type { DayActivity } from '../../lib/activity-utils'

const DAY_LABELS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

/** Get intensity level 0-3 based on total seconds */
function intensityLevel(totalSeconds: number): number {
  if (totalSeconds === 0) return 0
  if (totalSeconds < 3600) return 1     // < 1h
  if (totalSeconds < 10800) return 2    // < 3h
  return 3                               // 3h+
}

const INTENSITY_CLASSES = [
  'bg-gray-100 dark:bg-gray-800',         // 0: no activity
  'bg-blue-200 dark:bg-blue-900',          // 1: < 1h
  'bg-blue-400 dark:bg-blue-700',          // 2: 1-3h
  'bg-blue-600 dark:bg-blue-500',          // 3: 3h+
] as const

interface CalendarHeatmapProps {
  days: DayActivity[]
  onDayClick?: (date: string) => void
  selectedDate?: string | null
}

export function CalendarHeatmap({ days, onDayClick, selectedDate }: CalendarHeatmapProps) {
  const [monthOffset, setMonthOffset] = useState(0)

  // Build lookup map: YYYY-MM-DD -> DayActivity
  // Memoize on content-aware key (dates + totals) to catch time-range changes
  // that return the same number of days but different data
  const daysKey = days.map(d => `${d.date}:${d.totalSeconds}`).join(',')
  const dayMap = useMemo(() => {
    const map = new Map<string, DayActivity>()
    for (const d of days) map.set(d.date, d)
    return map
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [daysKey])

  // Compute the grid for the current month
  const { monthLabel, cells } = useMemo(() => {
    const now = new Date()
    const target = new Date(now.getFullYear(), now.getMonth() + monthOffset, 1)
    const year = target.getFullYear()
    const month = target.getMonth()
    const label = target.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })

    // Days in this month
    const daysInMonth = new Date(year, month + 1, 0).getDate()
    const firstDayOfWeek = new Date(year, month, 1).getDay() // 0=Sun
    // Convert to Mon=0 format
    const startOffset = (firstDayOfWeek + 6) % 7

    const grid: { date: string; day: number; activity: DayActivity | undefined }[] = []

    // Pad start with empty cells
    for (let i = 0; i < startOffset; i++) {
      grid.push({ date: '', day: 0, activity: undefined })
    }

    for (let d = 1; d <= daysInMonth; d++) {
      const dateStr = `${year}-${String(month + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`
      grid.push({ date: dateStr, day: d, activity: dayMap.get(dateStr) })
    }

    return { monthLabel: label, cells: grid }
  }, [monthOffset, dayMap])

  // Arrange into rows (weeks)
  const weeks: typeof cells[] = []
  for (let i = 0; i < cells.length; i += 7) {
    weeks.push(cells.slice(i, i + 7))
  }
  // Pad the last week to 7 cells
  const lastWeek = weeks[weeks.length - 1]
  if (lastWeek) {
    while (lastWeek.length < 7) {
      lastWeek.push({ date: '', day: 0, activity: undefined })
    }
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-200">Activity Map</h2>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setMonthOffset(prev => prev - 1)}
            className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-800 transition-colors cursor-pointer"
            aria-label="Previous month"
          >
            <ChevronLeft className="w-4 h-4 text-gray-500" />
          </button>
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300 min-w-[140px] text-center">
            {monthLabel}
          </span>
          <button
            type="button"
            onClick={() => setMonthOffset(prev => Math.min(prev + 1, 0))}
            disabled={monthOffset >= 0}
            aria-disabled={monthOffset >= 0}
            className={cn(
              'p-1 rounded transition-colors cursor-pointer',
              monthOffset >= 0
                ? 'text-gray-300 dark:text-gray-600 cursor-default'
                : 'hover:bg-gray-200 dark:hover:bg-gray-800 text-gray-500'
            )}
            aria-label="Next month"
          >
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Day labels + Calendar grid — scrollable on narrow screens */}
      <div className="overflow-x-auto">
      <div>
      <div className="grid grid-cols-7 gap-1 mb-1">
        {DAY_LABELS.map((label) => (
          <div key={label} className="text-[10px] text-center text-gray-400 dark:text-gray-500">
            {label}
          </div>
        ))}
      </div>

      {/* Calendar grid */}
      <div className="space-y-1">
        {weeks.map((week, wi) => (
          <div key={wi} className="grid grid-cols-7 gap-1">
            {week.map((cell, ci) => {
              if (!cell.date) {
                return <div key={ci} className="w-9 h-9 rounded" aria-hidden="true" />
              }
              const level = intensityLevel(cell.activity?.totalSeconds ?? 0)
              const isSelected = selectedDate === cell.date
              return (
                <button
                  key={cell.date}
                  type="button"
                  onClick={() => onDayClick?.(cell.date)}
                  className={cn(
                    'w-9 h-9 rounded-sm transition-all duration-150 cursor-pointer relative group',
                    INTENSITY_CLASSES[level],
                    isSelected && 'ring-2 ring-blue-500 ring-offset-1 dark:ring-offset-gray-950',
                  )}
                  aria-label={`${cell.date}: ${cell.activity ? formatHumanDuration(cell.activity.totalSeconds) + ' across ' + cell.activity.sessionCount + ' sessions' : 'No activity'}`}
                >
                  {/* Tooltip */}
                  <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10 pointer-events-none">
                    <div className="bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 text-[10px] px-2 py-1 rounded whitespace-nowrap shadow-lg">
                      {cell.activity
                        ? `${cell.date}: ${formatHumanDuration(cell.activity.totalSeconds)} (${cell.activity.sessionCount} sessions)`
                        : `${cell.date}: No activity`
                      }
                    </div>
                  </div>
                </button>
              )
            })}
          </div>
        ))}
      </div>
      </div>
      </div>{/* end overflow-x-auto */}

      {/* Legend */}
      <div className="flex items-center gap-2 mt-3 text-[10px] text-gray-400 dark:text-gray-500">
        <span>Less</span>
        {INTENSITY_CLASSES.map((cls, i) => (
          <div key={i} className={cn('w-4 h-4 rounded', cls)} />
        ))}
        <span>More</span>
      </div>
    </div>
  )
}
