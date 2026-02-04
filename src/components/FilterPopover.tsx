// src/components/FilterPopover.tsx
import { useState, useRef, useEffect } from 'react';
import { Filter, X, Search } from 'lucide-react';
import { cn } from '../lib/utils';
import type { SessionFilters } from '../hooks/use-session-filters';
import { useBranches } from '../hooks/use-branches';

interface FilterPopoverProps {
  filters: SessionFilters;
  onChange: (filters: SessionFilters) => void;
  onClear: () => void;
  activeCount: number;
}

/**
 * Extended filter popover panel with all session filter options.
 *
 * Features:
 * - Commits filter (radio: any/has/none)
 * - Duration filter (radio: any/>30m/>1h/>2h)
 * - Branch filter (searchable checkboxes with 150ms debounce)
 * - Model filter (checkboxes)
 * - Has skills filter (radio: any/yes/no)
 * - Re-edit rate filter (radio: any/high >20%)
 * - Files edited filter (radio: any/>5/>10/>20)
 * - Token usage filter (radio: any/>10K/>50K/>100K)
 * - Apply button (filters don't apply until clicked)
 * - Clear link to reset all filters
 * - Escape key closes without applying
 */
export function FilterPopover({ filters, onChange, onClear, activeCount }: FilterPopoverProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [draftFilters, setDraftFilters] = useState<SessionFilters>(filters);
  const [branchSearch, setBranchSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const ref = useRef<HTMLDivElement>(null);

  const { data: allBranches = [], isLoading: branchesLoading } = useBranches();

  // Debounce branch search (150ms)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(branchSearch), 150);
    return () => clearTimeout(timer);
  }, [branchSearch]);

  // Reset draft filters when popover opens
  useEffect(() => {
    if (isOpen) {
      setDraftFilters(filters);
      setBranchSearch('');
    }
  }, [isOpen, filters]);

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

  const handleApply = () => {
    onChange(draftFilters);
    setIsOpen(false);
  };

  const handleClear = () => {
    onClear();
    setIsOpen(false);
  };

  // Filter branches by debounced search query
  const filteredBranches = allBranches.filter((branch) =>
    branch.toLowerCase().includes(debouncedSearch.toLowerCase())
  );

  // Model options (hardcoded for now - could be fetched from API like branches)
  const modelOptions = ['claude-opus-4', 'claude-sonnet-4', 'claude-haiku-4'];

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
            <button
              type="button"
              onClick={handleClear}
              className="text-xs text-blue-600 dark:text-blue-400 hover:underline"
              disabled={!hasAnyFiltersSet}
            >
              Clear
            </button>
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
                      setDraftFilters({ ...draftFilters, hasCommits: option as 'any' | 'yes' | 'no' })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.hasCommits === option
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
                      setDraftFilters({ ...draftFilters, minDuration: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.minDuration === option.value
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
              <div className="max-h-[200px] overflow-y-auto border border-gray-200 dark:border-gray-700 rounded">
                {branchesLoading ? (
                  <div className="px-3 py-2 text-xs text-gray-500">Loading branches...</div>
                ) : filteredBranches.length === 0 ? (
                  <div className="px-3 py-2 text-xs text-gray-500">No branches found</div>
                ) : (
                  filteredBranches.map((branch) => (
                    <label
                      key={branch}
                      className="flex items-center px-3 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-750 cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={draftFilters.branches.includes(branch)}
                        onChange={(e) => {
                          const newBranches = e.target.checked
                            ? [...draftFilters.branches, branch]
                            : draftFilters.branches.filter((b) => b !== branch);
                          setDraftFilters({ ...draftFilters, branches: newBranches });
                        }}
                        className="w-3.5 h-3.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                      />
                      <span className="ml-2 text-xs text-gray-700 dark:text-gray-300">{branch}</span>
                    </label>
                  ))
                )}
              </div>
            </div>

            {/* Model filter */}
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
                      checked={draftFilters.models.includes(model)}
                      onChange={(e) => {
                        const newModels = e.target.checked
                          ? [...draftFilters.models, model]
                          : draftFilters.models.filter((m) => m !== model);
                        setDraftFilters({ ...draftFilters, models: newModels });
                      }}
                      className="w-3.5 h-3.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                    />
                    <span className="ml-2 text-xs text-gray-700 dark:text-gray-300">
                      {model.replace('claude-', '')}
                    </span>
                  </label>
                ))}
              </div>
            </div>

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
                      setDraftFilters({ ...draftFilters, hasSkills: option as 'any' | 'yes' | 'no' })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.hasSkills === option
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
                      setDraftFilters({ ...draftFilters, highReedit: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.highReedit === option.value
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
                      setDraftFilters({ ...draftFilters, minFiles: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.minFiles === option.value
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
                      setDraftFilters({ ...draftFilters, minTokens: option.value })
                    }
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      draftFilters.minTokens === option.value
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
          <div className="flex items-center justify-between px-4 py-3 border-t border-gray-200 dark:border-gray-700">
            <div className="text-xs text-gray-500">
              {activeCount > 0 ? `${activeCount} active` : 'No filters'}
            </div>
            <button
              type="button"
              onClick={handleApply}
              className="px-3 py-1.5 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 rounded focus:outline-none focus:ring-2 focus:ring-blue-400 focus:ring-offset-1"
            >
              Apply
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
