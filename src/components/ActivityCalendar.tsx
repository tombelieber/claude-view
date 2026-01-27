import { useMemo } from 'react'
import { DayPicker, type DateRange } from 'react-day-picker'
import type { DayButtonProps } from 'react-day-picker'
import { countSessionsByDay, toDateKey } from '../lib/date-groups'
import type { SessionInfo } from '../hooks/use-projects'

export interface ActivityCalendarProps {
  sessions: SessionInfo[]
  selectedRange: DateRange | undefined
  onRangeChange: (range: DateRange | undefined) => void
  totalProjects?: number
}

function getHeatmapClasses(count: number): string {
  if (count === 0) return 'bg-gray-100'
  if (count <= 2) return 'bg-emerald-100 text-emerald-900'
  if (count <= 5) return 'bg-emerald-300 text-emerald-900'
  if (count <= 10) return 'bg-emerald-500 text-white'
  return 'bg-emerald-700 text-white'
}

export function ActivityCalendar({
  sessions,
  selectedRange,
  onRangeChange,
  totalProjects,
}: ActivityCalendarProps) {
  const countsByDay = useMemo(() => countSessionsByDay(sessions), [sessions])

  const maxCount = useMemo(() => {
    let max = 0
    for (const c of countsByDay.values()) {
      if (c > max) max = c
    }
    return max
  }, [countsByDay])

  const earliestDate = useMemo(() => {
    if (sessions.length === 0) return null
    let min = Infinity
    for (const s of sessions) {
      if (s.modifiedAt < min) min = s.modifiedAt
    }
    return new Date(min * 1000)
  }, [sessions])

  const totalSessions = sessions.length

  const sinceLabel = earliestDate
    ? earliestDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })
    : ''

  function HeatmapDayButton({ day, modifiers, ...buttonProps }: DayButtonProps) {
    const dateKey = toDateKey(day.date)
    const count = countsByDay.get(dateKey) ?? 0
    const heatmapClasses = getHeatmapClasses(count)

    const dateLabel = day.date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    })
    const tooltip = `${dateLabel}: ${count} session${count !== 1 ? 's' : ''}`

    return (
      <button
        {...buttonProps}
        title={tooltip}
        className={`${heatmapClasses} hover:ring-2 hover:ring-emerald-400 inline-flex items-center justify-center w-9 h-9 rounded-md text-xs transition-all duration-150`}
      />
    )
  }

  return (
    <div>
      <DayPicker
        mode="range"
        selected={selectedRange}
        onSelect={onRangeChange}
        showOutsideDays={false}
        components={{
          DayButton: HeatmapDayButton,
        }}
      />
      <p className="mt-2 text-sm text-gray-500">
        {'\u25C9'} {totalSessions} sessions
        {totalProjects != null && <> &middot; {totalProjects} projects</>}
        {sinceLabel && <> &middot; since {sinceLabel}</>}
      </p>
    </div>
  )
}
