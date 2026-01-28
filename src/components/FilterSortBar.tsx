import { useState, useRef, useEffect } from 'react'
import { Filter, SortDesc, ChevronDown, Check } from 'lucide-react'
import { cn } from '../lib/utils'

export type SessionFilter = 'all' | 'has_commits' | 'high_reedit' | 'long_session'
export type SessionSort = 'recent' | 'tokens' | 'prompts' | 'files_edited' | 'duration'

interface FilterOption {
  value: SessionFilter
  label: string
  description?: string
}

interface SortOption {
  value: SessionSort
  label: string
}

const FILTER_OPTIONS: FilterOption[] = [
  { value: 'all', label: 'All sessions' },
  { value: 'has_commits', label: 'Has commits', description: '1+ linked commits' },
  { value: 'high_reedit', label: 'High re-edit', description: '>20% re-edit rate' },
  { value: 'long_session', label: 'Long sessions', description: '>30 minutes' },
]

const SORT_OPTIONS: SortOption[] = [
  { value: 'recent', label: 'Most recent' },
  { value: 'tokens', label: 'Most tokens' },
  { value: 'prompts', label: 'Most prompts' },
  { value: 'files_edited', label: 'Most files edited' },
  { value: 'duration', label: 'Longest duration' },
]

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

  const selectedOption = options.find(o => o.value === value)

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border transition-all',
          isActive
            ? 'bg-blue-50 border-blue-200 text-blue-700'
            : 'bg-white border-gray-200 text-gray-600 hover:border-gray-300 hover:bg-gray-50'
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
          className="absolute top-full left-0 mt-1.5 min-w-[180px] bg-white border border-gray-200 rounded-lg shadow-lg z-50 py-1"
          role="listbox"
          aria-label={label}
        >
          {options.map(option => {
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
                  'w-full flex items-start gap-2.5 px-3 py-2 text-left hover:bg-gray-50 transition-colors',
                  isSelected && 'bg-blue-50'
                )}
              >
                <div className={cn(
                  'w-4 h-4 flex-shrink-0 flex items-center justify-center mt-0.5',
                  isSelected ? 'text-blue-600' : 'text-transparent'
                )}>
                  <Check className="w-3.5 h-3.5" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className={cn(
                    'text-sm',
                    isSelected ? 'text-blue-700 font-medium' : 'text-gray-700'
                  )}>
                    {option.label}
                  </div>
                  {option.description && (
                    <div className="text-xs text-gray-400 mt-0.5">
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

interface FilterSortBarProps {
  filter: SessionFilter
  sort: SessionSort
  onFilterChange: (filter: SessionFilter) => void
  onSortChange: (sort: SessionSort) => void
  className?: string
}

/**
 * FilterSortBar provides filter and sort dropdowns for session lists.
 *
 * Filter options:
 * - All sessions
 * - Has commits (1+ linked commits)
 * - High re-edit (>20% re-edit rate)
 * - Long sessions (>30 minutes)
 *
 * Sort options:
 * - Most recent
 * - Most tokens
 * - Most prompts
 * - Most files edited
 * - Longest duration
 */
export function FilterSortBar({
  filter,
  sort,
  onFilterChange,
  onSortChange,
  className,
}: FilterSortBarProps) {
  const isFilterActive = filter !== 'all'
  const isSortActive = sort !== 'recent'

  return (
    <div className={cn('flex items-center gap-2', className)}>
      <Dropdown
        label="Filter"
        icon={<Filter className="w-3.5 h-3.5" />}
        value={filter}
        options={FILTER_OPTIONS}
        onChange={(v) => onFilterChange(v as SessionFilter)}
        isActive={isFilterActive}
      />
      <Dropdown
        label="Sort"
        icon={<SortDesc className="w-3.5 h-3.5" />}
        value={sort}
        options={SORT_OPTIONS}
        onChange={(v) => onSortChange(v as SessionSort)}
        isActive={isSortActive}
      />
    </div>
  )
}

// Re-export the hook from its separate file for convenience
export { useFilterSort } from '../hooks/use-filter-sort'
