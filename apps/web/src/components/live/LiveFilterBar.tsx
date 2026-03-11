import { ArrowUpDown, ChevronDown, Filter, Layers, Loader2, Search, Square, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { IndexingPhase } from '../../hooks/use-indexing-progress'
import type { LiveSessionFilters } from './live-filter'
import type { KanbanGroupBy, KanbanSort } from './types'

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
  /** Current indexing phase — when set, shows spinner and phase-aware placeholder. */
  indexingPhase?: IndexingPhase
  /** Indexing percentage (0-100) shown in placeholder during deep-indexing. */
  indexingPercent?: number
  /** Number of sessions after filtering. */
  filteredCount?: number
  /** Total number of sessions before filtering. */
  totalCount?: number
  /** Kanban group-by value (only shown when in kanban view). */
  groupByValue?: KanbanGroupBy
  /** Callback when kanban group-by changes. */
  onGroupByChange?: (value: KanbanGroupBy) => void
  /** Kanban sort value (only shown when in kanban view with grouping active). */
  sortValue?: KanbanSort
  /** Callback when kanban sort changes. */
  onSortChange?: (value: KanbanSort) => void
  /** Whether the "Closed" column is visible (kanban only). */
  showClosed?: boolean
  /** Callback when show-closed toggle changes. */
  onShowClosedChange?: (value: boolean) => void
  /** Number of recently closed sessions (for badge when toggle is off). */
  closedCount?: number
}

type DropdownType = 'status' | 'project' | 'branch' | 'groupBy' | 'sort' | null

const GROUP_BY_OPTIONS: { value: KanbanGroupBy; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'project-branch', label: 'Project + Branch' },
]

const SORT_OPTIONS: { value: KanbanSort; label: string }[] = [
  { value: 'recent', label: 'Most recent' },
  { value: 'alphabetical', label: 'Alphabetical' },
  { value: 'cost', label: 'Highest cost' },
]

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
  indexingPhase,
  indexingPercent = 0,
  filteredCount,
  totalCount,
  groupByValue,
  onGroupByChange,
  sortValue,
  onSortChange,
  showClosed,
  onShowClosedChange,
  closedCount = 0,
}: LiveFilterBarProps) {
  const [localSearch, setLocalSearch] = useState(filters.search)
  const [openDropdown, setOpenDropdown] = useState<DropdownType>(null)
  const barRef = useRef<HTMLDivElement>(null)

  const isIndexing =
    indexingPhase &&
    indexingPhase !== 'done' &&
    indexingPhase !== 'error' &&
    indexingPhase !== 'idle'
  const searchPlaceholder = isIndexing
    ? indexingPhase === 'reading-indexes' || indexingPhase === 'ready'
      ? 'Preparing search...'
      : indexingPhase === 'finalizing'
        ? 'Building search index...'
        : `Search (indexing ${indexingPercent}%)...`
    : 'Search sessions...'

  const showFiltered =
    filteredCount !== undefined && totalCount !== undefined && filteredCount !== totalCount

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

  function toggleItem(current: string[], item: string, onChange: (items: string[]) => void) {
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
      {/* Row 1: Search + filter buttons + group-by + clear */}
      <div className="flex items-center gap-2">
        {/* Search input — grows but capped to keep filters close */}
        <div className="relative flex-1 max-w-sm min-w-0">
          {isIndexing ? (
            <Loader2 className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-blue-400 dark:text-blue-500 animate-spin motion-reduce:animate-none" />
          ) : (
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-gray-400 dark:text-gray-500" />
          )}
          <input
            ref={searchInputRef}
            type="text"
            value={localSearch}
            onChange={(e) => setLocalSearch(e.target.value)}
            placeholder={searchPlaceholder}
            aria-label={isIndexing ? 'Search sessions — indexing in progress' : 'Search sessions'}
            className="w-full bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-md pl-8 pr-8 py-1.5 text-sm text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30"
          />
          {localSearch && (
            <button
              type="button"
              onClick={() => {
                setLocalSearch('')
                onSearchChange('')
              }}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer"
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
          onItemToggle={(item) => toggleItem(filters.statuses, item, onStatusChange)}
        />

        <FilterDropdownButton
          label="Project"
          count={filters.projects.length}
          isOpen={openDropdown === 'project'}
          onToggle={() => toggleDropdown('project')}
          options={availableProjects}
          selected={filters.projects}
          onItemToggle={(item) => toggleItem(filters.projects, item, onProjectChange)}
        />

        <FilterDropdownButton
          label="Branch"
          count={filters.branches.length}
          isOpen={openDropdown === 'branch'}
          onToggle={() => toggleDropdown('branch')}
          options={availableBranches}
          selected={filters.branches}
          onItemToggle={(item) => toggleItem(filters.branches, item, onBranchChange)}
        />

        {/* Group-by dropdown — only in kanban view */}
        {groupByValue !== undefined && onGroupByChange && (
          <div className="relative">
            <button
              type="button"
              onClick={() => toggleDropdown('groupBy')}
              className={`flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-md border transition-colors ${
                groupByValue !== 'none'
                  ? 'border-indigo-500/40 text-indigo-400 bg-indigo-500/10'
                  : 'border-gray-300 dark:border-gray-700 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50'
              }`}
            >
              <Layers className="h-3 w-3" />
              <span className="hidden sm:inline">Group</span>
              <ChevronDown
                className={`h-3 w-3 transition-transform ${openDropdown === 'groupBy' ? 'rotate-180' : ''}`}
              />
            </button>

            {openDropdown === 'groupBy' && (
              <div className="absolute z-20 mt-1 right-0 bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-lg shadow-xl p-2 min-w-40">
                {GROUP_BY_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => {
                      onGroupByChange(opt.value)
                      setOpenDropdown(null)
                    }}
                    className={`w-full flex items-center gap-2 px-2 py-1.5 text-xs rounded cursor-pointer transition-colors ${
                      groupByValue === opt.value
                        ? 'text-indigo-400 bg-indigo-500/10'
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Sort dropdown — only in kanban view when grouping is active */}
        {sortValue !== undefined && onSortChange && groupByValue === 'project-branch' && (
          <div className="relative">
            <button
              type="button"
              onClick={() => toggleDropdown('sort')}
              className={`flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-md border transition-colors ${
                sortValue !== 'recent'
                  ? 'border-indigo-500/40 text-indigo-400 bg-indigo-500/10'
                  : 'border-gray-300 dark:border-gray-700 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50'
              }`}
            >
              <ArrowUpDown className="h-3 w-3" />
              <span className="hidden sm:inline">Sort</span>
              <ChevronDown
                className={`h-3 w-3 transition-transform ${openDropdown === 'sort' ? 'rotate-180' : ''}`}
              />
            </button>

            {openDropdown === 'sort' && (
              <div className="absolute z-20 mt-1 right-0 bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-lg shadow-xl p-2 min-w-44">
                {SORT_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => {
                      onSortChange(opt.value)
                      setOpenDropdown(null)
                    }}
                    className={`w-full flex items-center gap-2 px-2 py-1.5 text-xs rounded cursor-pointer transition-colors ${
                      sortValue === opt.value
                        ? 'text-indigo-400 bg-indigo-500/10'
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Show Closed toggle — only in kanban view */}
        {showClosed !== undefined && onShowClosedChange && (
          <button
            type="button"
            onClick={() => onShowClosedChange(!showClosed)}
            className={`flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-md border transition-colors cursor-pointer ${
              showClosed
                ? 'border-indigo-500/40 text-indigo-400 bg-indigo-500/10'
                : 'border-gray-300 dark:border-gray-700 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50'
            }`}
          >
            <Square className="h-3 w-3" />
            <span className="hidden sm:inline">Closed</span>
            {!showClosed && closedCount > 0 && (
              <span className="ml-0.5 px-1.5 py-0.5 text-[10px] rounded-full bg-zinc-500/20 text-zinc-400 leading-none">
                {closedCount}
              </span>
            )}
          </button>
        )}

        {/* Filtered count — sticks with filter buttons */}
        {showFiltered && (
          <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums whitespace-nowrap">
            {filteredCount} of {totalCount}
          </span>
        )}

        {/* Clear all */}
        {activeCount > 0 && (
          <button
            type="button"
            onClick={onClear}
            className="flex items-center gap-1 text-xs text-red-400 hover:text-red-300 whitespace-nowrap cursor-pointer"
          >
            <Filter className="h-3 w-3" />
            Clear ({activeCount})
          </button>
        )}
      </div>

      {/* Row 2: Active filter pills — only when filters are active */}
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
        type="button"
        onClick={onToggle}
        className={`flex items-center gap-1 px-2.5 py-1.5 text-xs rounded-md border transition-colors cursor-pointer ${
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
        <ChevronDown className={`h-3 w-3 transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>

      {isOpen && (
        <div className="absolute z-20 mt-1 bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-lg shadow-xl p-2 min-w-40">
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
    <span className="inline-flex items-center px-2 py-0.5 text-xs rounded-full bg-indigo-500/10 text-indigo-400 border border-indigo-500/30">
      {label}
      <button type="button" onClick={onRemove} className="ml-1 hover:text-red-400 cursor-pointer">
        <X className="h-3 w-3" />
      </button>
    </span>
  )
}
