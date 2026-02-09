import { useMemo, useState, useRef, useCallback, useId, useEffect } from 'react'
import { DayPicker, type DateRange } from 'react-day-picker'
import type { DayButtonProps } from 'react-day-picker'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import * as Tooltip from '@radix-ui/react-tooltip'
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

/**
 * Format a date for tooltip display
 * Returns format like "Wed, Jan 29, 2026"
 */
function formatDateForTooltip(date: Date): string {
  return new Intl.DateTimeFormat('en-US', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  }).format(date)
}

/**
 * Format a date for screen reader announcement
 * Returns format like "January 15, 2026: 8 sessions"
 */
function formatDateForScreenReader(date: Date, count: number): string {
  const dateStr = new Intl.DateTimeFormat('en-US', {
    month: 'long',
    day: 'numeric',
    year: 'numeric',
  }).format(date)
  return `${dateStr}: ${count} session${count !== 1 ? 's' : ''}`
}

export function ActivityCalendar({
  sessions,
  selectedRange,
  onRangeChange,
  totalProjects,
}: ActivityCalendarProps) {
  // Null safety: handle null/undefined sessions
  const safeSessions = sessions || []

  const countsByDay = useMemo(() => countSessionsByDay(safeSessions), [safeSessions])

  const earliestDate = useMemo(() => {
    if (!safeSessions || safeSessions.length === 0) return null
    let min = Infinity
    for (const s of safeSessions) {
      if (s?.modifiedAt && s.modifiedAt < min) min = s.modifiedAt
    }
    return min === Infinity ? null : new Date(min * 1000)
  }, [safeSessions])

  const totalSessions = safeSessions?.length || 0

  const sinceLabel = earliestDate
    ? earliestDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })
    : ''

  // Generate unique IDs for accessibility
  const legendId = useId()

  // Track focused cell index for keyboard navigation (roving tabindex pattern)
  const [focusedIndex, setFocusedIndex] = useState(0)
  const cellRefs = useRef<Map<string, HTMLButtonElement | null>>(new Map())
  const cellKeysRef = useRef<string[]>([])

  // Register cell ref by date key
  const registerCellRef = useCallback((dateKey: string, el: HTMLButtonElement | null) => {
    cellRefs.current.set(dateKey, el)
  }, [])

  // Update cell keys list when they're registered
  const updateCellKeys = useCallback((dateKey: string) => {
    if (!cellKeysRef.current.includes(dateKey)) {
      cellKeysRef.current.push(dateKey)
      // Sort by date to maintain proper order
      cellKeysRef.current.sort()
    }
  }, [])

  // Handle keyboard navigation for ARIA grid pattern
  const handleKeyDown = useCallback((e: React.KeyboardEvent, dateKey: string) => {
    const keys = cellKeysRef.current
    const currentIndex = keys.indexOf(dateKey)
    if (currentIndex === -1) return

    let newIndex = currentIndex
    const cols = 7 // Days per week

    switch (e.key) {
      case 'ArrowRight':
        newIndex = Math.min(currentIndex + 1, keys.length - 1)
        break
      case 'ArrowLeft':
        newIndex = Math.max(currentIndex - 1, 0)
        break
      case 'ArrowDown':
        newIndex = Math.min(currentIndex + cols, keys.length - 1)
        break
      case 'ArrowUp':
        newIndex = Math.max(currentIndex - cols, 0)
        break
      case 'Home':
        newIndex = 0
        break
      case 'End':
        newIndex = keys.length - 1
        break
      default:
        return // Don't prevent default for other keys
    }

    if (newIndex !== currentIndex) {
      e.preventDefault()
      setFocusedIndex(newIndex)
      const newKey = keys[newIndex]
      const newCell = cellRefs.current.get(newKey)
      newCell?.focus()
    }
  }, [])

  /**
   * HeatmapDayButton with controlled tooltip state for 150ms close delay.
   * AC-2.4: Tooltip stays open for 150ms after mouse leaves to prevent flickering.
   */
  function HeatmapDayButton({ day, modifiers: _modifiers, ...buttonProps }: DayButtonProps) {
    const dateKey = toDateKey(day.date)
    const count = countsByDay.get(dateKey) ?? 0
    const style = getHeatmapStyle(count)
    const isToday = toDateKey(day.date) === toDateKey(new Date())
    const tooltipId = `tooltip-${dateKey}`

    // Format dates for display and accessibility
    const formattedDate = formatDateForTooltip(day.date)
    const screenReaderLabel = formatDateForScreenReader(day.date, count)

    // Register this cell for keyboard navigation
    updateCellKeys(dateKey)
    const cellIndex = cellKeysRef.current.indexOf(dateKey)
    const isFocusedCell = cellIndex === focusedIndex

    // Controlled tooltip state with 150ms close delay (AC-2.4)
    const [isTooltipOpen, setIsTooltipOpen] = useState(false)
    const closeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

    const handleOpenChange = useCallback((open: boolean) => {
      // Clear any pending close timeout
      if (closeTimeoutRef.current) {
        clearTimeout(closeTimeoutRef.current)
        closeTimeoutRef.current = null
      }

      if (open) {
        // Open immediately
        setIsTooltipOpen(true)
      } else {
        // Close with 150ms delay (AC-2.4)
        closeTimeoutRef.current = setTimeout(() => {
          setIsTooltipOpen(false)
        }, 150)
      }
    }, [])

    // Cleanup timeout on unmount
    useEffect(() => {
      return () => {
        if (closeTimeoutRef.current) {
          clearTimeout(closeTimeoutRef.current)
        }
      }
    }, [])

    return (
      <Tooltip.Root delayDuration={0} open={isTooltipOpen} onOpenChange={handleOpenChange}>
        <Tooltip.Trigger asChild>
          <button
            {...buttonProps}
            ref={(el) => registerCellRef(dateKey, el)}
            role="gridcell"
            aria-label={screenReaderLabel}
            aria-describedby={tooltipId}
            tabIndex={isFocusedCell ? 0 : -1}
            onKeyDown={(e) => handleKeyDown(e, dateKey)}
            className={cn(
              'relative inline-flex items-center justify-center',
              'w-full aspect-square rounded-lg text-xs font-medium',
              'transition-all duration-150 cursor-pointer',
              'hover:ring-2 hover:ring-emerald-400/60 hover:scale-105',
              'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1',
              style.bg, style.text,
              isToday && 'ring-2 ring-emerald-500 ring-offset-1',
            )}
          />
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            id={tooltipId}
            role="tooltip"
            className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm z-50 animate-in fade-in-0 zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
            sideOffset={5}
            side="top"
            align="center"
          >
            <div className="font-medium">{formattedDate}</div>
            <div className="text-gray-200">
              {count} session{count !== 1 ? 's' : ''}
            </div>
            <div className="text-gray-400 text-xs mt-1">Click to filter</div>
            <Tooltip.Arrow className="fill-gray-900" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    )
  }

  return (
    <Tooltip.Provider delayDuration={0}>
      <div
        className="activity-calendar"
        role="grid"
        aria-label="Activity calendar showing sessions per day"
        aria-describedby={legendId}
      >
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
          <div id={legendId} className="flex items-center gap-1.5 text-[10px] text-gray-400">
            <span>Less</span>
            <div className="flex gap-0.5" role="img" aria-label="Activity intensity scale from low to high">
              <div className="w-3 h-3 rounded-sm bg-gray-50 border border-gray-200" aria-hidden="true" />
              <div className="w-3 h-3 rounded-sm bg-emerald-50" aria-hidden="true" />
              <div className="w-3 h-3 rounded-sm bg-emerald-200" aria-hidden="true" />
              <div className="w-3 h-3 rounded-sm bg-emerald-400" aria-hidden="true" />
              <div className="w-3 h-3 rounded-sm bg-emerald-600" aria-hidden="true" />
            </div>
            <span>More</span>
          </div>
        </div>

        {/* Screen reader only legend description */}
        <div id={`${legendId}-description`} className="sr-only">
          Activity levels range from no sessions (empty) to high activity (filled).
          Use arrow keys to navigate between days.
        </div>
      </div>
    </Tooltip.Provider>
  )
}
