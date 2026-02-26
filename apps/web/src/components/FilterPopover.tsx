// src/components/FilterPopover.tsx
import { useState, useRef, useEffect } from 'react';
import { Filter, Search } from 'lucide-react';
import { cn } from '../lib/utils';
import { formatModelName } from '../lib/format-model';
import type { SessionFilters } from '../hooks/use-session-filters';

interface FilterPopoverProps {
  filters: SessionFilters;
  onChange: (filters: SessionFilters) => void;
  onClear: () => void;
  activeCount: number;
  /** Available branch names derived from loaded sessions */
  branches: string[];
  /** Available model IDs from indexed session data (data-driven) */
  models?: string[];
}

export function FilterPopover({ filters, onChange, onClear, activeCount, branches = [], models = [] }: FilterPopoverProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [branchSearch, setBranchSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const ref = useRef<HTMLDivElement>(null);

  // Debounce branch search (150ms)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(branchSearch), 150);
    return () => clearTimeout(timer);
  }, [branchSearch]);

  // Reset branch search when popover opens
  const prevIsOpenRef = useRef(false);
  useEffect(() => {
    if (isOpen && !prevIsOpenRef.current) {
      setBranchSearch('');
    }
    prevIsOpenRef.current = isOpen;
  }, [isOpen]);

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

  // Close on Escape key
  useEffect(() => {
    function handleEscape(e: KeyboardEvent) {
      if (e.key === 'Escape' && isOpen) {
        setIsOpen(false);
      }
    }
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen]);

  // Filter branches by debounced search query (exclude unnamed/empty branches)
  const filteredBranches = branches
    .filter((branch) => branch !== '')
    .filter((branch) => branch.toLowerCase().includes(debouncedSearch.toLowerCase()));

  const modelOptions = models;

  const hasAnyFiltersSet = activeCount > 0;

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          'inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium rounded-md border cursor-pointer',
          'transition-all duration-150 ease-out',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
          hasAnyFiltersSet
            ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-300'
            : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-750'
        )}
        aria-expanded={isOpen}
        aria-label={`Filters ${activeCount > 0 ? `(${activeCount} active)` : ''}`}
      >
        <Filter className="w-3.5 h-3.5" />
        <span>Filters</span>
        {activeCount > 0 && (
          <span className="inline-flex items-center justify-center min-w-[16px] h-4 px-1 text-[10px] font-semibold rounded-full bg-blue-600 dark:bg-blue-500 text-white">
            {activeCount}
          </span>
        )}
      </button>

      {isOpen && (
        <div
          className="absolute top-full left-0 mt-1.5 w-[320px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg z-50"
          role="dialog"
          aria-label="Filter sessions"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Filters</h3>
            {hasAnyFiltersSet && (
              <span className="text-[10px] text-gray-400 tabular-nums">{activeCount} active</span>
            )}
          </div>

          {/* Filter options */}
          <div className="px-4 py-3 max-h-[400px] overflow-y-auto space-y-4">
            {/* Commits filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Commits
              </label>
              <div className="flex gap-2">
                {['any', 'yes', 'no'].map((option) => (
                  <button
                    key={option}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, hasCommits: option as 'any' | 'yes' | 'no' })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.hasCommits === option
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option === 'any' ? 'Any' : option === 'yes' ? 'Has' : 'None'}
                  </button>
                ))}
              </div>
            </div>

            {/* Duration filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Duration
              </label>
              <div className="flex gap-2">
                {[
                  { label: 'Any', value: null },
                  { label: '>30m', value: 1800 },
                  { label: '>1h', value: 3600 },
                  { label: '>2h', value: 7200 },
                ].map((option) => (
                  <button
                    key={option.label}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, minDuration: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.minDuration === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Branch filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Branch
              </label>
              {branches.length > 5 && (
                <div className="relative mb-2">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
                  <input
                    type="text"
                    placeholder="Search branches..."
                    value={branchSearch}
                    onChange={(e) => setBranchSearch(e.target.value)}
                    className="w-full pl-7 pr-3 py-1.5 text-xs border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-400"
                  />
                </div>
              )}
              <div className="max-h-[160px] overflow-y-auto border border-gray-200 dark:border-gray-700 rounded">
                {filteredBranches.length === 0 ? (
                  <div className="px-3 py-2 text-xs text-gray-500">No branches found</div>
                ) : (
                  filteredBranches.map((branch) => (
                    <label
                      key={branch}
                      className="flex items-center px-3 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-750 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={filters.branches.includes(branch)}
                        onChange={(e) => {
                          const newBranches = e.target.checked
                            ? [...filters.branches, branch]
                            : filters.branches.filter((b) => b !== branch);
                          onChange({ ...filters, branches: newBranches });
                        }}
                        className="w-3.5 h-3.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                      />
                      <span className="ml-2 text-xs text-gray-700 dark:text-gray-300 truncate">{branch}</span>
                    </label>
                  ))
                )}
              </div>
            </div>

            {/* Model filter */}
            {modelOptions.length > 0 && (
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Model
              </label>
              <div className="flex flex-wrap gap-2">
                {modelOptions.map((model) => (
                  <label
                    key={model}
                    className="flex items-center px-2 py-1.5 border border-gray-200 dark:border-gray-700 rounded hover:bg-gray-50 dark:hover:bg-gray-750 cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={filters.models.includes(model)}
                      onChange={(e) => {
                        const newModels = e.target.checked
                          ? [...filters.models, model]
                          : filters.models.filter((m) => m !== model);
                        onChange({ ...filters, models: newModels });
                      }}
                      className="w-3.5 h-3.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                    />
                    <span className="ml-2 text-xs text-gray-700 dark:text-gray-300">
                      {formatModelName(model)}
                    </span>
                  </label>
                ))}
              </div>
            </div>
            )}

            {/* Has skills filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Has skills
              </label>
              <div className="flex gap-2">
                {['any', 'yes', 'no'].map((option) => (
                  <button
                    key={option}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, hasSkills: option as 'any' | 'yes' | 'no' })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.hasSkills === option
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option === 'any' ? 'Any' : option === 'yes' ? 'Yes' : 'No'}
                  </button>
                ))}
              </div>
            </div>

            {/* Re-edit rate filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Re-edit rate
              </label>
              <div className="flex gap-2">
                {[
                  { label: 'Any', value: null },
                  { label: 'High (>20%)', value: true },
                ].map((option) => (
                  <button
                    key={String(option.value)}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, highReedit: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.highReedit === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {/* File count filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Files edited
              </label>
              <div className="flex gap-2">
                {[
                  { label: 'Any', value: null },
                  { label: '>5', value: 5 },
                  { label: '>10', value: 10 },
                  { label: '>20', value: 20 },
                ].map((option) => (
                  <button
                    key={String(option.value)}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, minFiles: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.minFiles === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Token range filter */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Token usage
              </label>
              <div className="flex gap-2">
                {[
                  { label: 'Any', value: null },
                  { label: '>10K', value: 10000 },
                  { label: '>50K', value: 50000 },
                  { label: '>100K', value: 100000 },
                ].map((option) => (
                  <button
                    key={String(option.value)}
                    type="button"
                    onClick={() =>
                      onChange({ ...filters, minTokens: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.minTokens === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600'
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end px-4 py-3 border-t border-gray-200 dark:border-gray-700">
            <button
              type="button"
              onClick={() => { onClear(); }}
              disabled={!hasAnyFiltersSet}
              className={cn(
                'px-3 py-1.5 text-xs font-medium rounded transition-colors',
                hasAnyFiltersSet
                  ? 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 hover:bg-red-100 dark:hover:bg-red-950/50 cursor-pointer'
                  : 'text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 cursor-not-allowed'
              )}
            >
              Reset all
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
