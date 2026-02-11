import { PatternGroup } from './PatternGroup'
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
    <div className="space-y-8 pt-4">
      <PatternGroup tier="high" patterns={groups.high} />
      <PatternGroup tier="medium" patterns={groups.medium} />
      <PatternGroup tier="observations" patterns={groups.observations} />
    </div>
  )
}
