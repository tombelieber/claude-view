import { Activity, FileCode2, Target, TrendingUp, TrendingDown } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatNumber } from '../../lib/format-utils'
import { InsightLineCompact } from './InsightLine'
import { MetricTooltip } from './MetricTooltip'
import type { OverviewMetrics } from '../../types/generated'

interface OverviewCardsProps {
  overview: OverviewMetrics
}

/**
 * OverviewCards displays the three pillars: Fluency, Output, Effectiveness.
 */
export function OverviewCards({ overview }: OverviewCardsProps) {
  const { fluency, output, effectiveness } = overview

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
      {/* Fluency Card */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
        <div className="flex items-center gap-2 mb-3">
          <Activity className="w-4 h-4 text-blue-500" aria-hidden="true" />
          <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            Fluency
          </h3>
        </div>

        <div className="space-y-3">
          <div>
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
                {fluency.sessions}
              </span>
              <span className="text-sm text-gray-500 dark:text-gray-400">sessions</span>
            </div>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {fluency.promptsPerSession.toFixed(1)} prompts/session avg
            </p>
          </div>

          {fluency.trend !== null && (
            <TrendBadge value={fluency.trend} label="vs last period" />
          )}

          <InsightLineCompact insight={fluency.insight} className="pt-2 border-t border-gray-100 dark:border-gray-800" />
        </div>
      </div>

      {/* Output Card */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
        <div className="flex items-center gap-2 mb-3">
          <FileCode2 className="w-4 h-4 text-green-500" aria-hidden="true" />
          <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            AI Output
          </h3>
        </div>

        <div className="space-y-3">
          <div>
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-semibold text-green-600 dark:text-green-400 tabular-nums">
                +{formatNumber(output.linesAdded)}
              </span>
              <span className="text-lg font-medium text-red-500 dark:text-red-400 tabular-nums">
                -{formatNumber(output.linesRemoved)}
              </span>
              <span className="text-sm text-gray-500 dark:text-gray-400">lines</span>
            </div>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {output.filesCount} files, {output.commitsCount} commits
            </p>
          </div>

          <InsightLineCompact insight={output.insight} className="pt-2 border-t border-gray-100 dark:border-gray-800" />
        </div>
      </div>

      {/* Effectiveness Card */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
        <div className="flex items-center gap-2 mb-3">
          <Target className="w-4 h-4 text-purple-500" aria-hidden="true" />
          <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            Effectiveness
          </h3>
        </div>

        <div className="space-y-3">
          <div>
            <div className="flex items-baseline gap-2">
              <span className="text-2xl font-semibold text-gray-900 dark:text-gray-100 tabular-nums">
                {effectiveness.commitRate !== null ? `${(effectiveness.commitRate * 100).toFixed(0)}%` : '--'}
              </span>
              <span className="text-sm text-gray-500 dark:text-gray-400">committed</span>
            </div>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {effectiveness.reeditRate !== null
                ? (
                  <span className="inline-flex items-center">
                    {effectiveness.reeditRate.toFixed(2)} re-edit rate
                    <MetricTooltip>
                      <span className="font-medium text-gray-900 dark:text-gray-100">Re-edit rate</span> measures how often AI-generated files need further editing after the initial write.
                      <br /><br />
                      <span className="font-medium text-gray-900 dark:text-gray-100">Lower is better.</span> 0 = no re-edits needed.
                      <br /><br />
                      Formula: files re-edited / total files edited
                    </MetricTooltip>
                  </span>
                )
                : 'Re-edit rate unavailable'}
            </p>
          </div>

          <InsightLineCompact insight={effectiveness.insight} className="pt-2 border-t border-gray-100 dark:border-gray-800" />
        </div>
      </div>
    </div>
  )
}

/**
 * TrendBadge shows a trend indicator with percentage change.
 */
function TrendBadge({ value, label }: { value: number; label: string }) {
  const isPositive = value > 0
  const isNeutral = value === 0

  return (
    <div
      className={cn(
        'inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium',
        isNeutral
          ? 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
          : isPositive
            ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
            : 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400'
      )}
    >
      {isPositive ? (
        <TrendingUp className="w-3 h-3" aria-hidden="true" />
      ) : (
        <TrendingDown className="w-3 h-3" aria-hidden="true" />
      )}
      <span className="tabular-nums">
        {isPositive ? '+' : ''}{value.toFixed(0)}%
      </span>
      <span className="text-gray-500 dark:text-gray-400">{label}</span>
    </div>
  )
}

