import { useMemo } from 'react'
import {
  ArrowLeft,
  TrendingUp,
  TrendingDown,
  Clock,
  MessageSquare,
  GitCommit,
  RefreshCw,
} from 'lucide-react'
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from 'recharts'
import type { CategoryNode } from '../../types/generated/CategoryNode'
import type { OverallAverages } from '../../types/generated/OverallAverages'

// ============================================================================
// Formatting helpers
// ============================================================================

function formatDuration(seconds: number): string {
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes} min`
  const hours = Math.floor(minutes / 60)
  const remaining = minutes % 60
  return remaining > 0 ? `${hours}h ${remaining}m` : `${hours}h`
}

function formatPercentage(value: number): string {
  // If value is a ratio (0-1), multiply by 100
  const pct = value > 1 ? value : value * 100
  return `${pct.toFixed(0)}%`
}

// ============================================================================
// Main component
// ============================================================================

interface DrillDownProps {
  category: CategoryNode
  parentCategory?: CategoryNode
  overallAverages: OverallAverages
  onBack: () => void
  onDrillDown: (categoryId: string) => void
}

export function CategoryDrillDown({
  category,
  overallAverages,
  onBack,
  onDrillDown,
}: DrillDownProps) {
  // Build breadcrumb path
  const breadcrumbs = useMemo(() => {
    const parts = category.id.split('/')
    return parts.map((_, index) => ({
      id: parts.slice(0, index + 1).join('/'),
      name: parts[index]
        .split(/[_-]/)
        .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
        .join(' '),
    }))
  }, [category.id])

  // Subcategories bar chart data
  const subcategoryData = useMemo(() => {
    if (!category.children?.length) return []
    return [...category.children]
      .map((child) => ({
        name: child.name,
        id: child.id,
        count: child.count,
        percentage: child.percentage,
      }))
      .sort((a, b) => b.count - a.count)
  }, [category.children])

  // Compare metric to overall average
  const compareToAverage = (
    value: number,
    average: number,
    lowerIsBetter = false,
  ) => {
    const diff = value - average
    const percentDiff = average > 0 ? (diff / average) * 100 : 0
    const isGood = lowerIsBetter ? diff < 0 : diff > 0
    return { diff, percentDiff, isGood }
  }

  return (
    <div className="space-y-6">
      {/* Header with Breadcrumb */}
      <div className="flex items-center gap-4">
        <button
          onClick={onBack}
          className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors cursor-pointer"
          aria-label="Go back"
        >
          <ArrowLeft className="w-5 h-5 text-gray-600 dark:text-gray-400" />
        </button>
        <div>
          <nav className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
            <button
              onClick={() => onDrillDown('')}
              className="hover:text-gray-900 dark:hover:text-white cursor-pointer"
            >
              All
            </button>
            {breadcrumbs.map((crumb, index) => (
              <span key={crumb.id} className="flex items-center gap-2">
                <span>/</span>
                {index === breadcrumbs.length - 1 ? (
                  <span className="text-gray-900 dark:text-white font-medium">
                    {crumb.name}
                  </span>
                ) : (
                  <button
                    onClick={() => onDrillDown(crumb.id)}
                    className="hover:text-gray-900 dark:hover:text-white cursor-pointer"
                  >
                    {crumb.name}
                  </button>
                )}
              </span>
            ))}
          </nav>
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mt-1">
            {category.name}
            <span className="text-gray-500 dark:text-gray-400 ml-2 font-normal text-base">
              ({category.percentage.toFixed(0)}% -- {category.count} session
              {category.count !== 1 ? 's' : ''})
            </span>
          </h2>
        </div>
      </div>

      {/* Subcategories Chart */}
      {subcategoryData.length > 0 && (
        <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4">
            Subcategories
          </h3>
          <div className="h-[200px]">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={subcategoryData} layout="vertical">
                <XAxis type="number" hide />
                <YAxis
                  type="category"
                  dataKey="name"
                  width={120}
                  tick={{ fontSize: 12 }}
                />
                <Tooltip
                  content={({ payload }) => {
                    if (!payload?.[0]) return null
                    const d = payload[0].payload as (typeof subcategoryData)[0]
                    return (
                      <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
                        <div className="font-semibold">{d.name}</div>
                        <div>
                          {d.count} sessions ({d.percentage.toFixed(1)}%)
                        </div>
                      </div>
                    )
                  }}
                />
                <Bar
                  dataKey="count"
                  radius={[0, 4, 4, 0]}
                  onClick={(_, index) => {
                    const entry = subcategoryData[index]
                    if (entry) onDrillDown(entry.id)
                  }}
                  style={{ cursor: 'pointer' }}
                >
                  {subcategoryData.map((_, index) => (
                    <Cell key={index} fill="#3B82F6" />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}

      {/* Performance Metrics */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4">
          {category.name} Performance
        </h3>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <MetricCard
            icon={RefreshCw}
            label="Avg Re-edit Rate"
            value={formatPercentage(category.avgReeditRate)}
            comparison={compareToAverage(
              category.avgReeditRate,
              overallAverages.avgReeditRate,
              true,
            )}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={Clock}
            label="Avg Session Length"
            value={formatDuration(category.avgDuration)}
            comparison={compareToAverage(
              category.avgDuration,
              overallAverages.avgDuration,
            )}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={MessageSquare}
            label="Avg Prompts"
            value={category.avgPrompts.toFixed(1)}
            comparison={compareToAverage(
              category.avgPrompts,
              overallAverages.avgPrompts,
            )}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={GitCommit}
            label="Commit Rate"
            value={formatPercentage(category.commitRate > 1 ? category.commitRate : category.commitRate * 100)}
            comparison={compareToAverage(
              category.commitRate,
              overallAverages.commitRate,
            )}
            compareLabel="vs overall"
          />
        </div>

        {/* AI Insight */}
        {category.insight && (
          <div className="mt-4 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
            <p className="text-sm text-amber-800 dark:text-amber-200">
              <span className="font-medium">Insight:</span> {category.insight}
            </p>
          </div>
        )}
      </div>
    </div>
  )
}

// ============================================================================
// Metric Card Sub-component
// ============================================================================

interface MetricCardProps {
  icon: React.ElementType
  label: string
  value: string
  comparison: { diff: number; percentDiff: number; isGood: boolean }
  compareLabel: string
}

function MetricCard({
  icon: Icon,
  label,
  value,
  comparison,
  compareLabel,
}: MetricCardProps) {
  const TrendIcon = comparison.isGood ? TrendingUp : TrendingDown
  const trendColor = comparison.isGood
    ? 'text-green-600 dark:text-green-400'
    : 'text-red-600 dark:text-red-400'

  return (
    <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800">
      <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 mb-1">
        <Icon className="w-4 h-4" />
        <span className="text-xs">{label}</span>
      </div>
      <div className="text-lg font-semibold text-gray-900 dark:text-gray-100">
        {value}
      </div>
      {Math.abs(comparison.percentDiff) > 0.5 && (
        <div className={`flex items-center gap-1 text-xs ${trendColor}`}>
          <TrendIcon className="w-3 h-3" />
          <span>
            {comparison.percentDiff > 0 ? '+' : ''}
            {comparison.percentDiff.toFixed(0)}% {compareLabel}
          </span>
        </div>
      )}
    </div>
  )
}
