import { Zap } from 'lucide-react'
import { MetricCard, StackedBar } from './ui'
import type { StackedBarSegment } from './ui/StackedBar'
import { formatTokens } from '../hooks/use-ai-generation'

interface TokenBreakdownProps {
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
}

const SEGMENTS: Array<{ key: keyof TokenBreakdownProps; label: string; cardLabel: string; color: string; darkColor: string }> = [
  { key: 'cacheReadTokens', label: 'Cache Read', cardLabel: 'Cache Read', color: 'bg-emerald-500', darkColor: 'dark:bg-emerald-400' },
  { key: 'cacheCreationTokens', label: 'Cache Write', cardLabel: 'Cache Write', color: 'bg-amber-500', darkColor: 'dark:bg-amber-400' },
  { key: 'totalOutputTokens', label: 'Output', cardLabel: 'Output', color: 'bg-blue-600', darkColor: 'dark:bg-blue-400' },
  { key: 'totalInputTokens', label: 'Fresh Input', cardLabel: 'Fresh Input', color: 'bg-gray-400', darkColor: 'dark:bg-gray-500' },
]

export function TokenBreakdown(props: TokenBreakdownProps) {
  const grandTotal =
    props.totalInputTokens +
    props.totalOutputTokens +
    props.cacheReadTokens +
    props.cacheCreationTokens

  if (grandTotal === 0) return null

  const segments: StackedBarSegment[] = SEGMENTS.map((s) => ({
    label: s.label,
    value: props[s.key],
    color: s.color,
    darkColor: s.darkColor,
  }))

  return (
    <div className="space-y-3">
      {/* Hero card with stacked bar */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2 flex items-center gap-1.5">
          <Zap className="w-3.5 h-3.5" />
          Total Tokens Processed
        </p>
        <p className="text-3xl sm:text-4xl font-semibold text-blue-800 dark:text-blue-300 tabular-nums mb-4">
          {formatTokens(grandTotal)}
        </p>
        <StackedBar segments={segments} />
      </div>

      {/* Detail cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        {SEGMENTS.map((s) => {
          const value = props[s.key]
          const pct = grandTotal > 0 ? ((value / grandTotal) * 100).toFixed(1) : '0.0'
          return (
            <MetricCard
              key={s.key}
              label={s.cardLabel}
              value={formatTokens(value)}
              subValue={`${pct}% of total`}
            />
          )
        })}
      </div>
    </div>
  )
}
