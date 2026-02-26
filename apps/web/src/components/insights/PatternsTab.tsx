import { PatternGroup } from './PatternGroup'
import { CoachingBudget } from './CoachingBudget'
import { BulkRuleActions } from './BulkRuleActions'
import type { GeneratedInsight } from '../../types/generated/GeneratedInsight'

interface PatternsTabProps {
  groups: {
    high: GeneratedInsight[]
    medium: GeneratedInsight[]
    observations: GeneratedInsight[]
  }
}

export function PatternsTab({ groups }: PatternsTabProps) {
  const totalPatterns =
    groups.high.length + groups.medium.length + groups.observations.length

  const allPatterns = [...groups.high, ...groups.medium, ...groups.observations]
  const patternsWithRecommendations = allPatterns.filter((p) => p.recommendation)

  if (totalPatterns === 0) {
    return (
      <div className="py-12 text-center">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No patterns detected in the selected time range.
        </p>
        <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
          Try expanding the time range or adding more sessions.
        </p>
      </div>
    )
  }

  return (
    <div>
      <div className="flex items-center justify-between pt-4 mb-4">
        <div className="flex items-center gap-3">
          <span className="text-sm text-gray-500 dark:text-gray-400">
            {totalPatterns} patterns
          </span>
          <BulkRuleActions patterns={patternsWithRecommendations} />
        </div>
        <CoachingBudget />
      </div>
      <div className="space-y-8">
        <PatternGroup tier="high" patterns={groups.high} />
        <PatternGroup tier="medium" patterns={groups.medium} />
        <PatternGroup tier="observations" patterns={groups.observations} />
      </div>
    </div>
  )
}
