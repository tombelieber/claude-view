import { Check, ChevronDown, Filter, RotateCcw, SortDesc } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import {
  type PromptFilters,
  type PromptGroupBy,
  type PromptSort,
  countActivePromptFilters,
  defaultPromptFilters,
} from '../../hooks/use-prompt-filters'
import { cn } from '../../lib/utils'
import { PromptFilterPopover } from './PromptFilterPopover'

interface PromptToolbarProps {
  filters: PromptFilters
  onFiltersChange: (filters: PromptFilters) => void
  totalCount: number
}

interface DropdownProps {
  label: string
  icon: React.ReactNode
  value: string
  options: Array<{ value: string; label: string; description?: string }>
  onChange: (value: string) => void
  isActive?: boolean
}

function Dropdown({ label, icon, value, options, onChange, isActive }: DropdownProps) {
  const [isOpen, setIsOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  // Close on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setIsOpen(false)
      }
    }
    if (isOpen) {
      document.addEventListener('mousedown', handleClick)
      return () => document.removeEventListener('mousedown', handleClick)
    }
  }, [isOpen])

  const selectedOption = options.find((o) => o.value === value)

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border',
          'transition-all duration-150 ease-out cursor-pointer',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
          isActive
            ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300'
            : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-750',
        )}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={`${label}: ${selectedOption?.label}`}
      >
        {icon}
        <span className="max-w-[120px] truncate">{selectedOption?.label || label}</span>
        <ChevronDown className={cn('w-3.5 h-3.5 transition-transform', isOpen && 'rotate-180')} />
      </button>

      {isOpen && (
        <div
          className="absolute top-full left-0 mt-1.5 min-w-[180px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 py-1"
          role="listbox"
          aria-label={label}
        >
          {options.map((option) => {
            const isSelected = option.value === value
            return (
              <button
                key={option.value}
                type="button"
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onChange(option.value)
                  setIsOpen(false)
                }}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-2 text-left transition-colors',
                  isSelected
                    ? 'bg-blue-50 dark:bg-blue-950/30 text-blue-700 dark:text-blue-300'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-750',
                )}
              >
                <div
                  className={cn(
                    'flex items-center justify-center w-4 h-4 rounded border',
                    isSelected
                      ? 'bg-blue-600 border-blue-600'
                      : 'border-gray-300 dark:border-gray-600',
                  )}
                >
                  {isSelected && <Check className="w-3.5 h-3.5 text-white" />}
                </div>
                <div className="flex-1 min-w-0">
                  <div
                    className={cn(
                      'text-xs font-medium',
                      isSelected
                        ? 'text-blue-700 dark:text-blue-300'
                        : 'text-gray-700 dark:text-gray-300',
                    )}
                  >
                    {option.label}
                  </div>
                  {option.description && (
                    <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                      {option.description}
                    </div>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

const SORT_OPTIONS: Array<{ value: PromptSort; label: string }> = [
  { value: 'recent', label: 'Recent' },
  { value: 'oldest', label: 'Oldest' },
  { value: 'most_repeated', label: 'Most Repeated' },
  { value: 'longest', label: 'Longest' },
]

const GROUP_BY_OPTIONS: Array<{
  value: PromptGroupBy
  label: string
  description?: string
}> = [
  { value: 'none', label: 'None', description: 'Flat list' },
  { value: 'day', label: 'Day', description: 'Group by calendar day' },
  { value: 'week', label: 'Week', description: 'Group by week' },
  { value: 'project', label: 'Project', description: 'Group by project' },
  { value: 'intent', label: 'Intent', description: 'Group by intent type' },
  { value: 'model', label: 'Model', description: 'Group by model used' },
]

/**
 * Toolbar for the Prompt History page.
 *
 * Features:
 * - Group-by dropdown (none/day/week/project/intent/model)
 * - Filter popover (intent, model, branch, paste, complexity, template match)
 * - Sort dropdown (recent/oldest/most_repeated/longest)
 * - Total count display
 * - Reset button when any non-default value is active
 */
export function PromptToolbar({ filters, onFiltersChange, totalCount }: PromptToolbarProps) {
  const activeFilterCount = countActivePromptFilters(filters)
  const hasNonDefaults =
    activeFilterCount > 0 || filters.sort !== 'recent' || filters.groupBy !== 'none'

  const handleSortChange = (sort: string) => {
    onFiltersChange({ ...filters, sort: sort as PromptSort })
  }

  const handleGroupByChange = (groupBy: string) => {
    onFiltersChange({ ...filters, groupBy: groupBy as PromptGroupBy })
  }

  const handleClearFilters = () => {
    onFiltersChange({
      ...defaultPromptFilters,
      availableBranches: filters.availableBranches,
      availableModels: filters.availableModels,
    })
  }

  return (
    <div className="flex items-center justify-between gap-2">
      {/* Left side: Group by, Filter, Sort */}
      <div className="flex items-center gap-2">
        {/* Group by dropdown */}
        <Dropdown
          label="Group by"
          icon={<div className="w-3.5 h-3.5 flex items-center justify-center text-xs">&#8862;</div>}
          value={filters.groupBy}
          options={GROUP_BY_OPTIONS}
          onChange={handleGroupByChange}
          isActive={filters.groupBy !== 'none'}
        />

        {/* Filter popover */}
        <PromptFilterPopover filters={filters} onFiltersChange={onFiltersChange}>
          <button
            type="button"
            className={cn(
              'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border cursor-pointer',
              'transition-all duration-150 ease-out',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
              activeFilterCount > 0
                ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300'
                : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-750',
            )}
            aria-label={`Filters ${activeFilterCount > 0 ? `(${activeFilterCount} active)` : ''}`}
          >
            <Filter className="w-3.5 h-3.5" />
            <span>Filters</span>
            {activeFilterCount > 0 && (
              <span className="inline-flex items-center justify-center min-w-[16px] h-4 px-1 text-xs font-semibold rounded-full bg-blue-600 dark:bg-blue-500 text-white">
                {activeFilterCount}
              </span>
            )}
          </button>
        </PromptFilterPopover>

        {/* Sort dropdown */}
        <Dropdown
          label="Sort"
          icon={<SortDesc className="w-3.5 h-3.5" />}
          value={filters.sort}
          options={SORT_OPTIONS}
          onChange={handleSortChange}
          isActive={filters.sort !== 'recent'}
        />

        {/* Reset all filters/sort/grouping */}
        {hasNonDefaults && (
          <button
            type="button"
            onClick={handleClearFilters}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1.5 text-xs font-medium rounded-md',
              'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            )}
            aria-label="Reset all filters"
          >
            <RotateCcw className="w-3 h-3" />
            <span>Reset</span>
          </button>
        )}
      </div>

      {/* Right side: Total count */}
      <div className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
        {totalCount.toLocaleString()} prompt{totalCount !== 1 ? 's' : ''}
      </div>
    </div>
  )
}
