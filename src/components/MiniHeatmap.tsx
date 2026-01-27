// src/components/MiniHeatmap.tsx
// Compact GitHub-style contribution heatmap strip

import { useMemo } from 'react'
import { countSessionsByDay, toDateKey } from '../lib/date-groups'
import type { SessionInfo } from '../hooks/use-projects'

interface MiniHeatmapProps {
  sessions: SessionInfo[]
  /** Number of weeks to show (default: 14) */
  weeks?: number
}

function getColor(count: number): string {
  if (count === 0) return '#f3f4f6'    // gray-100
  if (count <= 2) return '#a7f3d0'     // emerald-200
  if (count <= 5) return '#34d399'     // emerald-400
  if (count <= 10) return '#10b981'    // emerald-500
  return '#047857'                      // emerald-700
}

interface Cell {
  key: string
  count: number
  date: Date
  col: number
  row: number
}

export function MiniHeatmap({ sessions, weeks = 14 }: MiniHeatmapProps) {
  const countsByDay = useMemo(() => countSessionsByDay(sessions), [sessions])

  const { cells, monthLabels, totalCols } = useMemo(() => {
    const today = new Date()
    today.setHours(23, 59, 59, 999)

    // Start: go back N weeks, align to Sunday
    const start = new Date(today)
    start.setDate(start.getDate() - (weeks * 7) + 1)
    start.setDate(start.getDate() - start.getDay())
    start.setHours(0, 0, 0, 0)

    const cells: Cell[] = []
    const labels: { col: number; label: string }[] = []
    let lastMonth = -1
    const cursor = new Date(start)
    let col = 0

    while (cursor <= today) {
      for (let row = 0; row < 7; row++) {
        if (cursor > today) break
        const d = new Date(cursor)
        const key = toDateKey(d)
        const count = countsByDay.get(key) ?? 0
        cells.push({ key, count, date: d, col, row })

        if (row === 0 && d.getMonth() !== lastMonth) {
          labels.push({ col, label: d.toLocaleDateString('en-US', { month: 'short' }) })
          lastMonth = d.getMonth()
        }
        cursor.setDate(cursor.getDate() + 1)
      }
      col++
    }

    return { cells, monthLabels: labels, totalCols: col }
  }, [countsByDay, weeks])

  // Stats
  const totalSessions = sessions.length
  const activeDays = useMemo(() => {
    let count = 0
    countsByDay.forEach(v => { if (v > 0) count++ })
    return count
  }, [countsByDay])

  const streak = useMemo(() => {
    let s = 0
    const d = new Date()
    d.setHours(0, 0, 0, 0)
    while (true) {
      const key = toDateKey(d)
      if ((countsByDay.get(key) ?? 0) > 0) {
        s++
        d.setDate(d.getDate() - 1)
      } else break
    }
    return s
  }, [countsByDay])

  const size = 11
  const gap = 3
  const step = size + gap
  const labelH = 16
  const svgW = totalCols * step
  const svgH = 7 * step + labelH

  return (
    <div className="flex items-start gap-6">
      {/* Heatmap */}
      <div className="flex-1 min-w-0 overflow-x-auto">
        <svg width={svgW} height={svgH} className="block">
          {/* Month labels */}
          {monthLabels.map(m => (
            <text
              key={`${m.label}-${m.col}`}
              x={m.col * step}
              y={10}
              className="fill-gray-400"
              fontSize={10}
              fontWeight={500}
            >
              {m.label}
            </text>
          ))}

          {/* Day cells */}
          {cells.map(cell => {
            const x = cell.col * step
            const y = cell.row * step + labelH
            const dateLabel = cell.date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
            const tooltip = cell.count > 0
              ? `${dateLabel}: ${cell.count} session${cell.count !== 1 ? 's' : ''}`
              : dateLabel

            return (
              <rect
                key={cell.key}
                x={x}
                y={y}
                width={size}
                height={size}
                rx={2}
                fill={getColor(cell.count)}
                className="transition-opacity hover:opacity-80"
              >
                <title>{tooltip}</title>
              </rect>
            )
          })}
        </svg>

        {/* Legend */}
        <div className="flex items-center gap-1.5 mt-1.5">
          <span className="text-[10px] text-gray-400">Less</span>
          <div className="flex gap-[2px]">
            {['#f3f4f6', '#a7f3d0', '#34d399', '#10b981', '#047857'].map(c => (
              <div key={c} className="w-[10px] h-[10px] rounded-[2px]" style={{ backgroundColor: c }} />
            ))}
          </div>
          <span className="text-[10px] text-gray-400">More</span>
        </div>
      </div>

      {/* Quick stats */}
      <div className="flex-shrink-0 flex gap-5 pt-4">
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 tabular-nums leading-tight">{totalSessions}</p>
          <p className="text-[10px] text-gray-400 uppercase tracking-wider mt-0.5">sessions</p>
        </div>
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 tabular-nums leading-tight">{activeDays}</p>
          <p className="text-[10px] text-gray-400 uppercase tracking-wider mt-0.5">active days</p>
        </div>
        {streak > 0 && (
          <div className="text-right">
            <p className="text-xl font-semibold text-emerald-600 tabular-nums leading-tight">{streak}</p>
            <p className="text-[10px] text-gray-400 uppercase tracking-wider mt-0.5">day streak</p>
          </div>
        )}
      </div>
    </div>
  )
}
