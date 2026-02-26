import { Zap, ArrowRight, BarChart3 } from 'lucide-react'
import type { HeroInsightData } from '../../hooks/use-insights'

interface HeroInsightProps {
  insight: HeroInsightData | null
  isLoading: boolean
  onViewDetails?: () => void
}

function HeroInsightSkeleton() {
  return (
    <div
      role="status"
      aria-label="Loading insight"
      className="bg-gradient-to-r from-blue-50 to-blue-100 dark:from-blue-900/20 dark:to-blue-800/20 rounded-xl border border-blue-200 dark:border-blue-800 p-6"
    >
      <div className="animate-pulse">
        <div className="flex items-center gap-2 mb-3">
          <div className="w-4 h-4 bg-blue-200 dark:bg-blue-700 rounded" />
          <div className="h-3 w-24 bg-blue-200 dark:bg-blue-700 rounded" />
        </div>
        <div className="h-6 w-3/4 bg-blue-200 dark:bg-blue-700 rounded mb-2" />
        <div className="h-4 w-full bg-blue-200 dark:bg-blue-700 rounded mb-1" />
        <div className="h-4 w-2/3 bg-blue-200 dark:bg-blue-700 rounded mb-4" />
        <div className="h-3 w-32 bg-blue-200 dark:bg-blue-700 rounded" />
      </div>
    </div>
  )
}

function HeroInsightEmpty() {
  return (
    <div className="bg-gradient-to-r from-gray-50 to-gray-100 dark:from-gray-800/40 dark:to-gray-800/20 rounded-xl border border-gray-200 dark:border-gray-700 p-6 text-center">
      <BarChart3 className="w-8 h-8 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
      <p className="text-sm font-medium text-gray-500 dark:text-gray-400">
        Not enough data yet
      </p>
      <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
        Keep using Claude Code and insights will appear as patterns emerge.
      </p>
    </div>
  )
}

export function HeroInsight({ insight, isLoading, onViewDetails }: HeroInsightProps) {
  if (isLoading) {
    return <HeroInsightSkeleton />
  }

  if (!insight) {
    return <HeroInsightEmpty />
  }

  return (
    <div className="bg-gradient-to-r from-blue-50 to-blue-100 dark:from-blue-900/20 dark:to-blue-800/20 rounded-xl border border-blue-200 dark:border-blue-800 p-6">
      <div className="flex items-center gap-2 mb-3">
        <Zap className="w-4 h-4 text-amber-500" />
        <span className="text-xs font-semibold text-blue-600 dark:text-blue-400 uppercase tracking-wider">
          Your #1 Insight
        </span>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-2">
        {insight.title}
      </h2>

      <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
        {insight.description}
      </p>

      <div className="flex items-center justify-between">
        <span className="text-xs text-gray-500 dark:text-gray-500">
          Based on {insight.sampleSize} sessions
        </span>

        {onViewDetails && (
          <button
            onClick={onViewDetails}
            className="inline-flex items-center gap-1 text-sm font-medium text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 transition-colors cursor-pointer"
          >
            View Details
            <ArrowRight className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  )
}
