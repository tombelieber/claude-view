import { BarChart3, Gauge, Clock } from 'lucide-react'
import { QuickStatCard } from './QuickStatCard'
import { cn } from '../../lib/utils'
import type { WorkBreakdownData, EfficiencyData, PatternStatsData } from '../../hooks/use-insights'

interface QuickStatsRowProps {
  workBreakdown: WorkBreakdownData | null
  efficiency: EfficiencyData | null
  patterns: PatternStatsData | null
  isLoading: boolean
}

function TrendIndicator({
  direction,
  value,
}: {
  direction: 'improving' | 'stable' | 'declining'
  value: number
}) {
  const color =
    direction === 'improving'
      ? 'text-green-600 dark:text-green-400'
      : direction === 'declining'
        ? 'text-red-600 dark:text-red-400'
        : 'text-gray-500 dark:text-gray-400'

  const arrow =
    direction === 'improving' ? '\u2193' : direction === 'declining' ? '\u2191' : '\u2192'

  const label =
    direction === 'improving' ? 'improving' : direction === 'declining' ? 'declining' : 'stable'

  return (
    <span className={cn('text-xs font-medium', color)}>
      {arrow} {Math.abs(value).toFixed(0)}% {label}
    </span>
  )
}

export function QuickStatsRow({
  workBreakdown,
  efficiency,
  patterns,
  isLoading,
}: QuickStatsRowProps) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
      {/* Sessions Card */}
      <QuickStatCard
        title="Sessions"
        icon={<BarChart3 className="w-4 h-4" />}
        isLoading={isLoading}
      >
        {workBreakdown && (
          <div>
            <div className="text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">
              {workBreakdown.total}
            </div>
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-3">sessions</div>
            <div className="space-y-1 text-xs text-gray-500 dark:text-gray-400">
              <div>{workBreakdown.withCommits} committed</div>
              <div>{workBreakdown.exploration} exploration</div>
              <div>~{Math.round(workBreakdown.avgMinutes)} min avg</div>
            </div>
          </div>
        )}
      </QuickStatCard>

      {/* Efficiency Card */}
      <QuickStatCard
        title="Efficiency"
        icon={<Gauge className="w-4 h-4" />}
        isLoading={isLoading}
      >
        {efficiency && (
          <div>
            <div className="text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">
              {efficiency.editsPerFile.toFixed(1)}
            </div>
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-3">edits/file</div>
            <div className="space-y-1">
              <TrendIndicator
                direction={efficiency.trendDirection}
                value={efficiency.trend}
              />
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {efficiency.reeditRate.toFixed(2)} re-edit rate
              </div>
            </div>
          </div>
        )}
      </QuickStatCard>

      {/* Peak Time Card */}
      <QuickStatCard
        title="Peak Time"
        icon={<Clock className="w-4 h-4" />}
        isLoading={isLoading}
      >
        {patterns ? (
          <div>
            <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">
              {patterns.bestDay || '--'}
            </div>
            <div className="text-xs text-gray-500 dark:text-gray-400 mb-3">
              {patterns.bestTime || '--'}
            </div>
            {patterns.improvementPct > 0 && (
              <div className="text-xs text-green-600 dark:text-green-400 font-medium">
                {Math.round(patterns.improvementPct)}% more efficient
              </div>
            )}
          </div>
        ) : (
          <div className="text-sm text-gray-400 dark:text-gray-500">
            Not enough data to determine peak time
          </div>
        )}
      </QuickStatCard>
    </div>
  )
}
