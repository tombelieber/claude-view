import { useMemo } from 'react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
} from 'recharts'
import { TrendingDown, TrendingUp, Minus } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { MetricDataPoint } from '../../types/generated/MetricDataPoint'
import type {
  TrendsMetric,
  TrendsGranularity,
} from '../../hooks/use-trends-data'
import {
  METRIC_OPTIONS,
  GRANULARITY_OPTIONS,
} from '../../hooks/use-trends-data'

// ============================================================================
// Types
// ============================================================================

interface TrendsChartProps {
  data: MetricDataPoint[]
  metric: TrendsMetric
  average: number
  trend: number
  trendDirection: string
  insight: string
  onMetricChange: (metric: TrendsMetric) => void
  granularity: TrendsGranularity
  onGranularityChange: (granularity: TrendsGranularity) => void
}

// ============================================================================
// Trend line calculation (linear regression)
// ============================================================================

function calculateTrendLine(data: MetricDataPoint[]): MetricDataPoint[] {
  if (data.length < 2) return data

  const n = data.length
  const sumX = data.reduce((sum, _, i) => sum + i, 0)
  const sumY = data.reduce((sum, d) => sum + d.value, 0)
  const sumXY = data.reduce((sum, d, i) => sum + i * d.value, 0)
  const sumXX = data.reduce((sum, _, i) => sum + i * i, 0)

  const denominator = n * sumXX - sumX * sumX
  if (denominator === 0) return data

  const slope = (n * sumXY - sumX * sumY) / denominator
  const intercept = (sumY - slope * sumX) / n

  return [
    { date: data[0].date, value: intercept },
    { date: data[n - 1].date, value: intercept + slope * (n - 1) },
  ]
}

// ============================================================================
// Helpers
// ============================================================================

function isLowerBetter(metric: TrendsMetric): boolean {
  return METRIC_OPTIONS.find((m) => m.value === metric)?.isLowerBetter ?? false
}

function formatValue(value: number, metric: TrendsMetric): string {
  if (metric === 'reedit_rate') return `${(value * 100).toFixed(0)}%`
  if (metric === 'cost_per_line') return `$${value.toFixed(3)}`
  if (metric === 'prompts') return value.toFixed(1)
  return Math.round(value).toLocaleString()
}

function formatDateLabel(date: string): string {
  // Handle both ISO dates (2026-01-15) and week formats (2026-W03)
  if (date.includes('W')) {
    const parts = date.split('-W')
    return `W${parts[1]}`
  }
  try {
    return new Date(date + 'T00:00:00').toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    })
  } catch {
    return date
  }
}

function formatDateTooltip(date: string): string {
  if (date.includes('W')) {
    const parts = date.split('-W')
    return `${parts[0]} Week ${parts[1]}`
  }
  try {
    return new Date(date + 'T00:00:00').toLocaleDateString('en-US', {
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    })
  } catch {
    return date
  }
}

// ============================================================================
// Component
// ============================================================================

export function TrendsChart({
  data,
  metric,
  average,
  trend,
  trendDirection,
  insight,
  onMetricChange,
  granularity,
  onGranularityChange,
}: TrendsChartProps) {
  const trendLine = useMemo(() => calculateTrendLine(data), [data])

  const lowerBetter = isLowerBetter(metric)

  const TrendIcon =
    trendDirection === 'stable'
      ? Minus
      : (trendDirection === 'improving') === lowerBetter
        ? TrendingDown
        : TrendingUp

  const trendColor =
    trendDirection === 'stable'
      ? 'text-gray-500'
      : trendDirection === 'improving'
        ? 'text-green-500'
        : 'text-red-500'

  // Merge data with trend line for Recharts
  const chartData = useMemo(() => {
    return data.map((d, i) => ({
      ...d,
      trendValue:
        trendLine.length === 2
          ? trendLine[0].value +
            ((trendLine[1].value - trendLine[0].value) * i) /
              Math.max(data.length - 1, 1)
          : undefined,
    }))
  }, [data, trendLine])

  if (data.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Efficiency Over Time
          </h3>
          <div className="flex items-center gap-2">
            <MetricSelector value={metric} onChange={onMetricChange} />
            <GranularitySelector value={granularity} onChange={onGranularityChange} />
          </div>
        </div>
        <div className="flex items-center justify-center py-16 text-gray-500 dark:text-gray-400 text-sm">
          No data for this period. Try expanding the time range.
        </div>
      </div>
    )
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-4">
        <div className="flex items-center gap-3">
          <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Efficiency Over Time
          </h3>
          <div className="flex items-center gap-1.5">
            <TrendIcon className={cn('h-4 w-4', trendColor)} />
            <span className={cn('text-sm font-medium', trendColor)}>
              {Math.abs(Math.round(trend))}%
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <MetricSelector value={metric} onChange={onMetricChange} />
          <GranularitySelector value={granularity} onChange={onGranularityChange} />
        </div>
      </div>

      <ResponsiveContainer width="100%" height={300}>
        <LineChart
          data={chartData}
          margin={{ top: 5, right: 30, left: 20, bottom: 5 }}
        >
          <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.2} />
          <XAxis
            dataKey="date"
            tickFormatter={formatDateLabel}
            stroke="#9CA3AF"
            fontSize={12}
            tickLine={false}
          />
          <YAxis
            tickFormatter={(v) => formatValue(v, metric)}
            stroke="#9CA3AF"
            fontSize={12}
            tickLine={false}
            width={55}
          />
          <Tooltip
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            formatter={((value: number) => [
              formatValue(value, metric),
              METRIC_OPTIONS.find((m) => m.value === metric)?.label ?? metric,
            ]) as any}
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            labelFormatter={formatDateTooltip as any}
            contentStyle={{
              backgroundColor: '#1F2937',
              border: 'none',
              borderRadius: '8px',
              color: '#F9FAFB',
              fontSize: '13px',
            }}
            itemStyle={{ color: '#F9FAFB' }}
            labelStyle={{ color: '#9CA3AF', marginBottom: '4px' }}
          />
          <ReferenceLine
            y={average}
            stroke="#9CA3AF"
            strokeDasharray="5 5"
            label={{
              value: `Avg: ${formatValue(average, metric)}`,
              position: 'right',
              fill: '#9CA3AF',
              fontSize: 11,
            }}
          />
          <Line
            type="monotone"
            dataKey="value"
            stroke="#3B82F6"
            strokeWidth={2}
            dot={{ fill: '#3B82F6', strokeWidth: 2, r: 3 }}
            activeDot={{ r: 5, stroke: '#3B82F6', strokeWidth: 2 }}
          />
          <Line
            type="linear"
            dataKey="trendValue"
            stroke="#F59E0B"
            strokeWidth={1}
            strokeDasharray="5 5"
            dot={false}
            activeDot={false}
            connectNulls
          />
        </LineChart>
      </ResponsiveContainer>

      <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
        <span className="shrink-0 mt-0.5 text-amber-500">*</span>
        <span>{insight}</span>
      </p>
    </div>
  )
}

// ============================================================================
// Metric Selector
// ============================================================================

function MetricSelector({
  value,
  onChange,
}: {
  value: TrendsMetric
  onChange: (metric: TrendsMetric) => void
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as TrendsMetric)}
      className="text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md px-2 py-1.5 text-gray-700 dark:text-gray-300 cursor-pointer focus:outline-none focus:ring-2 focus:ring-blue-500"
      aria-label="Select metric"
    >
      {METRIC_OPTIONS.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  )
}

// ============================================================================
// Granularity Selector
// ============================================================================

function GranularitySelector({
  value,
  onChange,
}: {
  value: TrendsGranularity
  onChange: (granularity: TrendsGranularity) => void
}) {
  return (
    <div className="inline-flex items-center gap-0.5 p-0.5 bg-gray-100 dark:bg-gray-800 rounded-md">
      {GRANULARITY_OPTIONS.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={cn(
            'px-2 py-1 text-xs font-medium rounded transition-all cursor-pointer',
            value === opt.value
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  )
}
