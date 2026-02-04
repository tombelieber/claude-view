import { GitBranch, Calendar, ArrowRight } from 'lucide-react'
import { Link } from 'react-router-dom'
import type { TimeRange } from '../../hooks/use-contributions'

interface ContributionsEmptyStateProps {
  range: TimeRange
  onRangeChange: (range: TimeRange) => void
}

const RANGE_LABELS: Record<TimeRange, string> = {
  today: 'today',
  week: 'this week',
  month: 'this month',
  '90days': 'the last 90 days',
  all: 'all time',
}

/**
 * Empty state for the contributions page when no sessions exist.
 */
export function ContributionsEmptyState({ range, onRangeChange }: ContributionsEmptyStateProps) {
  const isFiltered = range !== 'all'

  return (
    <div className="flex flex-col items-center justify-center py-16 px-4 text-center">
      <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 dark:bg-gray-800 mb-6">
        {isFiltered ? (
          <Calendar className="w-7 h-7 text-gray-400" />
        ) : (
          <GitBranch className="w-7 h-7 text-gray-400" />
        )}
      </div>

      {isFiltered ? (
        <>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
            No sessions {RANGE_LABELS[range]}
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 max-w-md mb-6">
            There are no Claude Code sessions recorded for this time period.
            Try expanding your time range to see more data.
          </p>
          <button
            type="button"
            onClick={() => onRangeChange('all')}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-blue-600 hover:text-blue-700 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded-lg transition-colors cursor-pointer"
          >
            View All Time
            <ArrowRight className="w-4 h-4" />
          </button>
        </>
      ) : (
        <>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
            No contribution data yet
          </h2>
          <p className="text-sm text-gray-500 dark:text-gray-400 max-w-md mb-6">
            Start using Claude Code in your terminal to track your AI-assisted development.
            Contribution metrics will appear after your first session.
          </p>
          <Link
            to="/"
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
          >
            Go to Dashboard
            <ArrowRight className="w-4 h-4" />
          </Link>
        </>
      )}
    </div>
  )
}
