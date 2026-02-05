import { RefreshCw, AlertTriangle } from 'lucide-react'
import { useBenchmarks } from '../../hooks/use-benchmarks'
import type { TimeRange } from '../../hooks/use-insights'
import { ThenVsNow } from './ThenVsNow'
import { CategoryPerformanceTable } from './CategoryPerformanceTable'
import { SkillAdoptionImpact } from './SkillAdoptionImpact'
import { MonthlyReportGenerator } from './MonthlyReportGenerator'

// ============================================================================
// Types
// ============================================================================

interface BenchmarksTabProps {
  timeRange: TimeRange
}

// ============================================================================
// Time range mapping
// ============================================================================

/**
 * Map the Phase 5 TimeRange to the benchmarks API range parameter.
 */
function mapTimeRange(timeRange: TimeRange): 'all' | '30d' | '90d' | '1y' {
  switch (timeRange) {
    case '7d':
      return '30d' // 7d is too short for benchmarks; use 30d
    case '30d':
      return '30d'
    case '90d':
      return '90d'
    case 'all':
      return 'all'
  }
}

// ============================================================================
// Loading Skeleton
// ============================================================================

function BenchmarksSkeleton() {
  return (
    <div className="space-y-6 animate-pulse">
      {/* ThenVsNow skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-36 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="space-y-3">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="grid grid-cols-4 gap-4">
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
          ))}
        </div>
      </div>

      {/* Category table skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-48 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div key={i} className="grid grid-cols-4 gap-4">
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
          ))}
        </div>
      </div>

      {/* Skill adoption skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-44 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="space-y-3">
          {[1, 2].map((i) => (
            <div key={i} className="grid grid-cols-4 gap-4">
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
          ))}
        </div>
      </div>

      {/* Report skeleton */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="h-5 w-36 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
        <div className="h-32 bg-gray-100 dark:bg-gray-800 rounded" />
      </div>
    </div>
  )
}

// ============================================================================
// BenchmarksTab
// ============================================================================

export function BenchmarksTab({ timeRange }: BenchmarksTabProps) {
  const range = mapTimeRange(timeRange)
  const { data, isLoading, error, refetch } = useBenchmarks({ range })

  // Loading state
  if (isLoading) {
    return <BenchmarksSkeleton />
  }

  // Error state
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <AlertTriangle className="w-8 h-8 text-amber-400 mb-3" />
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
          Failed to load benchmark data.
        </p>
        <button
          onClick={() => refetch()}
          className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors cursor-pointer"
        >
          <RefreshCw className="w-4 h-4" />
          Retry
        </button>
      </div>
    )
  }

  if (!data) return null

  return (
    <div className="space-y-6">
      {/* Section 1: Then vs Now - Progress comparison */}
      <ThenVsNow progress={data.progress} />

      {/* Section 2: Category Performance Table */}
      <CategoryPerformanceTable
        categories={data.byCategory}
        userAverage={data.userAverageReeditRate}
      />

      {/* Section 3: Skill Adoption Impact */}
      <SkillAdoptionImpact skills={data.skillAdoption} />

      {/* Section 4: Monthly Report Generator */}
      <MonthlyReportGenerator reportSummary={data.reportSummary} />
    </div>
  )
}
