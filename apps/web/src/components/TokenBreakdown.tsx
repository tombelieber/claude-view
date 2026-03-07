import { Zap } from 'lucide-react'
import { formatTokens } from '../hooks/use-ai-generation'
import { TOKEN_SEGMENT_CONFIG } from '../theme'
import { MetricCard, StackedBar } from './ui'
import type { StackedBarSegment } from './ui/StackedBar'

interface TokenBreakdownProps {
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
}

export function TokenBreakdown(props: TokenBreakdownProps) {
  const grandTotal =
    props.totalInputTokens +
    props.totalOutputTokens +
    props.cacheReadTokens +
    props.cacheCreationTokens

  if (grandTotal === 0) return null

  const segments: StackedBarSegment[] = TOKEN_SEGMENT_CONFIG.map((s) => ({
    label: s.label,
    value: props[s.key],
    color: s.color.light,
    darkColor: s.color.dark,
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
        {TOKEN_SEGMENT_CONFIG.map((s) => {
          const value = props[s.key]
          const pct = grandTotal > 0 ? ((value / grandTotal) * 100).toFixed(1) : '0.0'
          return (
            <MetricCard
              key={s.key}
              label={s.label}
              value={formatTokens(value)}
              subValue={`${pct}% of total`}
            />
          )
        })}
      </div>
    </div>
  )
}
