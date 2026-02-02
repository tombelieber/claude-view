// src/components/ActivitySparkline.tsx
// Area chart showing daily session counts with click-to-filter, powered by Recharts

import { useMemo, useCallback, useRef, useEffect } from 'react'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  type TooltipProps,
} from 'recharts'
import { countSessionsByDay, toDateKey } from '../lib/date-groups'
import { useTheme } from '../hooks/use-theme'
import type { SessionInfo } from '../hooks/use-projects'

interface ActivitySparklineProps {
  sessions: SessionInfo[]
  /** Currently selected date key (YYYY-MM-DD) or null */
  selectedDate: string | null
  /** Called when a day dot is clicked */
  onDateSelect: (dateKey: string | null) => void
}

interface DayDatum {
  key: string       // YYYY-MM-DD
  date: number      // timestamp ms (for XAxis)
  count: number
  label: string     // formatted label for tooltip
}

export function ActivitySparkline({
  sessions,
  selectedDate,
  onDateSelect,
}: ActivitySparklineProps) {
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'
  const countsByDay = useMemo(() => countSessionsByDay(sessions), [sessions])

  // Auto-size time range: from earliest session (or 14 days ago) to today,
  // with a minimum of 14 days and padded by 2 days on each end
  const dayData = useMemo((): DayDatum[] => {
    const today = new Date()
    today.setHours(0, 0, 0, 0)

    // Find earliest session date
    let earliest = new Date(today)
    earliest.setDate(earliest.getDate() - 13) // minimum 14 days
    for (const s of sessions) {
      const d = new Date(s.modifiedAt * 1000)
      d.setHours(0, 0, 0, 0)
      if (d < earliest) earliest = d
    }

    // Pad 2 days before earliest
    const start = new Date(earliest)
    start.setDate(start.getDate() - 2)

    // Pad 1 day after today
    const end = new Date(today)
    end.setDate(end.getDate() + 1)

    const data: DayDatum[] = []
    const cursor = new Date(start)
    while (cursor <= end) {
      const key = toDateKey(cursor)
      data.push({
        key,
        date: cursor.getTime(),
        count: countsByDay.get(key) ?? 0,
        label: cursor.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
      })
      cursor.setDate(cursor.getDate() + 1)
    }
    return data
  }, [countsByDay, sessions])

  // Stats
  const totalSessions = sessions.length
  const activeDays = useMemo(() => {
    let c = 0
    countsByDay.forEach(v => { if (v > 0) c++ })
    return c
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

  // Compute nice Y-axis ticks: 0 and a clean max (round up to nearest nice number)
  const maxCount = useMemo(() => Math.max(...dayData.map(d => d.count), 1), [dayData])
  const yTicks = useMemo(() => {
    if (maxCount <= 1) return [0, 1]
    if (maxCount <= 4) return Array.from({ length: maxCount + 1 }, (_, i) => i)
    // Round up to a nice ceiling: nearest multiple of 5 or 10
    const step = maxCount <= 10 ? 2 : maxCount <= 25 ? 5 : 10
    const ceil = Math.ceil(maxCount / step) * step
    const ticks = [0]
    for (let v = step; v <= ceil; v += step) ticks.push(v)
    return ticks
  }, [maxCount])
  const yDomain = [0, yTicks[yTicks.length - 1]]

  // Compute smart X-axis ticks: evenly-spaced dates that always include first and last
  const xTicks = useMemo(() => {
    if (dayData.length === 0) return []
    const totalDays = dayData.length
    // Aim for ~6-8 labels max, spaced by week-ish intervals
    const targetCount = Math.min(8, totalDays)
    const step = Math.max(1, Math.floor(totalDays / targetCount))
    const ticks: number[] = []
    for (let i = 0; i < totalDays; i += step) {
      ticks.push(dayData[i].date)
    }
    // Always include the last date
    const last = dayData[totalDays - 1].date
    if (ticks[ticks.length - 1] !== last) ticks.push(last)
    return ticks
  }, [dayData])

  const handleClick = useCallback((data: { activePayload?: Array<{ payload: DayDatum }> }) => {
    const datum = data?.activePayload?.[0]?.payload
    if (!datum || datum.count === 0) return
    onDateSelect(selectedDate === datum.key ? null : datum.key)
  }, [selectedDate, onDateSelect])

  const formatXTick = useCallback((ts: number) => {
    const d = new Date(ts)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }, [])

  // Horizontal scroll: give each day at least 12px. If that exceeds the container, scroll.
  const MIN_PX_PER_DAY = 12
  const scrollRef = useRef<HTMLDivElement>(null)
  const chartWidth = dayData.length * MIN_PX_PER_DAY
  // needsScroll is determined at render; container width checked via ref
  const containerRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to the right (most recent) on mount and when data changes
  useEffect(() => {
    const el = scrollRef.current
    if (el) el.scrollLeft = el.scrollWidth
  }, [dayData])

  // Custom dot renderer: show dots on days with activity
  const renderDot = useCallback((props: {
    cx: number; cy: number; payload: DayDatum; index: number
  }) => {
    const { cx, cy, payload } = props
    if (payload.count === 0) return <g key={`dot-empty-${payload.key}`} />

    const isSelected = payload.key === selectedDate
    return (
      <circle
        key={`dot-${payload.key}`}
        cx={cx}
        cy={cy}
        r={isSelected ? 5 : 3}
        fill={isSelected ? '#059669' : '#10b981'}
        stroke={isDark ? '#111827' : 'white'}
        strokeWidth={isSelected ? 2 : 1.5}
        style={{ cursor: 'pointer' }}
      />
    )
  }, [selectedDate, isDark])

  return (
    <div className="flex items-start gap-6">
      {/* Chart area */}
      <div ref={containerRef} className="flex-1 min-w-0">
        <div
          ref={scrollRef}
          className="overflow-x-auto"
          style={{ scrollbarWidth: 'thin', scrollbarColor: isDark ? '#4b5563 transparent' : '#d1d5db transparent' }}
        >
          <div style={{ minWidth: chartWidth, width: '100%' }}>
            <ResponsiveContainer width="100%" height={140}>
              <AreaChart
                data={dayData}
                margin={{ top: 8, right: 12, bottom: 0, left: -12 }}
                onClick={handleClick}
                style={{ cursor: 'pointer' }}
              >
                <defs>
                  <linearGradient id="sparkGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#10b981" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#10b981" stopOpacity={0.02} />
                  </linearGradient>
                </defs>

                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke={isDark ? '#374151' : '#f0f0f0'}
                  vertical={false}
                />

                <XAxis
                  dataKey="date"
                  type="number"
                  domain={['dataMin', 'dataMax']}
                  ticks={xTicks}
                  tickFormatter={formatXTick}
                  tick={{ fontSize: 10, fill: isDark ? '#9ca3af' : '#9ca3af' }}
                  axisLine={{ stroke: isDark ? '#4b5563' : '#e5e7eb' }}
                  tickLine={{ stroke: isDark ? '#4b5563' : '#e5e7eb', strokeWidth: 1 }}
                  tickMargin={6}
                />

                <YAxis
                  domain={yDomain}
                  ticks={yTicks}
                  tick={{ fontSize: 10, fill: isDark ? '#9ca3af' : '#9ca3af' }}
                  axisLine={false}
                  tickLine={false}
                  tickMargin={4}
                  allowDecimals={false}
                  width={32}
                />

                <Tooltip
                  content={<CustomTooltip />}
                  cursor={{ stroke: isDark ? '#6b7280' : '#d1d5db', strokeDasharray: '3 3' }}
                />

                <Area
                  type="monotone"
                  dataKey="count"
                  stroke="#10b981"
                  strokeWidth={2}
                  fill="url(#sparkGrad)"
                  dot={renderDot}
                  activeDot={{
                    r: 5,
                    fill: '#059669',
                    stroke: isDark ? '#111827' : 'white',
                    strokeWidth: 2,
                    cursor: 'pointer',
                  }}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* Selected date chip */}
        {selectedDate && (
          <button
            onClick={() => onDateSelect(null)}
            className="mt-2 inline-flex items-center gap-1 px-2 py-0.5 text-[11px] font-medium bg-emerald-50 dark:bg-emerald-950/30 text-emerald-700 dark:text-emerald-300 rounded-full hover:bg-emerald-100 dark:hover:bg-emerald-900/30 transition-colors"
          >
            {(() => {
              const [y, m, d] = selectedDate.split('-').map(Number)
              return new Date(y, m - 1, d).toLocaleDateString('en-US', {
                weekday: 'short', month: 'short', day: 'numeric',
              })
            })()}
            <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        )}
      </div>

      {/* Quick stats */}
      <div className="flex-shrink-0 flex gap-5 pt-2">
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums leading-tight">{totalSessions}</p>
          <p className="text-[10px] text-gray-400 uppercase tracking-wider mt-0.5">sessions</p>
        </div>
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums leading-tight">{activeDays}</p>
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

/** Custom tooltip matching the existing dark style */
function CustomTooltip({ active, payload }: TooltipProps<number, string>) {
  if (!active || !payload?.[0]) return null
  const datum = payload[0].payload as DayDatum
  return (
    <div className="px-2.5 py-1.5 bg-gray-900 text-white text-[11px] rounded-md shadow-lg whitespace-nowrap tabular-nums">
      {datum.label}
      {' — '}
      <span className="font-semibold">{datum.count}</span>
      {' session'}{datum.count !== 1 ? 's' : ''}
    </div>
  )
}
