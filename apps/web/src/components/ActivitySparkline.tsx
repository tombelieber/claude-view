// src/components/ActivitySparkline.tsx
// Self-contained area chart — fetches activity data from /api/sessions/activity

import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { chartFontSize } from '@claude-view/design-tokens'
import { useTheme } from '../hooks/use-theme'

interface ActivityPoint {
  date: string
  count: number
}

interface ChartDatum {
  date: number // timestamp ms for XAxis
  count: number
  label: string // formatted for tooltip
}

export function ActivitySparkline() {
  const { resolvedTheme } = useTheme()
  const isDark = resolvedTheme === 'dark'

  const { data } = useQuery({
    queryKey: ['session-activity'],
    queryFn: async () => {
      const res = await fetch('/api/sessions/activity')
      if (!res.ok) throw new Error('Failed to fetch activity')
      return res.json() as Promise<{ activity: ActivityPoint[]; bucket: string; total: number }>
    },
    staleTime: 60_000,
  })

  const chartData = useMemo((): ChartDatum[] => {
    if (!data?.activity) return []
    return data.activity.map((pt) => {
      const ts = pt.date.includes('W') ? parseWeekDate(pt.date) : new Date(pt.date).getTime()
      return {
        date: ts,
        count: pt.count,
        label: formatBucketLabel(pt.date, data.bucket),
      }
    })
  }, [data])

  // Use server-provided total (includes sessions with no timestamp that can't
  // appear on the chart axis) so it matches the list's "N sessions" count.
  const totalSessions = data?.total ?? chartData.reduce((sum, d) => sum + d.count, 0)

  const activeDays = useMemo(() => chartData.filter((d) => d.count > 0).length, [chartData])

  if (chartData.length === 0) return null

  // Compute nice Y-axis ticks — aim for 4-6 ticks regardless of magnitude
  const maxCount = Math.max(...chartData.map((d) => d.count), 1)
  const yTicks = (() => {
    if (maxCount <= 1) return [0, 1]
    if (maxCount <= 4) return Array.from({ length: maxCount + 1 }, (_, i) => i)
    // "Nice numbers" step: pick a round step that yields ~5 ticks
    const rawStep = maxCount / 5
    const magnitude = Math.pow(10, Math.floor(Math.log10(rawStep)))
    const normalized = rawStep / magnitude
    const step =
      (normalized <= 1.5 ? 1 : normalized <= 3 ? 2 : normalized <= 7 ? 5 : 10) * magnitude
    const ceil = Math.ceil(maxCount / step) * step
    const ticks = [0]
    for (let v = step; v <= ceil; v += step) ticks.push(v)
    return ticks
  })()
  const yDomain: [number, number] = [0, yTicks[yTicks.length - 1]]
  // Size Y-axis width to fit the widest tick label (~7px per digit at axisTick size)
  const yAxisWidth = Math.max(28, String(yTicks[yTicks.length - 1]).length * 8 + 8)

  // Compute smart X-axis ticks
  const xTicks = (() => {
    if (chartData.length === 0) return []
    const totalPoints = chartData.length
    const targetCount = Math.min(8, totalPoints)
    const step = Math.max(1, Math.floor(totalPoints / targetCount))
    const ticks: number[] = []
    for (let i = 0; i < totalPoints; i += step) {
      ticks.push(chartData[i].date)
    }
    const last = chartData[totalPoints - 1].date
    if (ticks[ticks.length - 1] !== last) ticks.push(last)
    return ticks
  })()

  const formatXTick = (ts: number) => {
    const d = new Date(ts)
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }

  return (
    <div className="flex items-start gap-6">
      {/* Chart area */}
      <div className="flex-1 min-w-0">
        <ResponsiveContainer width="100%" height={140}>
          <AreaChart data={chartData} margin={{ top: 8, right: 12, bottom: 0, left: 0 }}>
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
              tick={{ fontSize: chartFontSize.axisTick, fill: '#9ca3af' }}
              axisLine={{ stroke: isDark ? '#4b5563' : '#e5e7eb' }}
              tickLine={{ stroke: isDark ? '#4b5563' : '#e5e7eb', strokeWidth: 1 }}
              tickMargin={6}
            />

            <YAxis
              domain={yDomain}
              ticks={yTicks}
              tick={{ fontSize: chartFontSize.axisTick, fill: '#9ca3af' }}
              axisLine={false}
              tickLine={false}
              tickMargin={4}
              allowDecimals={false}
              width={yAxisWidth}
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
              dot={false}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>

      {/* Quick stats */}
      <div className="flex-shrink-0 flex gap-5 pt-2">
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums leading-tight">
            {totalSessions}
          </p>
          <p className="text-xs text-gray-400 uppercase tracking-wider mt-0.5">sessions</p>
        </div>
        <div className="text-right">
          <p className="text-xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums leading-tight">
            {activeDays}
          </p>
          <p className="text-xs text-gray-400 uppercase tracking-wider mt-0.5">
            {data?.bucket === 'week'
              ? 'active weeks'
              : data?.bucket === 'month'
                ? 'active months'
                : 'active days'}
          </p>
        </div>
      </div>
    </div>
  )
}

function parseWeekDate(weekStr: string): number {
  const [year, w] = weekStr.split('-W').map(Number)
  const jan1 = new Date(year, 0, 1)
  return jan1.getTime() + (w - 1) * 7 * 86400000
}

function formatBucketLabel(date: string, bucket: string): string {
  if (bucket === 'month') {
    const d = new Date(date + '-01')
    return d.toLocaleDateString('en-US', { month: 'short', year: 'numeric' })
  }
  if (bucket === 'week') return date.replace('-W', ' W')
  const d = new Date(date)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** Custom tooltip matching the existing dark style */
function CustomTooltip({
  active,
  payload,
}: { active?: boolean; payload?: Array<{ payload: ChartDatum }> }) {
  if (!active || !payload?.[0]) return null
  const datum = payload[0].payload
  return (
    <div className="px-2.5 py-1.5 bg-gray-900 text-white text-xs rounded-md shadow-lg whitespace-nowrap tabular-nums">
      {datum.label}
      {' — '}
      <span className="font-semibold">{datum.count}</span>
      {' session'}
      {datum.count !== 1 ? 's' : ''}
    </div>
  )
}
