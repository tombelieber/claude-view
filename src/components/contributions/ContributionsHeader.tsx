import { GitBranch, X } from 'lucide-react'
import { TimeRangeFilter } from './TimeRangeFilter'
import type { TimeRange } from '../../hooks/use-contributions'

interface ContributionsHeaderProps {
  range: TimeRange
  onRangeChange: (range: TimeRange) => void
  sessionCount: number
  projectFilter?: string | null
  onClearProjectFilter?: () => void
  branchFilter?: string | null
  onClearBranchFilter?: () => void
}

/**
 * Header for the contributions page with title, time range filter,
 * and optional project filter indicator.
 */
export function ContributionsHeader({
  range,
  onRangeChange,
  sessionCount,
  projectFilter,
  onClearProjectFilter,
  branchFilter,
  onClearBranchFilter,
}: ContributionsHeaderProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-[#7c9885]/10 rounded-lg">
            <GitBranch className="w-5 h-5 text-[#7c9885]" />
          </div>
          <div>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
              AI Contributions
              {projectFilter ? (
                <span className="text-base font-normal text-gray-500 dark:text-gray-400">
                  {' '}&mdash; {projectFilter}{branchFilter ? ` / ${branchFilter}` : ''}
                </span>
              ) : (
                <span className="text-base font-normal text-gray-500 dark:text-gray-400">
                  {' '}&mdash; All Projects
                </span>
              )}
            </h1>
            <div className="flex items-center gap-2">
              <p className="text-sm text-gray-500 dark:text-gray-400">
                Tracking your AI-assisted development across{' '}
                <span className="font-medium tabular-nums">{sessionCount}</span>{' '}
                session{sessionCount !== 1 ? 's' : ''}
              </p>
              {projectFilter && onClearProjectFilter && (
                <button
                  onClick={onClearProjectFilter}
                  className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full
                    bg-[#7c9885]/10 text-[#7c9885] hover:bg-[#7c9885]/20
                    transition-colors cursor-pointer"
                  title="Clear project filter"
                >
                  {projectFilter}
                  <X className="w-3 h-3" />
                </button>
              )}
              {branchFilter && onClearBranchFilter && (
                <button
                  onClick={onClearBranchFilter}
                  className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-medium rounded-full
                    bg-orange-500/10 text-orange-600 hover:bg-orange-500/20
                    transition-colors cursor-pointer"
                  title="Clear branch filter"
                >
                  <GitBranch className="w-3 h-3" />
                  {branchFilter}
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          </div>
        </div>

        <TimeRangeFilter value={range} onChange={onRangeChange} />
      </div>
    </div>
  )
}
