// src/components/SessionToolbar.tsx
import { useState, useRef, useEffect } from 'react';
import { SortDesc, ChevronDown, Check, LayoutList, Table, RotateCcw } from 'lucide-react';
import { cn } from '../lib/utils';
import { FilterPopover } from './FilterPopover';
import type { SessionFilters, SessionSort, GroupBy, ViewMode } from '../hooks/use-session-filters';
import { countActiveFilters } from '../hooks/use-session-filters';

interface SessionToolbarProps {
  filters: SessionFilters;
  onFiltersChange: (filters: SessionFilters) => void;
  onClearFilters: () => void;
  groupByDisabled?: boolean;
  /** Available branch names derived from loaded sessions */
  branches?: string[];
  /** Available model IDs from indexed session data (data-driven) */
  models?: string[];
}

interface DropdownProps {
  label: string;
  icon: React.ReactNode;
  value: string;
  options: Array<{ value: string; label: string; description?: string }>;
  onChange: (value: string) => void;
  isActive?: boolean;
  disabled?: boolean;
  disabledTitle?: string;
}

function Dropdown({ label, icon, value, options, onChange, isActive, disabled, disabledTitle }: DropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    }
    if (isOpen) {
      document.addEventListener('mousedown', handleClick);
      return () => document.removeEventListener('mousedown', handleClick);
    }
  }, [isOpen]);

  const selectedOption = options.find((o) => o.value === value);

  return (
    <div className="relative group/dropdown" ref={ref}>
      <button
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        className={cn(
          'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border',
          'transition-all duration-150 ease-out',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
          disabled
            ? 'opacity-50 cursor-not-allowed bg-gray-50 dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-400 dark:text-gray-500'
            : isActive
              ? 'cursor-pointer bg-blue-50 dark:bg-blue-950/30 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300'
              : 'cursor-pointer bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-750'
        )}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-disabled={disabled}
        aria-label={`${label}: ${selectedOption?.label}`}
      >
        {icon}
        <span className="max-w-[120px] truncate">{selectedOption?.label || label}</span>
        <ChevronDown className={cn('w-3.5 h-3.5 transition-transform', isOpen && 'rotate-180')} />
      </button>

      {/* Styled tooltip for disabled state */}
      {disabled && disabledTitle && (
        <div className="absolute left-0 top-full mt-1.5 w-52 px-3 py-2 text-[11px] leading-relaxed text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-950/60 border border-amber-200 dark:border-amber-800 rounded-lg shadow-lg z-50 pointer-events-none opacity-0 group-hover/dropdown:opacity-100 transition-opacity duration-150">
          {disabledTitle}
        </div>
      )}

      {isOpen && (
        <div
          className="absolute top-full left-0 mt-1.5 min-w-[180px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50 py-1"
          role="listbox"
          aria-label={label}
        >
          {options.map((option) => {
            const isSelected = option.value === value;
            return (
              <button
                key={option.value}
                type="button"
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onChange(option.value);
                  setIsOpen(false);
                }}
                className={cn(
                  'w-full flex items-center gap-2 px-3 py-2 text-left transition-colors',
                  isSelected
                    ? 'bg-blue-50 dark:bg-blue-950/30 text-blue-700 dark:text-blue-300'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-750'
                )}
              >
                <div
                  className={cn(
                    'flex items-center justify-center w-4 h-4 rounded border',
                    isSelected ? 'bg-blue-600 border-blue-600' : 'border-gray-300 dark:border-gray-600'
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
                        : 'text-gray-700 dark:text-gray-300'
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
            );
          })}
        </div>
      )}
    </div>
  );
}

const SORT_OPTIONS: Array<{ value: SessionSort; label: string }> = [
  { value: 'recent', label: 'Most recent' },
  { value: 'tokens', label: 'Most tokens' },
  { value: 'prompts', label: 'Most prompts' },
  { value: 'files_edited', label: 'Most files edited' },
  { value: 'duration', label: 'Longest duration' },
];

const GROUP_BY_OPTIONS: Array<{ value: GroupBy; label: string; description?: string }> = [
  { value: 'none', label: 'None', description: 'Date-grouped list' },
  { value: 'branch', label: 'Branch', description: 'Group by git branch' },
  { value: 'project', label: 'Project', description: 'Group by project' },
  { value: 'model', label: 'Model', description: 'Group by primary model' },
  { value: 'day', label: 'Day', description: 'Group by calendar day' },
  { value: 'week', label: 'Week', description: 'Group by week (Monday start)' },
  { value: 'month', label: 'Month', description: 'Group by calendar month' },
];

/**
 * SessionToolbar component for filtering, sorting, and grouping sessions.
 *
 * Features:
 * - Group-by dropdown (none/branch/project/model/day/week/month)
 * - Filter trigger button with active count badge
 * - Sort dropdown
 * - All controls are inline in a single toolbar
 *
 * @example
 * ```tsx
 * <SessionToolbar
 *   filters={filters}
 *   onFiltersChange={setFilters}
 *   onClearFilters={() => setFilters(DEFAULT_FILTERS)}
 * />
 * ```
 */
export function SessionToolbar({ filters, onFiltersChange, onClearFilters, groupByDisabled, branches = [], models = [] }: SessionToolbarProps) {
  const activeFilterCount = countActiveFilters(filters);
  const hasNonDefaults = activeFilterCount > 0 || filters.sort !== 'recent' || filters.groupBy !== 'none';

  const handleSortChange = (sort: string) => {
    onFiltersChange({ ...filters, sort: sort as SessionSort });
  };

  const handleGroupByChange = (groupBy: string) => {
    onFiltersChange({ ...filters, groupBy: groupBy as GroupBy });
  };

  const handleViewModeChange = (viewMode: ViewMode) => {
    onFiltersChange({ ...filters, viewMode });
  };

  return (
    <div className="flex items-center justify-between gap-2">
      {/* Left side: Group by, Filter, Sort */}
      <div className="flex items-center gap-2">
        {/* Group by dropdown */}
        <Dropdown
          label="Group by"
          icon={<div className="w-3.5 h-3.5 flex items-center justify-center text-xs">⊞</div>}
          value={groupByDisabled ? 'none' : filters.groupBy}
          options={GROUP_BY_OPTIONS}
          onChange={handleGroupByChange}
          isActive={!groupByDisabled && filters.groupBy !== 'none'}
          disabled={groupByDisabled}
          disabledTitle="Grouping disabled — too many sessions. Use filters to narrow results."
        />

        {/* Filter popover */}
        <FilterPopover
          filters={filters}
          onChange={onFiltersChange}
          onClear={onClearFilters}
          activeCount={activeFilterCount}
          branches={branches}
          models={models}
        />

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
            onClick={onClearFilters}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1.5 text-[11px] font-medium rounded-md',
              'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1'
            )}
            aria-label="Reset all filters"
          >
            <RotateCcw className="w-3 h-3" />
            <span>Reset</span>
          </button>
        )}
      </div>

      {/* Right side: View mode toggle */}
      <div className="flex items-center gap-0.5 p-0.5 bg-gray-100 dark:bg-gray-800 rounded-md">
        <button
          type="button"
          onClick={() => handleViewModeChange('timeline')}
          className={cn(
            'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md transition-all',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            filters.viewMode === 'timeline'
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
          )}
          aria-label="Timeline view"
          aria-pressed={filters.viewMode === 'timeline'}
        >
          <LayoutList className="w-3.5 h-3.5" />
          <span>List</span>
        </button>
        <button
          type="button"
          onClick={() => handleViewModeChange('table')}
          className={cn(
            'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md transition-all',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            filters.viewMode === 'table'
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
          )}
          aria-label="Table view"
          aria-pressed={filters.viewMode === 'table'}
        >
          <Table className="w-3.5 h-3.5" />
          <span>Table</span>
        </button>
      </div>
    </div>
  );
}
