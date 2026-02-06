import { useState } from 'react'
import { useSearchParams, Link } from 'react-router-dom'
import { Lightbulb, AlertTriangle, RefreshCw, BarChart3 } from 'lucide-react'
import { useInsights, type TimeRange, type TabId } from '../hooks/use-insights'
import { HeroInsight } from './insights/HeroInsight'
import { QuickStatsRow } from './insights/QuickStatsRow'
import { PatternsTabs } from './insights/PatternsTabs'
import { PatternsTab } from './insights/PatternsTab'
import { CategoriesTab } from './insights/CategoriesTab'
import { TrendsTab } from './insights/TrendsTab'
import { BenchmarksTab } from './insights/BenchmarksTab'
import { TimeRangeFilter } from './insights/TimeRangeFilter'
import { InsightsSkeleton } from './insights/InsightsSkeleton'

const VALID_RANGES: TimeRange[] = ['7d', '30d', '90d', 'all']

function isValidTimeRange(value: string | null): value is TimeRange {
  return value !== null && VALID_RANGES.includes(value as TimeRange)
}

export function InsightsPage() {
  const [searchParams, setSearchParams] = useSearchParams()

  // Initialize time range from URL or default to 30d
  const rangeFromUrl = searchParams.get('range')
  const initialRange: TimeRange = isValidTimeRange(rangeFromUrl) ? rangeFromUrl : '30d'

  const [timeRange, setTimeRange] = useState<TimeRange>(initialRange)
  const [activeTab, setActiveTab] = useState<TabId>('patterns')

  const { data, isLoading, error, refetch } = useInsights({ timeRange })

  const handleTimeRangeChange = (range: TimeRange) => {
    setTimeRange(range)
    const params = new URLSearchParams(searchParams)
    params.set('range', range)
    setSearchParams(params)
  }

  // Error state
  if (error) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="max-w-5xl mx-auto px-6 py-6">
          <PageHeader timeRange={timeRange} onTimeRangeChange={handleTimeRangeChange} />
          <div className="flex flex-col items-center justify-center py-20">
            <AlertTriangle className="w-10 h-10 text-amber-400 mb-4" />
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
              Unable to Load Insights
            </h2>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4 text-center max-w-md">
              Something went wrong while analyzing your sessions.
            </p>
            <button
              onClick={() => refetch()}
              className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors cursor-pointer"
            >
              <RefreshCw className="w-4 h-4" />
              Try Again
            </button>
          </div>
        </div>
      </div>
    )
  }

  // Loading state (full skeleton)
  if (isLoading) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="max-w-5xl mx-auto px-6 py-6">
          <PageHeader timeRange={timeRange} onTimeRangeChange={handleTimeRangeChange} />
          <InsightsSkeleton />
        </div>
      </div>
    )
  }

  // Not enough data state
  if (data && !data.meta.hasEnoughData) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="max-w-5xl mx-auto px-6 py-6">
          <PageHeader timeRange={timeRange} onTimeRangeChange={handleTimeRangeChange} />
          <div className="flex flex-col items-center justify-center py-20">
            <BarChart3 className="w-10 h-10 text-gray-300 dark:text-gray-600 mb-4" />
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
              Not Enough Data
            </h2>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-1 text-center max-w-md">
              We need at least {data.meta.minSessionsRequired} sessions to detect patterns.
            </p>
            <p className="text-sm text-gray-400 dark:text-gray-500 mb-4">
              You have {data.meta.totalSessions} session{data.meta.totalSessions !== 1 ? 's' : ''} indexed.
            </p>
            <Link
              to="/history"
              className="inline-flex items-center gap-1 text-sm font-medium text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 transition-colors"
            >
              View Sessions
            </Link>
          </div>
        </div>
      </div>
    )
  }

  // Populated state
  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-5xl mx-auto px-6 py-6">
        <PageHeader timeRange={timeRange} onTimeRangeChange={handleTimeRangeChange} />

        <div className="space-y-6">
          {/* Hero Insight */}
          <HeroInsight insight={data?.heroInsight ?? null} isLoading={false} />

          {/* Quick Stats Row */}
          <QuickStatsRow
            workBreakdown={data?.quickStats.workBreakdown ?? null}
            efficiency={data?.quickStats.efficiency ?? null}
            patterns={data?.quickStats.patterns ?? null}
            isLoading={false}
          />

          {/* Tab Navigation */}
          <PatternsTabs
            activeTab={activeTab}
            onTabChange={setActiveTab}
          />

          {/* Tab Content */}
          {activeTab === 'patterns' && data && (
            <PatternsTab groups={data.patternGroups} />
          )}

          {activeTab === 'categories' && (
            <CategoriesTab timeRange={timeRange} />
          )}

          {activeTab === 'trends' && (
            <TrendsTab timeRange={timeRange} />
          )}

          {activeTab === 'benchmarks' && (
            <BenchmarksTab timeRange={timeRange} />
          )}
        </div>
      </div>
    </div>
  )
}

// ============================================================================
// Page Header
// ============================================================================

function PageHeader({
  timeRange,
  onTimeRangeChange,
}: {
  timeRange: TimeRange
  onTimeRangeChange: (range: TimeRange) => void
}) {
  return (
    <div className="flex items-center justify-between mb-6">
      <div className="flex items-center gap-2">
        <Lightbulb className="w-5 h-5 text-amber-500" />
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
          Insights
        </h1>
      </div>
      <TimeRangeFilter value={timeRange} onChange={onTimeRangeChange} />
    </div>
  )
}
