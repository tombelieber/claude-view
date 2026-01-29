import { useMemo } from 'react'
import { DayPicker, type DateRange } from 'react-day-picker'
import type { DayButtonProps } from 'react-day-picker'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { countSessionsByDay, toDateKey } from '../lib/date-groups'
import { cn } from '../lib/utils'
import type { SessionInfo } from '../hooks/use-projects'

export interface ActivityCalendarProps {
  sessions: SessionInfo[] | null | undefined
  selectedRange?: DateRange | undefined
  onRangeChange?: (range: DateRange | undefined) => void
  totalProjects?: number
}

function getHeatmapStyle(count: number): { bg: string; text: string } {
  if (count === 0) return { bg: 'bg-gray-50', text: 'text-gray-400' }
  if (count <= 2) return { bg: 'bg-emerald-50', text: 'text-emerald-700' }
  if (count <= 5) return { bg: 'bg-emerald-200', text: 'text-emerald-800' }
  if (count <= 10) return { bg: 'bg-emerald-400', text: 'text-white' }
  return { bg: 'bg-emerald-600', text: 'text-white' }
}

export function ActivityCalendar({
  sessions,
  selectedRange,
  onRangeChange,
  totalProjects,
}: ActivityCalendarProps) {
  // Null safety: handle null/undefined sessions
  const safeSessionss = sessions || []

  const countsByDay = useMemo(() => countSessionsByDay(safeSessionss), [safeSessionss])

  const earliestDate = useMemo(() => {
    if (!safeSessionss || safeSessionss.length === 0) return null
    let min = Infinity
    for (const s of safeSessionss) {
      if (s?.modifiedAt && s.modifiedAt < min) min = s.modifiedAt
    }
    return min === Infinity ? null : new Date(min * 1000)
  }, [safeSessionss])

  const totalSessions = safeSessionss?.length || 0

  const sinceLabel = earliestDate
    ? earliestDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })
    : ''

  function HeatmapDayButton({ day, modifiers, ...buttonProps }: DayButtonProps) {
    const dateKey = toDateKey(day.date)
    const count = countsByDay.get(dateKey) ?? 0
    const style = getHeatmapStyle(count)
    const isToday = toDateKey(day.date) === toDateKey(new Date())

    const dateLabel = day.date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    })
    const tooltip = count > 0
      ? `${dateLabel}: ${count} session${count !== 1 ? 's' : ''}`
      : dateLabel

    return (
      <button
        {...buttonProps}
        title={tooltip}
        className={cn(
          'relative inline-flex items-center justify-center',
          'w-full aspect-square rounded-lg text-xs font-medium',
          'transition-all duration-150 cursor-pointer',
          'hover:ring-2 hover:ring-emerald-400/60 hover:scale-105',
          style.bg, style.text,
          isToday && 'ring-2 ring-emerald-500 ring-offset-1',
        )}
      />
    )
  }

  return (
    <div className="activity-calendar">
      <DayPicker
        mode="range"
        selected={selectedRange}
        onSelect={onRangeChange}
        showOutsideDays={false}
        components={{
          DayButton: HeatmapDayButton,
          Chevron: ({ orientation }) => (
            orientation === 'left'
              ? <ChevronLeft className="w-4 h-4" />
              : <ChevronRight className="w-4 h-4" />
          ),
        }}
      />

      {/* Summary + Legend */}
      <div className="flex items-center justify-between mt-4 pt-4 border-t border-gray-100">
        <div className="flex items-center gap-1.5 text-sm text-gray-500">
          <span className="font-semibold text-gray-900 tabular-nums">{totalSessions}</span>
          <span>sessions</span>
          {totalProjects != null && (
            <>
              <span className="text-gray-300 mx-0.5">/</span>
              <span className="font-semibold text-gray-900 tabular-nums">{totalProjects}</span>
              <span>projects</span>
            </>
          )}
          {sinceLabel && (
            <span className="text-gray-400 ml-1">since {sinceLabel}</span>
          )}
        </div>

        {/* Heatmap legend */}
        <div className="flex items-center gap-1.5 text-[10px] text-gray-400">
          <span>Less</span>
          <div className="flex gap-0.5">
            <div className="w-3 h-3 rounded-sm bg-gray-50 border border-gray-200" />
            <div className="w-3 h-3 rounded-sm bg-emerald-50" />
            <div className="w-3 h-3 rounded-sm bg-emerald-200" />
            <div className="w-3 h-3 rounded-sm bg-emerald-400" />
            <div className="w-3 h-3 rounded-sm bg-emerald-600" />
          </div>
          <span>More</span>
        </div>
      </div>
    </div>
  )
}
