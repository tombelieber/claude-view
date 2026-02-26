import { DollarSign } from 'lucide-react'
import { MetricCard, StackedBar } from './ui'
import type { StackedBarSegment } from './ui/StackedBar'
import type { AggregateCostBreakdown } from '../types/generated/AggregateCostBreakdown'
import { formatCostUsd } from '../lib/format-utils'

interface CostBreakdownCardProps {
  cost: AggregateCostBreakdown
}

const SEGMENTS: Array<{
  key: keyof Pick<AggregateCostBreakdown, 'cacheReadCostUsd' | 'cacheCreationCostUsd' | 'outputCostUsd' | 'inputCostUsd'>
  label: string
  cardLabel: string
  color: string
  darkColor: string
}> = [
  { key: 'cacheReadCostUsd', label: 'Cache Read', cardLabel: 'Cache Read', color: 'bg-emerald-500', darkColor: 'dark:bg-emerald-400' },
  { key: 'cacheCreationCostUsd', label: 'Cache Write', cardLabel: 'Cache Write', color: 'bg-amber-500', darkColor: 'dark:bg-amber-400' },
  { key: 'outputCostUsd', label: 'Output', cardLabel: 'Output', color: 'bg-blue-600', darkColor: 'dark:bg-blue-400' },
  { key: 'inputCostUsd', label: 'Fresh Input', cardLabel: 'Fresh Input', color: 'bg-gray-400', darkColor: 'dark:bg-gray-500' },
]


export function CostBreakdownCard({ cost }: CostBreakdownCardProps) {
  if (cost.totalCostUsd === 0) return null

  const segments: StackedBarSegment[] = SEGMENTS.map((s) => ({
    label: s.label,
    value: cost[s.key],
    color: s.color,
    darkColor: s.darkColor,
  }))

  return (
    <div className="space-y-3">
      {/* Hero card with stacked bar */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2 flex items-center gap-1.5">
          <DollarSign className="w-3.5 h-3.5" />
          Estimated Total Cost
        </p>
        <p className="text-3xl sm:text-4xl font-semibold text-blue-800 dark:text-blue-300 tabular-nums mb-4">
          {formatCostUsd(cost.totalCostUsd)}
        </p>
        <StackedBar segments={segments} />

        {/* Cache savings callout */}
        {cost.cacheSavingsUsd > 0 && (
          <p className="mt-3 text-sm text-emerald-600 dark:text-emerald-400">
            Saved {formatCostUsd(cost.cacheSavingsUsd)} via prompt caching
          </p>
        )}
      </div>

      {/* Detail cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        {SEGMENTS.map((s) => {
          const value = cost[s.key]
          const pct = cost.totalCostUsd > 0 ? ((value / cost.totalCostUsd) * 100).toFixed(1) : '0.0'
          return (
            <MetricCard
              key={s.key}
              label={s.cardLabel}
              value={formatCostUsd(value)}
              subValue={`${pct}% of total`}
            />
          )
        })}
      </div>
    </div>
  )
}
