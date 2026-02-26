import { RefreshCw, AlertTriangle } from 'lucide-react'
import { useFacetStats } from '../../hooks/use-facet-stats'

// ============================================================================
// Loading Skeleton
// ============================================================================

function QualitySkeleton() {
  return (
    <div className="space-y-6 animate-pulse">
      {/* Stats grid skeleton */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
        {[1, 2, 3, 4, 5, 6].map((i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5"
          >
            <div className="h-3 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
            <div className="h-8 w-16 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
            <div className="h-3 w-32 bg-gray-200 dark:bg-gray-700 rounded" />
          </div>
        ))}
      </div>
    </div>
  )
}

// ============================================================================
// Stat Card
// ============================================================================

function StatCard({
  label,
  value,
  detail,
  color,
}: {
  label: string
  value: string
  detail?: string
  color: 'green' | 'amber' | 'red' | 'blue' | 'gray'
}) {
  const colorMap = {
    green: 'text-emerald-600 dark:text-emerald-400',
    amber: 'text-amber-600 dark:text-amber-400',
    red: 'text-red-600 dark:text-red-400',
    blue: 'text-blue-600 dark:text-blue-400',
    gray: 'text-gray-600 dark:text-gray-400',
  }

  const dotColorMap = {
    green: 'bg-emerald-500',
    amber: 'bg-amber-500',
    red: 'bg-red-500',
    blue: 'bg-blue-500',
    gray: 'bg-gray-400',
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
      <div className="flex items-center gap-2 mb-3">
        <span className={`w-2 h-2 rounded-full ${dotColorMap[color]}`} />
        <span className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          {label}
        </span>
      </div>
      <p className={`text-2xl font-bold ${colorMap[color]}`}>{value}</p>
      {detail && (
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
          {detail}
        </p>
      )}
    </div>
  )
}

// ============================================================================
// Coverage Bar
// ============================================================================

function CoverageBar({
  withFacets,
  withoutFacets,
}: {
  withFacets: number
  withoutFacets: number
}) {
  const total = withFacets + withoutFacets
  const pct = total > 0 ? (withFacets / total) * 100 : 0

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
      <div className="flex items-center justify-between mb-3">
        <span className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          Facet Coverage
        </span>
        <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {withFacets} / {total} sessions
        </span>
      </div>
      <div className="w-full h-2.5 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
        <div
          className="h-full bg-blue-500 dark:bg-blue-400 rounded-full transition-all duration-500"
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
        {pct.toFixed(0)}% of sessions have quality data analyzed
      </p>
    </div>
  )
}

// ============================================================================
// QualityTab
// ============================================================================

export function QualityTab() {
  const { data, isLoading, error, refetch } = useFacetStats()

  // Loading state
  if (isLoading) {
    return <QualitySkeleton />
  }

  // Error state
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <AlertTriangle className="w-8 h-8 text-amber-400 mb-3" />
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
          Failed to load quality data.
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

  const total = data.totalWithFacets + data.totalWithoutFacets
  const frictionRate =
    data.totalWithFacets > 0
      ? (data.frictionSessionCount / data.totalWithFacets) * 100
      : 0

  // Choose color for achievement rate
  const achievementColor: 'green' | 'amber' | 'red' =
    data.achievementRate >= 70
      ? 'green'
      : data.achievementRate >= 40
        ? 'amber'
        : 'red'

  // Choose color for friction rate
  const frictionColor: 'green' | 'amber' | 'red' =
    frictionRate <= 20 ? 'green' : frictionRate <= 40 ? 'amber' : 'red'

  return (
    <div className="space-y-6">
      {/* Coverage */}
      <CoverageBar
        withFacets={data.totalWithFacets}
        withoutFacets={data.totalWithoutFacets}
      />

      {/* Stats Grid */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
        <StatCard
          label="Achievement Rate"
          value={`${data.achievementRate.toFixed(0)}%`}
          detail="Sessions achieving their goals"
          color={achievementColor}
        />

        <StatCard
          label="Satisfied Sessions"
          value={data.satisfiedOrAboveCount.toLocaleString()}
          detail={
            data.totalWithFacets > 0
              ? `${((data.satisfiedOrAboveCount / data.totalWithFacets) * 100).toFixed(0)}% of analyzed sessions`
              : 'No sessions analyzed yet'
          }
          color="green"
        />

        <StatCard
          label="Frustrated Sessions"
          value={data.frustratedCount.toLocaleString()}
          detail={
            data.totalWithFacets > 0
              ? `${((data.frustratedCount / data.totalWithFacets) * 100).toFixed(0)}% of analyzed sessions`
              : 'No sessions analyzed yet'
          }
          color={data.frustratedCount > 0 ? 'red' : 'gray'}
        />

        <StatCard
          label="Friction Rate"
          value={`${frictionRate.toFixed(0)}%`}
          detail={`${data.frictionSessionCount} session${data.frictionSessionCount !== 1 ? 's' : ''} with friction detected`}
          color={frictionColor}
        />

        <StatCard
          label="Sessions Analyzed"
          value={data.totalWithFacets.toLocaleString()}
          detail={`Out of ${total.toLocaleString()} total sessions`}
          color="blue"
        />

        <StatCard
          label="Pending Analysis"
          value={data.totalWithoutFacets.toLocaleString()}
          detail="Sessions not yet analyzed"
          color="gray"
        />
      </div>
    </div>
  )
}
