import { useState } from 'react'
import { GitBranch, ChevronDown, ArrowUpDown } from 'lucide-react'
import { cn } from '../../lib/utils'
import { BranchCard } from './BranchCard'
import type { TimeRange } from '../../hooks/use-contributions'
import type { BranchBreakdown } from '../../types/generated'

interface BranchListProps {
  byBranch: BranchBreakdown[]
  onSessionDrillDown?: (sessionId: string) => void
  timeRange?: TimeRange
}

type SortKey = 'lines' | 'sessions' | 'commits' | 'recent'

const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: 'lines', label: 'AI Lines' },
  { value: 'sessions', label: 'Sessions' },
  { value: 'commits', label: 'Commits' },
  { value: 'recent', label: 'Recent' },
]

/**
 * BranchList displays branches with expand/collapse functionality.
 *
 * Features:
 * - Sortable by lines, sessions, commits, or recent activity
 * - Expand/collapse individual branches
 * - Shows AI share progress for each branch
 */
export function BranchList({ byBranch, onSessionDrillDown, timeRange = 'week' }: BranchListProps) {
  const [sortBy, setSortBy] = useState<SortKey>('lines')
  const [expandedBranch, setExpandedBranch] = useState<string | null>(null)
  const [showSortMenu, setShowSortMenu] = useState(false)

  // Sort branches
  const sortedBranches = [...byBranch].sort((a, b) => {
    switch (sortBy) {
      case 'lines':
        return Number(b.linesAdded) - Number(a.linesAdded)
      case 'sessions':
        return Number(b.sessionsCount) - Number(a.sessionsCount)
      case 'commits':
        return Number(b.commitsCount) - Number(a.commitsCount)
      case 'recent':
        return (Number(b.lastActivity) || 0) - (Number(a.lastActivity) || 0)
      default:
        return 0
    }
  })

  const handleToggleBranch = (branchName: string) => {
    setExpandedBranch((prev) => (prev === branchName ? null : branchName))
  }

  if (byBranch.length === 0) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center gap-2 mb-4">
          <GitBranch className="w-4 h-4 text-orange-500" aria-hidden="true" />
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            By Branch
          </h2>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No branch data available for this period.
        </p>
      </div>
    )
  }

  const currentSortLabel = SORT_OPTIONS.find((o) => o.value === sortBy)?.label || 'AI Lines'

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      {/* Header with Sort */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <GitBranch className="w-4 h-4 text-orange-500" aria-hidden="true" />
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            By Branch
          </h2>
        </div>

        {/* Sort Dropdown */}
        <div className="relative">
          <button
            onClick={() => setShowSortMenu(!showSortMenu)}
            className={cn(
              'flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded-lg',
              'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
              'cursor-pointer'
            )}
            aria-haspopup="listbox"
            aria-expanded={showSortMenu}
          >
            <ArrowUpDown className="w-3 h-3" aria-hidden="true" />
            <span>Sort: {currentSortLabel}</span>
            <ChevronDown
              className={cn('w-3 h-3 transition-transform', showSortMenu && 'rotate-180')}
              aria-hidden="true"
            />
          </button>

          {showSortMenu && (
            <>
              {/* Backdrop */}
              <div
                className="fixed inset-0 z-10"
                onClick={() => setShowSortMenu(false)}
                aria-hidden="true"
              />
              {/* Menu */}
              <div
                className="absolute right-0 top-full mt-1 z-20 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg py-1 min-w-32"
                role="listbox"
              >
                {SORT_OPTIONS.map((option) => (
                  <button
                    key={option.value}
                    onClick={() => {
                      setSortBy(option.value)
                      setShowSortMenu(false)
                    }}
                    className={cn(
                      'w-full text-left px-3 py-1.5 text-sm transition-colors cursor-pointer',
                      'focus-visible:outline-none focus-visible:bg-blue-50 dark:focus-visible:bg-blue-900/30',
                      option.value === sortBy
                        ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
                    )}
                    role="option"
                    aria-selected={option.value === sortBy}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </div>

      {/* Branch Cards */}
      <div className="space-y-3">
        {sortedBranches.map((branch) => (
          <BranchCard
            key={branch.branch}
            branch={branch}
            isExpanded={expandedBranch === branch.branch}
            onToggle={() => handleToggleBranch(branch.branch)}
            onDrillDown={onSessionDrillDown}
            timeRange={timeRange}
          />
        ))}
      </div>
    </div>
  )
}
