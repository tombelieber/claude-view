import { cn } from '../../lib/utils'
import { Check, ArrowRight, AlertTriangle } from 'lucide-react'
import type { CategoryPerformance } from '../../types/generated/CategoryPerformance'
import type { CategoryVerdict } from '../../types/generated/CategoryVerdict'

interface CategoryPerformanceTableProps {
  categories: CategoryPerformance[]
  userAverage: number
  className?: string
}

const VERDICT_CONFIG: Record<
  CategoryVerdict,
  { icon: typeof Check; className: string; label: string }
> = {
  excellent: { icon: Check, className: 'text-green-600 dark:text-green-400', label: 'Excellent' },
  good: { icon: Check, className: 'text-green-500 dark:text-green-400', label: 'Strong' },
  average: { icon: ArrowRight, className: 'text-gray-500 dark:text-gray-400', label: 'Average' },
  needs_work: {
    icon: AlertTriangle,
    className: 'text-amber-500 dark:text-amber-400',
    label: 'Needs work',
  },
}

export function CategoryPerformanceTable({
  categories,
  userAverage,
  className,
}: CategoryPerformanceTableProps) {
  // Sort by re-edit rate (best first)
  const sorted = [...categories].sort((a, b) => a.reeditRate - b.reeditRate)

  // Calculate bar scale
  const maxDelta = Math.max(...categories.map((c) => Math.abs(c.vsAverage)), 0.01)
  const scale = 50 / maxDelta

  const getBarStyle = (vsAverage: number) => {
    const width = Math.min(Math.abs(vsAverage) * scale, 50)
    const isBetter = vsAverage < 0
    return {
      width: `${width}%`,
      marginLeft: isBetter ? `${50 - width}%` : '50%',
      backgroundColor: isBetter ? '#22c55e' : '#f59e0b',
    }
  }

  if (categories.length === 0) {
    return (
      <div
        className={cn(
          'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
          className
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
          By Category Performance
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 text-center py-6">
          No categorized sessions yet. Run classification to see category performance.
        </p>
      </div>
    )
  }

  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
        className
      )}
    >
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        By Category Performance
      </h3>

      <div className="overflow-x-auto">
        <table className="w-full" role="table">
          <thead>
            <tr className="text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
              <th className="pb-3">Category</th>
              <th className="pb-3 text-right">Re-edit</th>
              <th className="pb-3 px-4">vs Your Avg</th>
              <th className="pb-3">Verdict</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {sorted.map((cat) => {
              const config = VERDICT_CONFIG[cat.verdict]
              const Icon = config.icon

              return (
                <tr key={cat.category} className="group" title={cat.insight}>
                  <td className="py-2.5 text-sm font-medium text-gray-700 dark:text-gray-300 capitalize">
                    {cat.category.replace(/_/g, ' ')}
                  </td>
                  <td className="py-2.5 text-sm font-mono text-right text-gray-600 dark:text-gray-400">
                    {cat.reeditRate.toFixed(2)}
                  </td>
                  <td className="py-2.5 px-4">
                    <div className="relative h-4 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                      {/* Center line */}
                      <div className="absolute left-1/2 top-0 bottom-0 w-px bg-gray-300 dark:bg-gray-600 z-10" />
                      {/* Bar */}
                      <div
                        className="absolute top-0.5 bottom-0.5 rounded"
                        style={getBarStyle(cat.vsAverage)}
                      />
                    </div>
                  </td>
                  <td className="py-2.5">
                    <div className={cn('flex items-center gap-1 text-sm', config.className)}>
                      <Icon className="w-4 h-4" />
                      <span>{config.label}</span>
                    </div>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      {/* Legend */}
      <div className="mt-4 flex justify-center gap-4 text-xs text-gray-500 dark:text-gray-400">
        <span>Better</span>
        <span className="font-mono">Your avg ({userAverage.toFixed(2)})</span>
        <span>Worse</span>
      </div>

      {/* Insight for worst categories */}
      {sorted.filter((c) => c.verdict === 'needs_work').length > 0 && (
        <div className="mt-4 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
          <p className="text-sm text-amber-800 dark:text-amber-200">
            {sorted.find((c) => c.verdict === 'needs_work')?.insight}
          </p>
        </div>
      )}
    </div>
  )
}
