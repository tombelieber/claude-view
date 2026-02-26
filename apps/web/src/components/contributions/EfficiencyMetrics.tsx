import { useState } from 'react'
import { DollarSign, ArrowRight } from 'lucide-react'
import { formatNumber, formatCostUsd } from '../../lib/format-utils'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
} from 'recharts'
import { cn } from '../../lib/utils'
import { InsightLine } from './InsightLine'
import type { EfficiencyMetrics as EfficiencyMetricsType } from '../../types/generated'
import type { DailyTrendPoint } from '../../types/generated'

type CostMetric = 'commit' | 'session' | 'line'

const METRIC_OPTIONS: { value: CostMetric; label: string }[] = [
  { value: 'commit', label: 'Per Commit' },
  { value: 'session', label: 'Per Session' },
  { value: 'line', label: 'Per Line' },
]

interface EfficiencyMetricsSectionProps {
  efficiency: EfficiencyMetricsType
  trendData?: DailyTrendPoint[]
  sessionCount: number
  commitsCount: number
}

/**
 * EfficiencyMetrics displays ROI metrics with a toggleable "cost per X" view.
 *
 * Toggle options: per Commit, per Session, per Line.
 * Chart updates to show the selected cost metric over time.
 */
export function EfficiencyMetricsSection({
  efficiency,
  trendData,
  sessionCount,
  commitsCount,
}: EfficiencyMetricsSectionProps) {
  const [metric, setMetric] = useState<CostMetric>('commit')

  const {
    totalCost,
    totalLines,
    costPerLine,
    costPerCommit,
    costIsEstimated,
    insight,
  } = efficiency

  const costPerSession = sessionCount > 0 ? totalCost / sessionCount : null

  // Hero metric based on toggle
  const heroValue = (() => {
    switch (metric) {
      case 'commit':
        return costPerCommit !== null ? formatCostUsd(costPerCommit) : '--'
      case 'session':
        return costPerSession !== null ? formatCostUsd(costPerSession) : '--'
      case 'line':
        return costPerLine !== null ? formatCostUsd(costPerLine) : '--'
    }
  })()

  const heroLabel = (() => {
    switch (metric) {
      case 'commit': return 'per commit'
      case 'session': return 'per session'
      case 'line': return 'per line of AI output'
    }
  })()

  const heroDenominator = (() => {
    switch (metric) {
      case 'commit': return commitsCount > 0 ? `${commitsCount} commits` : null
      case 'session': return sessionCount > 0 ? `${sessionCount} sessions` : null
      case 'line': return totalLines > 0 ? `${formatNumber(totalLines)} lines` : null
    }
  })()

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      {/* Header with Toggle */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <DollarSign className="w-4 h-4 text-emerald-500" aria-hidden="true" />
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            Efficiency
          </h2>
        </div>

        {/* Metric Toggle (matches TrendChart pattern) */}
        <div className="flex rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
          {METRIC_OPTIONS.map((option) => (
            <button
              key={option.value}
              onClick={() => setMetric(option.value)}
              className={cn(
                'px-3 py-1 text-xs font-medium transition-colors cursor-pointer',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-inset',
                option.value === metric
                  ? 'bg-emerald-500 text-white'
                  : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'
              )}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      {/* Summary Flow */}
      <div className="flex flex-wrap items-center gap-2 text-lg mb-4">
        <span className="font-semibold text-gray-900 dark:text-gray-100">
          {formatCostUsd(totalCost)} spent
          {costIsEstimated && (
            <span className="text-xs font-normal text-gray-400 dark:text-gray-500 ml-1">(estimated)</span>
          )}
        </span>
        <ArrowRight className="w-4 h-4 text-gray-400" aria-hidden="true" />
        <span className="font-semibold text-gray-900 dark:text-gray-100">
          {heroDenominator ?? '--'}
        </span>
      </div>

      {/* Hero Metric */}
      <div className="flex items-baseline gap-2 mb-4">
        <span className="text-3xl font-bold text-emerald-600 dark:text-emerald-400 tabular-nums">
          {heroValue}
        </span>
        <span className="text-sm text-gray-500 dark:text-gray-400">{heroLabel}</span>
      </div>

      {/* Secondary Metrics (show the other two) */}
      <div className="flex flex-wrap gap-6 text-sm text-gray-600 dark:text-gray-400 mb-4">
        {metric !== 'commit' && (
          <div>
            <span className="font-medium">Per commit:</span>{' '}
            <span className="text-gray-900 dark:text-gray-100 tabular-nums">
              {costPerCommit !== null ? formatCostUsd(costPerCommit) : '--'}
            </span>
          </div>
        )}
        {metric !== 'session' && (
          <div>
            <span className="font-medium">Per session:</span>{' '}
            <span className="text-gray-900 dark:text-gray-100 tabular-nums">
              {costPerSession !== null ? formatCostUsd(costPerSession) : '--'}
            </span>
          </div>
        )}
        {metric !== 'line' && (
          <div>
            <span className="font-medium">Per line:</span>{' '}
            <span className="text-gray-900 dark:text-gray-100 tabular-nums">
              {costPerLine !== null ? formatCostUsd(costPerLine) : '--'}
            </span>
          </div>
        )}
      </div>

      {/* Cost Trend Chart (adapts to selected metric) */}
      {trendData && trendData.length > 1 && (
        <CostTrendChart trendData={trendData} metric={metric} />
      )}

      {/* Insight */}
      <InsightLine insight={insight} />
    </div>
  )
}

/**
 * Cost trend chart that adapts to the selected cost metric.
 */
function CostTrendChart({
  trendData,
  metric,
}: {
  trendData: DailyTrendPoint[]
  metric: CostMetric
}) {
  const chartData = trendData
    .filter(d => {
      switch (metric) {
        case 'line': return (d.linesAdded + d.linesRemoved) > 0
        case 'commit': return d.commits > 0
        case 'session': return d.sessions > 0
      }
    })
    .map(d => {
      const cost = d.costCents / 100
      let value: number
      switch (metric) {
        case 'line':
          value = cost / Math.max(d.linesAdded + d.linesRemoved, 1)
          break
        case 'commit':
          value = cost / Math.max(d.commits, 1)
          break
        case 'session':
          value = cost / Math.max(d.sessions, 1)
          break
      }
      return { date: formatChartDate(d.date), value }
    })

  if (chartData.length < 2) return null

  const maxVal = Math.max(...chartData.map(d => d.value))
  const precision = metric === 'line'
    ? (maxVal >= 0.1 ? 2 : maxVal >= 0.01 ? 3 : 4)
    : 2

  const chartLabel = (() => {
    switch (metric) {
      case 'commit': return 'Cost per Commit'
      case 'session': return 'Cost per Session'
      case 'line': return 'Cost per Line'
    }
  })()

  return (
    <div className="mb-4">
      <p className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-2">
        {chartLabel} Over Time
      </p>
      <div className="h-40">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={chartData} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
            <defs>
              <linearGradient id="costGradient" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#22c55e" stopOpacity={0.2} />
                <stop offset="95%" stopColor="#22c55e" stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--chart-grid, #e5e7eb)" />
            <XAxis
              dataKey="date"
              tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
              tickLine={false}
              axisLine={{ stroke: 'var(--chart-axis, #d1d5db)' }}
            />
            <YAxis
              tick={{ fontSize: 11, fill: 'var(--chart-text, #6b7280)' }}
              tickLine={false}
              axisLine={{ stroke: 'var(--chart-axis, #d1d5db)' }}
              tickFormatter={(v: number) => formatCostUsd(v)}
            />
            <RechartsTooltip
              contentStyle={{
                backgroundColor: 'var(--tooltip-bg, #fff)',
                border: '1px solid var(--tooltip-border, #e5e7eb)',
                borderRadius: '8px',
                fontSize: '12px',
              }}
              formatter={(value: number) => [formatCostUsd(value), chartLabel]}
            />
            <Area
              type="monotone"
              dataKey="value"
              stroke="#22c55e"
              strokeWidth={2}
              fill="url(#costGradient)"
              dot={{ r: 3, fill: '#22c55e' }}
              activeDot={{ r: 5 }}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}

/**
 * Format date for chart X-axis labels.
 * Shows weekday name for recent dates, month+day for older ones.
 */
function formatChartDate(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diffDays = Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24))
  if (diffDays < 7) {
    return date.toLocaleDateString('en-US', { weekday: 'short' })
  }
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

