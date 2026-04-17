import { type TimeRangeParams, useAIGenerationStats } from '../hooks/use-ai-generation'
import { CostBreakdownCard } from './CostBreakdownCard'
import { TokenBreakdown } from './TokenBreakdown'

interface TokenCostSummaryProps {
  timeRange?: TimeRangeParams | null
  project?: string
  branch?: string
}

/**
 * Top-of-analytics hero section: tokens processed + total cost.
 *
 * Rendered directly under the "Your Claude Code Usage" header card so the
 * headline numbers sit above everything else. Shares the useAIGenerationStats
 * query key with AIGenerationStats below — TanStack dedupes, so it's one fetch.
 */
export function TokenCostSummary({ timeRange, project, branch }: TokenCostSummaryProps) {
  const { data: stats, isLoading } = useAIGenerationStats(timeRange, project, branch)

  if (isLoading) return <TokenCostSummarySkeleton />
  if (!stats) return null

  const hasTokenData = stats.totalInputTokens > 0 || stats.totalOutputTokens > 0
  const hasCostData = stats.cost ? stats.cost.totalCostUsd > 0 : false

  if (!hasTokenData && !hasCostData) return null

  return (
    <div className="space-y-4 sm:space-y-6">
      {hasTokenData && (
        <TokenBreakdown
          totalInputTokens={stats.totalInputTokens}
          totalOutputTokens={stats.totalOutputTokens}
          cacheReadTokens={stats.cacheReadTokens}
          cacheCreationTokens={stats.cacheCreationTokens}
        />
      )}

      {stats.cost && hasCostData && <CostBreakdownCard cost={stats.cost} />}
    </div>
  )
}

function TokenCostSummarySkeleton() {
  return (
    <div className="space-y-4 sm:space-y-6 animate-pulse">
      {[0, 1].map((group) => (
        <div key={group} className="space-y-3">
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
            <div className="h-3 w-40 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
            <div className="h-9 w-32 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
            <div className="h-2 w-full bg-gray-100 dark:bg-gray-800 rounded-full" />
          </div>
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            {[0, 1, 2, 3].map((i) => (
              <div
                key={i}
                className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4"
              >
                <div className="h-3 w-20 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
                <div className="h-7 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-1" />
                <div className="h-3 w-16 bg-gray-100 dark:bg-gray-800 rounded" />
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
