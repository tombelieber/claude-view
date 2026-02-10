import { useState } from 'react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts'
import { cn } from '../../lib/utils'
import { InsightLine } from './InsightLine'
import type { DailyTrendPoint, Insight } from '../../types/generated'

interface TrendChartProps {
  data: DailyTrendPoint[]
  insight?: Insight
}

type ChartMetric = 'lines' | 'commits' | 'sessions'

const METRIC_OPTIONS: { value: ChartMetric; label: string }[] = [
  { value: 'lines', label: 'Lines' },
  { value: 'commits', label: 'Commits' },
  { value: 'sessions', label: 'Sessions' },
]

/**
 * TrendChart displays contribution trends over time.
 */
export function TrendChart({ data, insight }: TrendChartProps) {
  const [metric, setMetric] = useState<ChartMetric>('lines')

  // Transform data for the selected metric
  const chartData = data.map((point) => ({
    date: formatDate(point.date),
    fullDate: point.date,
    linesAdded: point.linesAdded,
    linesRemoved: point.linesRemoved,
    net: point.linesAdded - point.linesRemoved,
    commits: point.commits,
    sessions: point.sessions,
  }))

  // Find max value for Y axis scaling
  const getMaxValue = () => {
    switch (metric) {
      case 'lines':
        return Math.max(...chartData.map((d) => Math.max(d.linesAdded, d.linesRemoved)))
      case 'commits':
        return Math.max(...chartData.map((d) => d.commits))
      case 'sessions':
        return Math.max(...chartData.map((d) => d.sessions))
    }
  }

  const maxValue = getMaxValue() || 1

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          Contribution Trend
        </h2>

        {/* Metric Toggle */}
        <div className="flex rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
          {METRIC_OPTIONS.map((option) => (
            <button
              key={option.value}
              onClick={() => setMetric(option.value)}
              className={cn(
                'px-3 py-1 text-xs font-medium transition-colors cursor-pointer',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-inset',
                option.value === metric
                  ? 'bg-blue-500 text-white'
                  : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'
              )}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      {/* Chart */}
      <div className="h-64">
        {data.length === 0 ? (
          <div className="h-full flex items-center justify-center text-gray-400 text-sm">
            No trend data available for this period
          </div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData} margin={{ top: 5, right: 20, left: 0, bottom: 5 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid, #e5e7eb)" />
              <XAxis
                dataKey="date"
                tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
                tickLine={false}
                axisLine={{ stroke: 'var(--chart-axis, #d1d5db)' }}
              />
              <YAxis
                domain={[0, Math.ceil(maxValue * 1.1)]}
                tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
                tickLine={false}
                axisLine={{ stroke: 'var(--chart-axis, #d1d5db)' }}
                tickFormatter={formatYAxisValue}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: 'var(--tooltip-bg, #fff)',
                  border: '1px solid var(--tooltip-border, #e5e7eb)',
                  borderRadius: '8px',
                  fontSize: '12px',
                }}
                labelFormatter={(label) => {
                  const point = chartData.find((d) => d.date === label)
                  return point?.fullDate || label
                }}
              />
              <Legend
                wrapperStyle={{ fontSize: '12px', paddingTop: '10px' }}
                iconType="line"
              />

              {metric === 'lines' && (
                <>
                  <Line
                    type="monotone"
                    dataKey="linesAdded"
                    name="Added"
                    stroke="#22c55e"
                    strokeWidth={2}
                    dot={{ r: 3 }}
                    activeDot={{ r: 5 }}
                  />
                  <Line
                    type="monotone"
                    dataKey="linesRemoved"
                    name="Removed"
                    stroke="#ef4444"
                    strokeWidth={2}
                    dot={{ r: 3 }}
                    activeDot={{ r: 5 }}
                  />
                  <Line
                    type="monotone"
                    dataKey="net"
                    name="Net"
                    stroke="#3b82f6"
                    strokeWidth={2}
                    strokeDasharray="5 5"
                    dot={{ r: 2 }}
                  />
                </>
              )}

              {metric === 'commits' && (
                <Line
                  type="monotone"
                  dataKey="commits"
                  name="Commits"
                  stroke="#8b5cf6"
                  strokeWidth={2}
                  dot={{ r: 3 }}
                  activeDot={{ r: 5 }}
                />
              )}

              {metric === 'sessions' && (
                <Line
                  type="monotone"
                  dataKey="sessions"
                  name="Sessions"
                  stroke="#f59e0b"
                  strokeWidth={2}
                  dot={{ r: 3 }}
                  activeDot={{ r: 5 }}
                />
              )}
            </LineChart>
          </ResponsiveContainer>
        )}
      </div>

      {/* Insight */}
      {insight && <InsightLine insight={insight} className="mt-4" />}
    </div>
  )
}

/**
 * Format date for X axis (Mon, Tue, etc. or Jan 1, etc.)
 */
function formatDate(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diffDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24))

  if (diffDays < 7) {
    return date.toLocaleDateString('en-US', { weekday: 'short' })
  }
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/**
 * Format Y axis values (K suffix for thousands)
 */
function formatYAxisValue(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(0)}K`
  return value.toString()
}
