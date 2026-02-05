import { cn } from '../../lib/utils'
import { TrendingUp, TrendingDown, Minus } from 'lucide-react'
import type { ProgressComparison } from '../../types/generated/ProgressComparison'

interface ThenVsNowProps {
  progress: ProgressComparison
  className?: string
}

export function ThenVsNow({ progress, className }: ThenVsNowProps) {
  const { firstMonth, lastMonth, improvement, insight } = progress

  const formatPercent = (value: number) => {
    const absValue = Math.abs(value)
    const sign = value > 0 ? '+' : ''
    return `${sign}${absValue.toFixed(0)}%`
  }

  const getImprovementIcon = (value: number, lowerIsBetter: boolean) => {
    const isImprovement = lowerIsBetter ? value < 0 : value > 0
    const isRegression = lowerIsBetter ? value > 0 : value < 0

    if (isImprovement) return <TrendingDown className="w-4 h-4 text-green-600 dark:text-green-400" />
    if (isRegression) return <TrendingUp className="w-4 h-4 text-red-600 dark:text-red-400" />
    return <Minus className="w-4 h-4 text-gray-400" />
  }

  const getChangeColor = (value: number, lowerIsBetter: boolean) => {
    const isImprovement = lowerIsBetter ? value < 0 : value > 0
    const isRegression = lowerIsBetter ? value > 0 : value < 0
    if (isImprovement) return 'text-green-600 dark:text-green-400'
    if (isRegression) return 'text-red-600 dark:text-red-400'
    return 'text-gray-500 dark:text-gray-400'
  }

  const metrics = [
    {
      label: 'Re-edit rate',
      then: firstMonth?.reeditRate,
      now: lastMonth.reeditRate,
      change: improvement?.reeditRate ?? 0,
      lowerIsBetter: true,
      format: (v: number) => v.toFixed(2),
    },
    {
      label: 'Edits/file',
      then: firstMonth?.editsPerFile,
      now: lastMonth.editsPerFile,
      change: improvement?.editsPerFile ?? 0,
      lowerIsBetter: true,
      format: (v: number) => v.toFixed(1),
    },
    {
      label: 'Prompts/task',
      then: firstMonth?.promptsPerTask,
      now: lastMonth.promptsPerTask,
      change: improvement?.promptsPerTask ?? 0,
      lowerIsBetter: true,
      format: (v: number) => v.toFixed(1),
    },
    {
      label: 'Commit rate',
      then: firstMonth?.commitRate,
      now: lastMonth.commitRate,
      change: improvement?.commitRate ?? 0,
      lowerIsBetter: false,
      format: (v: number) => `${(v * 100).toFixed(0)}%`,
    },
  ]

  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6',
        className
      )}
    >
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Your Progress
      </h3>

      {!firstMonth ? (
        <div className="py-6 text-center">
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Not enough historical data for comparison.
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            At least 30 days of history needed for "Then vs Now" comparison.
          </p>
        </div>
      ) : (
        <div className="space-y-0">
          {/* Header row */}
          <div className="grid grid-cols-4 gap-4 pb-3 border-b border-gray-100 dark:border-gray-800">
            <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
              Metric
            </div>
            <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider text-center">
              Then
              <span className="block text-[10px] normal-case tracking-normal font-normal">
                (First Month)
              </span>
            </div>
            <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider text-center">
              Now
              <span className="block text-[10px] normal-case tracking-normal font-normal">
                (Last 30 Days)
              </span>
            </div>
            <div className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider text-center">
              Change
            </div>
          </div>

          {/* Metric rows */}
          {metrics.map((m) => (
            <div
              key={m.label}
              className="grid grid-cols-4 gap-4 py-3 border-b border-gray-50 dark:border-gray-800/50 last:border-b-0"
            >
              <div className="text-sm text-gray-700 dark:text-gray-300">{m.label}</div>
              <div className="text-sm font-mono text-gray-600 dark:text-gray-400 text-center">
                {m.then != null ? m.format(m.then) : '--'}
              </div>
              <div className="text-sm font-mono font-semibold text-gray-900 dark:text-gray-100 text-center">
                {m.format(m.now)}
              </div>
              <div className="flex items-center justify-center gap-1.5">
                {improvement && (
                  <>
                    {getImprovementIcon(m.change, m.lowerIsBetter)}
                    <span
                      className={cn(
                        'text-xs font-mono',
                        getChangeColor(m.change, m.lowerIsBetter)
                      )}
                    >
                      {formatPercent(m.change)}
                    </span>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Insight */}
      {insight && (
        <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
          <p className="text-sm text-blue-800 dark:text-blue-200">{insight}</p>
        </div>
      )}
    </div>
  )
}
