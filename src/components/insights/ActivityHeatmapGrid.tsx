import { useMemo, useState, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { cn } from '../../lib/utils'
import type { HeatmapCell } from '../../types/generated/HeatmapCell'

// ============================================================================
// Types
// ============================================================================

interface ActivityHeatmapGridProps {
  data: HeatmapCell[]
  insight: string
}

interface TooltipState {
  cell: HeatmapCell
  dayIdx: number
  hourIdx: number
  x: number
  y: number
}

// ============================================================================
// Constants
// ============================================================================

const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'] as const
const HOURS = [6, 9, 12, 15, 18, 21, 0] as const
const HOUR_LABELS = [
  '6 AM',
  '9 AM',
  '12 PM',
  '3 PM',
  '6 PM',
  '9 PM',
  '12 AM',
] as const

// ============================================================================
// Helpers
// ============================================================================

function getIntensityClasses(sessions: number, max: number): string {
  if (sessions === 0 || max === 0)
    return 'bg-gray-100 dark:bg-gray-800'
  const ratio = sessions / max
  if (ratio < 0.25) return 'bg-blue-100 dark:bg-blue-900/50'
  if (ratio < 0.5) return 'bg-blue-300 dark:bg-blue-800'
  if (ratio < 0.75) return 'bg-blue-500 dark:bg-blue-600'
  return 'bg-blue-700 dark:bg-blue-500'
}

// ============================================================================
// Component
// ============================================================================

export function ActivityHeatmapGrid({ data, insight }: ActivityHeatmapGridProps) {
  const navigate = useNavigate()
  const [tooltip, setTooltip] = useState<TooltipState | null>(null)

  const { grid, maxSessions } = useMemo(() => {
    const grid = new Map<string, HeatmapCell>()
    let maxSessions = 0

    for (const cell of data) {
      const key = `${cell.dayOfWeek}-${cell.hourOfDay}`
      grid.set(key, cell)
      if (cell.sessions > maxSessions) maxSessions = cell.sessions
    }

    return { grid, maxSessions }
  }, [data])

  const getCell = useCallback(
    (day: number, hour: number): HeatmapCell | undefined => {
      return grid.get(`${day}-${hour}`)
    },
    [grid]
  )

  const handleCellClick = useCallback(
    (day: number, hour: number) => {
      const dayName = DAYS[day].toLowerCase()
      navigate(`/history?day=${dayName}&hour=${hour}`)
    },
    [navigate]
  )

  const handleMouseEnter = useCallback(
    (e: React.MouseEvent, cell: HeatmapCell, dayIdx: number, hourIdx: number) => {
      const rect = (e.target as HTMLElement).getBoundingClientRect()
      setTooltip({
        cell,
        dayIdx,
        hourIdx,
        x: rect.left + rect.width / 2,
        y: rect.top,
      })
    },
    []
  )

  const handleMouseLeave = useCallback(() => {
    setTooltip(null)
  }, [])

  if (data.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          Activity Heatmap
        </h3>
        <div className="flex items-center justify-center py-16 text-gray-500 dark:text-gray-400 text-sm">
          No activity data for this period.
        </div>
      </div>
    )
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Activity Heatmap
      </h3>

      <div className="overflow-x-auto">
        <table
          className="w-full"
          role="grid"
          aria-label="Activity heatmap by day and hour"
        >
          <thead>
            <tr>
              <th className="w-16" />
              {DAYS.map((day) => (
                <th
                  key={day}
                  className="text-xs font-medium text-gray-500 dark:text-gray-400 pb-2 text-center"
                >
                  {day}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {HOURS.map((hour, hourIdx) => (
              <tr key={hour}>
                <td className="text-xs font-medium text-gray-500 dark:text-gray-400 pr-2 text-right whitespace-nowrap">
                  {HOUR_LABELS[hourIdx]}
                </td>
                {DAYS.map((_, dayIdx) => {
                  const cell = getCell(dayIdx, hour)
                  const sessions = cell?.sessions ?? 0
                  const reeditRate = cell?.avgReeditRate ?? 0

                  return (
                    <td key={dayIdx} className="p-0.5">
                      <button
                        onClick={() => handleCellClick(dayIdx, hour)}
                        onMouseEnter={(e) =>
                          cell
                            ? handleMouseEnter(e, cell, dayIdx, hourIdx)
                            : handleMouseEnter(
                                e,
                                {
                                  dayOfWeek: dayIdx,
                                  hourOfDay: hour,
                                  sessions: 0,
                                  avgReeditRate: 0,
                                },
                                dayIdx,
                                hourIdx
                              )
                        }
                        onMouseLeave={handleMouseLeave}
                        className={cn(
                          'w-full aspect-square min-w-[28px] min-h-[28px] rounded-sm cursor-pointer transition-all',
                          'hover:ring-2 hover:ring-blue-400 hover:ring-offset-1 dark:hover:ring-offset-gray-900',
                          'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1 dark:focus:ring-offset-gray-900',
                          getIntensityClasses(sessions, maxSessions)
                        )}
                        aria-label={`${DAYS[dayIdx]} ${HOUR_LABELS[hourIdx]}: ${sessions} sessions, ${(reeditRate * 100).toFixed(0)}% re-edit rate`}
                      />
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Tooltip (portal-free, positioned via fixed) */}
      {tooltip && (
        <div
          className="fixed z-50 pointer-events-none"
          style={{
            left: tooltip.x,
            top: tooltip.y - 8,
            transform: 'translate(-50%, -100%)',
          }}
        >
          <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
            <div className="font-medium">
              {DAYS[tooltip.dayIdx]},{' '}
              {HOUR_LABELS[tooltip.hourIdx]} -{' '}
              {HOUR_LABELS[tooltip.hourIdx + 1] ?? '6 AM'}
            </div>
            <div>
              {tooltip.cell.sessions} session
              {tooltip.cell.sessions !== 1 ? 's' : ''}
            </div>
            <div>
              {(tooltip.cell.avgReeditRate * 100).toFixed(0)}% avg re-edit rate
            </div>
            <div className="text-gray-400 text-xs mt-1">Click to filter</div>
            {/* Arrow */}
            <div className="absolute left-1/2 -translate-x-1/2 top-full w-0 h-0 border-l-[6px] border-r-[6px] border-t-[6px] border-l-transparent border-r-transparent border-t-gray-900" />
          </div>
        </div>
      )}

      {/* Legend */}
      <div className="flex flex-wrap items-center gap-4 mt-4 text-xs text-gray-500 dark:text-gray-400">
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-gray-100 dark:bg-gray-800" />
          <span>None</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-100 dark:bg-blue-900/50" />
          <span>Low</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-300 dark:bg-blue-800" />
          <span>Medium</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-500 dark:bg-blue-600" />
          <span>High</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-700 dark:bg-blue-500" />
          <span>Peak</span>
        </div>
      </div>

      <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
        <span className="shrink-0 mt-0.5 text-amber-500">*</span>
        <span>{insight}</span>
      </p>
    </div>
  )
}
