import { useState, useEffect, useRef } from 'react'
import { Search, X, ChevronDown, Filter } from 'lucide-react'
import type { LiveSessionFilters } from './live-filter'

interface LiveFilterBarProps {
  filters: LiveSessionFilters
  onStatusChange: (statuses: string[]) => void
  onProjectChange: (projects: string[]) => void
  onBranchChange: (branches: string[]) => void
  onSearchChange: (query: string) => void
  onClear: () => void
  activeCount: number
  availableStatuses: string[]
  availableProjects: string[]
  availableBranches: string[]
  searchInputRef?: React.RefObject<HTMLInputElement | null>
}

type DropdownType = 'status' | 'project' | 'branch' | null

export function LiveFilterBar({
  filters,
  onStatusChange,
  onProjectChange,
  onBranchChange,
  onSearchChange,
  onClear,
  activeCount,
  availableStatuses,
  availableProjects,
  availableBranches,
  searchInputRef,
}: LiveFilterBarProps) {
  const [localSearch, setLocalSearch] = useState(filters.search)
  const [openDropdown, setOpenDropdown] = useState<DropdownType>(null)
  const barRef = useRef<HTMLDivElement>(null)

  // Sync local search from external changes (e.g. clearAll)
  useEffect(() => {
    setLocalSearch(filters.search)
  }, [filters.search])

  // Debounce search input to parent
  useEffect(() => {
    const timer = setTimeout(() => {
      if (localSearch !== filters.search) {
        onSearchChange(localSearch)
      }
    }, 200)
    return () => clearTimeout(timer)
  }, [localSearch, filters.search, onSearchChange])

  // Close dropdown on outside click
  useEffect(() => {
    if (!openDropdown) return

    function handleMouseDown(e: MouseEvent) {
      if (barRef.current && !barRef.current.contains(e.target as Node)) {
        setOpenDropdown(null)
      }
    }

    document.addEventListener('mousedown', handleMouseDown)
    return () => document.removeEventListener('mousedown', handleMouseDown)
  }, [openDropdown])

  function toggleDropdown(type: DropdownType) {
    setOpenDropdown((prev) => (prev === type ? null : type))
  }

  function toggleItem(
    current: string[],
    item: string,
    onChange: (items: string[]) => void
  ) {
    if (current.includes(item)) {
      onChange(current.filter((i) => i !== item))
    } else {
      onChange([...current, item])
    }
  }

  function removeFilterPill(type: 'status' | 'project' | 'branch', value: string) {
    switch (type) {
      case 'status':
        onStatusChange(filters.statuses.filter((s) => s !== value))
        break
      case 'project':
        onProjectChange(filters.projects.filter((p) => p !== value))
        break
      case 'branch':
        onBranchChange(filters.branches.filter((b) => b !== value))
        break
    }
  }

  return (
    <div ref={barRef} className="space-y-2">
      {/* Row 1: Search + filter buttons + clear */}
      <div className="flex items-center gap-2">
        {/* Search input */}
        <div className="relative flex-1 max-w-xs">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-gray-400 dark:text-gray-500" />
          <input
            ref={searchInputRef}
            type="text"
            value={localSearch}
            onChange={(e) => setLocalSearch(e.target.value)}
            placeholder="Search sessions..."
            className="w-full bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-md pl-8 pr-8 py-1.5 text-sm text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30"
          />
          {localSearch && (
            <button
              onClick={() => {
                setLocalSearch('')
                onSearchChange('')
              }}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        {/* Filter dropdown buttons */}
        <FilterDropdownButton
          label="Status"
          count={filters.statuses.length}
          isOpen={openDropdown === 'status'}
          onToggle={() => toggleDropdown('status')}
          options={availableStatuses}
          selected={filters.statuses}
          onItemToggle={(item) =>
            toggleItem(filters.statuses, item, onStatusChange)
          }
        />

        <FilterDropdownButton
          label="Project"
          count={filters.projects.length}
          isOpen={openDropdown === 'project'}
          onToggle={() => toggleDropdown('project')}
          options={availableProjects}
          selected={filters.projects}
          onItemToggle={(item) =>
            toggleItem(filters.projects, item, onProjectChange)
          }
        />

        <FilterDropdownButton
          label="Branch"
          count={filters.branches.length}
          isOpen={openDropdown === 'branch'}
          onToggle={() => toggleDropdown('branch')}
          options={availableBranches}
          selected={filters.branches}
          onItemToggle={(item) =>
            toggleItem(filters.branches, item, onBranchChange)
          }
        />

        {/* Clear all */}
        {activeCount > 0 && (
          <button
            onClick={onClear}
            className="flex items-center gap-1 text-xs text-red-400 hover:text-red-300 whitespace-nowrap"
          >
            <Filter className="h-3 w-3" />
            Clear all ({activeCount})
          </button>
        )}
      </div>

      {/* Row 2: Active filter pills */}
      {activeCount > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {filters.statuses.map((s) => (
            <FilterPill
              key={`status-${s}`}
              label={s}
              onRemove={() => removeFilterPill('status', s)}
            />
          ))}
          {filters.projects.map((p) => (
            <FilterPill
              key={`project-${p}`}
              label={p}
              onRemove={() => removeFilterPill('project', p)}
            />
          ))}
          {filters.branches.map((b) => (
            <FilterPill
              key={`branch-${b}`}
              label={b}
              onRemove={() => removeFilterPill('branch', b)}
            />
          ))}
          {filters.search.trim() && (
            <FilterPill
              label={`"${filters.search}"`}
              onRemove={() => {
                setLocalSearch('')
                onSearchChange('')
              }}
            />
          )}
        </div>
      )}
    </div>
  )
}

// --- Internal sub-components ---

function FilterDropdownButton({
  label,
  count,
  isOpen,
  onToggle,
  options,
  selected,
  onItemToggle,
}: {
  label: string
  count: number
  isOpen: boolean
  onToggle: () => void
  options: string[]
  selected: string[]
  onItemToggle: (item: string) => void
}) {
  return (
    <div className="relative">
      <button
        onClick={onToggle}
        className={`flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-md border transition-colors ${
          count > 0
            ? 'border-indigo-500/40 text-indigo-400 bg-indigo-500/10'
            : 'border-gray-300 dark:border-gray-700 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50'
        }`}
      >
        {label}
        {count > 0 && (
          <span className="ml-0.5 px-1.5 py-0.5 text-[10px] rounded-full bg-indigo-500/20 text-indigo-400 leading-none">
            {count}
          </span>
        )}
        <ChevronDown
          className={`h-3 w-3 transition-transform ${isOpen ? 'rotate-180' : ''}`}
        />
      </button>

      {isOpen && (
        <div className="absolute z-20 mt-1 bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-lg shadow-xl p-2 min-w-[160px]">
          {options.length === 0 ? (
            <div className="text-xs text-gray-400 dark:text-gray-500 px-2 py-1">No options</div>
          ) : (
            options.map((option) => (
              <label
                key={option}
                className="flex items-center gap-2 px-2 py-1.5 text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={selected.includes(option)}
                  onChange={() => onItemToggle(option)}
                  className="h-3.5 w-3.5 rounded border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-indigo-500 focus:ring-indigo-500/30 focus:ring-offset-0"
                />
                <span className="truncate">{option}</span>
              </label>
            ))
          )}
        </div>
      )}
    </div>
  )
}

function FilterPill({
  label,
  onRemove,
}: {
  label: string
  onRemove: () => void
}) {
  return (
    <span className="inline-flex items-center px-2 py-1 text-xs rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/30">
      {label}
      <button
        onClick={onRemove}
        className="ml-1 hover:text-red-400 cursor-pointer"
      >
        <X className="h-3 w-3" />
      </button>
    </span>
  )
}
