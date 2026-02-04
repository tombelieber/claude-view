import { GitBranch } from 'lucide-react'
import { TimeRangeFilter } from './TimeRangeFilter'
import type { TimeRange } from '../../hooks/use-contributions'

interface ContributionsHeaderProps {
  range: TimeRange
  onRangeChange: (range: TimeRange) => void
  sessionCount: number
}

/**
 * Header for the contributions page with title and time range filter.
 */
export function ContributionsHeader({ range, onRangeChange, sessionCount }: ContributionsHeaderProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-[#7c9885]/10 rounded-lg">
            <GitBranch className="w-5 h-5 text-[#7c9885]" />
          </div>
          <div>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
              AI Contributions
            </h1>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Tracking your AI-assisted development across{' '}
              <span className="font-medium tabular-nums">{sessionCount}</span>{' '}
              session{sessionCount !== 1 ? 's' : ''}
            </p>
          </div>
        </div>

        <TimeRangeFilter value={range} onChange={onRangeChange} />
      </div>
    </div>
  )
}
