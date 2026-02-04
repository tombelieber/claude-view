import { Link } from 'react-router-dom'
import { TrendingUp, ArrowRight, GitBranch, Code2, RefreshCcw } from 'lucide-react'
import { useContributions } from '../hooks/use-contributions'
import { formatNumber, formatPercent } from '../lib/format-utils'
import { cn } from '../lib/utils'

export interface ContributionSummaryCardProps {
  /** Optional className for additional styling */
  className?: string
}

/**
 * ContributionSummaryCard displays a summary of AI contributions for the dashboard.
 *
 * Links to the full /contributions page for detailed analysis.
 *
 * Design:
 * - AI contribution progress bar (lines written / total)
 * - Key metrics: lines, commits, re-edit rate
 * - Insight line with trend comparison
 * - "View All" link to /contributions
 */
export function ContributionSummaryCard({ className }: ContributionSummaryCardProps) {
  const { data, isLoading, error } = useContributions('week')

  // Loading state
  if (isLoading) {
    return (
      <div className={cn(
        'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
        className
      )}>
        <div className="flex items-center justify-between mb-4">
          <div className="h-5 w-48 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
          <div className="h-4 w-20 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
        </div>
        <div className="h-3 w-full bg-gray-100 dark:bg-gray-800 rounded-full mb-4" />
        <div className="flex items-center gap-4">
          <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
          <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
          <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
        </div>
        <div className="mt-4 pt-4 border-t border-gray-100 dark:border-gray-800">
          <div className="h-4 w-64 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" />
        </div>
      </div>
    )
  }

  // Error state - show minimal card with link
  if (error || !data) {
    return (
      <Link
        to="/contributions"
        className={cn(
          'block bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
          'hover:border-gray-300 dark:hover:border-gray-600 transition-colors',
          className
        )}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <GitBranch className="w-5 h-5 text-[#7c9885]" />
            <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
              AI Contributions
            </h2>
          </div>
          <span className="flex items-center gap-1 text-xs text-gray-400 hover:text-blue-600 dark:hover:text-blue-400">
            View All <ArrowRight className="w-3.5 h-3.5" />
          </span>
        </div>
        <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
          {error ? 'Unable to load contribution data' : 'No contribution data available'}
        </p>
      </Link>
    )
  }

  // Extract metrics
  const { overview } = data
  const linesAdded = Number(overview.output.linesAdded)
  const linesRemoved = Number(overview.output.linesRemoved)
  const netLines = linesAdded - linesRemoved
  const commits = Number(overview.output.commitsCount)
  const reeditRate = overview.effectiveness.reeditRate
  const fluencyTrend = overview.fluency.trend

  // Calculate AI contribution percentage (simplified: based on commit rate)
  const commitRate = overview.effectiveness.commitRate
  const aiContributionPercent = commitRate !== null ? commitRate : 0

  // Determine insight text
  const insightText = overview.output.insight?.text || overview.fluency.insight?.text || ''

  return (
    <Link
      to="/contributions"
      className={cn(
        'block bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
        'hover:border-gray-300 dark:hover:border-gray-600 hover:shadow-sm transition-all',
        className
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <GitBranch className="w-5 h-5 text-[#7c9885]" />
          <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
            AI Contribution This Week
          </h2>
        </div>
        <span className="flex items-center gap-1 text-xs text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors">
          View All <ArrowRight className="w-3.5 h-3.5" />
        </span>
      </div>

      {/* Progress bar */}
      <div className="mb-4">
        <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400 mb-1.5">
          <span>AI-assisted commits</span>
          <span className="tabular-nums font-medium">
            {formatPercent(aiContributionPercent)}
          </span>
        </div>
        <div className="h-2.5 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
          <div
            className="h-full bg-[#7c9885] rounded-full transition-all duration-500"
            style={{ width: `${Math.min(aiContributionPercent, 100)}%` }}
          />
        </div>
      </div>

      {/* Metrics row */}
      <div className="flex items-center gap-4 text-sm">
        <div className="flex items-center gap-1.5 text-gray-600 dark:text-gray-400">
          <Code2 className="w-4 h-4" />
          <span className="tabular-nums font-medium text-gray-900 dark:text-gray-100">
            {netLines >= 0 ? '+' : ''}{formatNumber(netLines)}
          </span>
          <span>lines</span>
        </div>

        <span className="text-gray-300 dark:text-gray-600">|</span>

        <div className="flex items-center gap-1.5 text-gray-600 dark:text-gray-400">
          <GitBranch className="w-4 h-4" />
          <span className="tabular-nums font-medium text-gray-900 dark:text-gray-100">
            {commits}
          </span>
          <span>{commits === 1 ? 'commit' : 'commits'}</span>
        </div>

        {reeditRate !== null && (
          <>
            <span className="text-gray-300 dark:text-gray-600">|</span>
            <div className="flex items-center gap-1.5 text-gray-600 dark:text-gray-400">
              <RefreshCcw className="w-4 h-4" />
              <span className="tabular-nums font-medium text-gray-900 dark:text-gray-100">
                {formatPercent(reeditRate * 100)}
              </span>
              <span>re-edit</span>
            </div>
          </>
        )}
      </div>

      {/* Insight line */}
      {insightText && (
        <div className="mt-4 pt-4 border-t border-gray-100 dark:border-gray-800">
          <p className="text-sm text-gray-500 dark:text-gray-400 flex items-center gap-1.5">
            {fluencyTrend !== null && fluencyTrend > 0 && (
              <TrendingUp className="w-4 h-4 text-green-500 flex-shrink-0" />
            )}
            <span>{insightText}</span>
          </p>
        </div>
      )}
    </Link>
  )
}
