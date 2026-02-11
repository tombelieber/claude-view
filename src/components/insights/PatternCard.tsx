import { cn } from '../../lib/utils'
import type { GeneratedInsight } from '../../types/generated/GeneratedInsight'

interface PatternCardProps {
  pattern: GeneratedInsight
}

/**
 * Visual impact bar showing relative strength of the impact score.
 */
function ImpactBar({ score }: { score: number }) {
  const percentage = Math.round(score * 100)
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          className={cn(
            'h-full rounded-full transition-all duration-300',
            score > 0.7
              ? 'bg-blue-500'
              : score > 0.4
                ? 'bg-gray-400 dark:bg-gray-500'
                : 'bg-gray-300 dark:bg-gray-600'
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums w-8 text-right">
        {percentage}%
      </span>
    </div>
  )
}

/**
 * Comparison bar showing two values side by side.
 */
function ComparisonBar({ value, comparison }: { value: number; comparison: number }) {
  if (value === 0 && comparison === 0) return null

  const max = Math.max(value, comparison, 0.01)
  const valuePct = (value / max) * 100
  const comparisonPct = (comparison / max) * 100

  return (
    <div className="mt-3 space-y-1">
      <div className="flex items-center gap-2">
        <div className="flex-1 h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
          <div
            className="h-full bg-blue-500 rounded-full"
            style={{ width: `${valuePct}%` }}
          />
        </div>
        <span className="text-[10px] text-gray-500 dark:text-gray-400 tabular-nums w-10 text-right">
          {value.toFixed(2)}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <div className="flex-1 h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
          <div
            className="h-full bg-gray-400 dark:bg-gray-600 rounded-full"
            style={{ width: `${comparisonPct}%` }}
          />
        </div>
        <span className="text-[10px] text-gray-500 dark:text-gray-400 tabular-nums w-10 text-right">
          {comparison.toFixed(2)}
        </span>
      </div>
    </div>
  )
}

/**
 * Confidence badge based on sample size.
 */
function ConfidenceBadge({ sampleSize }: { sampleSize: number }) {
  const level = sampleSize >= 50 ? 'high' : sampleSize >= 20 ? 'medium' : 'low'
  const color =
    level === 'high'
      ? 'text-green-600 dark:text-green-400'
      : level === 'medium'
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-gray-400 dark:text-gray-500'

  return (
    <span className={cn('text-[10px] font-medium capitalize', color)}>
      {level} confidence
    </span>
  )
}

export function PatternCard({ pattern }: PatternCardProps) {
  const compValue = pattern.evidence.comparisonValues['value'] ?? 0
  const compComparison = pattern.evidence.comparisonValues['comparison'] ?? 0
  const hasComparison = compValue > 0 || compComparison > 0

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4 transition-colors duration-150 hover:border-gray-300 dark:hover:border-gray-600">
      {/* Header row: category + impact */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-[11px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
          {pattern.category}
        </span>
        <div className="w-24">
          <ImpactBar score={pattern.impactScore} />
        </div>
      </div>

      {/* Title */}
      <h4 className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-1">
        {pattern.title}
      </h4>

      {/* Body */}
      <p className="text-xs text-gray-600 dark:text-gray-400 mb-2">{pattern.body}</p>

      {/* Recommendation */}
      {pattern.recommendation && (
        <p className="text-xs text-blue-600 dark:text-blue-400 mb-2 italic">
          {pattern.recommendation}
        </p>
      )}

      {/* Comparison bar */}
      {hasComparison && <ComparisonBar value={compValue} comparison={compComparison} />}

      {/* Footer: sample size + confidence */}
      <div className="flex items-center justify-between mt-3 pt-2 border-t border-gray-100 dark:border-gray-800">
        <span className="text-[10px] text-gray-400 dark:text-gray-500">
          Based on {pattern.evidence.sampleSize} sessions
        </span>
        <ConfidenceBadge sampleSize={pattern.evidence.sampleSize} />
      </div>
    </div>
  )
}
