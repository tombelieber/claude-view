import * as Popover from '@radix-ui/react-popover'
import { Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import { type PromptFilters, defaultPromptFilters } from '../../hooks/use-prompt-filters'
import { formatModelName } from '../../lib/format-model'
import { cn } from '../../lib/utils'

interface PromptFilterPopoverProps {
  filters: PromptFilters
  onFiltersChange: (filters: PromptFilters) => void
  children: React.ReactNode // trigger element
}

const INTENT_OPTIONS = [
  'fix',
  'create',
  'review',
  'explain',
  'ship',
  'refactor',
  'confirm',
  'command',
] as const

const COMPLEXITY_OPTIONS = [
  { label: 'Any', value: null },
  { label: 'Micro', value: 'micro' },
  { label: 'Short', value: 'short' },
  { label: 'Medium', value: 'medium' },
  { label: 'Detailed', value: 'detailed' },
  { label: 'Long', value: 'long' },
] as const

const TEMPLATE_OPTIONS = [
  { label: 'Any', value: null },
  { label: 'Is Template', value: 'template' },
  { label: 'Unique', value: 'unique' },
] as const

export function PromptFilterPopover({
  filters,
  onFiltersChange,
  children,
}: PromptFilterPopoverProps) {
  const [branchSearch, setBranchSearch] = useState('')
  const [debouncedSearch, setDebouncedSearch] = useState('')

  // Debounce branch search (150ms)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(branchSearch), 150)
    return () => clearTimeout(timer)
  }, [branchSearch])

  // Filter branches by debounced search query
  const filteredBranches = (filters.availableBranches ?? [])
    .filter((branch) => branch !== '')
    .filter((branch) => branch.toLowerCase().includes(debouncedSearch.toLowerCase()))

  const filteredModels = filters.availableModels ?? []

  const hasAnyFiltersSet =
    filters.intents.length > 0 ||
    filters.models.length > 0 ||
    filters.branches.length > 0 ||
    filters.hasPaste !== 'any' ||
    filters.complexity !== null ||
    filters.templateMatch !== null

  return (
    <Popover.Root
      onOpenChange={(open) => {
        if (open) {
          setBranchSearch('')
        }
      }}
    >
      <Popover.Trigger asChild>{children}</Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="start"
          sideOffset={6}
          className="z-50 w-[320px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg animate-in fade-in-0 zoom-in-95"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Filters</h3>
            {hasAnyFiltersSet && (
              <span className="text-[10px] text-gray-400 tabular-nums">active</span>
            )}
          </div>

          {/* Filter options */}
          <div className="px-4 py-3 max-h-[400px] overflow-y-auto space-y-4">
            {/* Intent filter — multi-select toggle buttons */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Intent
              </label>
              <div className="flex flex-wrap gap-1.5">
                {INTENT_OPTIONS.map((intent) => {
                  const isSelected = filters.intents.includes(intent)
                  return (
                    <button
                      key={intent}
                      type="button"
                      onClick={() => {
                        const newIntents = isSelected
                          ? filters.intents.filter((i) => i !== intent)
                          : [...filters.intents, intent]
                        onFiltersChange({ ...filters, intents: newIntents })
                      }}
                      className={cn(
                        'px-2 py-1 text-xs rounded border cursor-pointer transition-colors capitalize',
                        isSelected
                          ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                          : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600',
                      )}
                    >
                      {intent}
                    </button>
                  )
                })}
              </div>
            </div>

            {/* Model filter — checkbox list */}
            {filteredModels.length > 0 && (
              <div>
                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Model
                </label>
                <div className="flex flex-wrap gap-2">
                  {filteredModels.map((model) => (
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
                            : filters.models.filter((m) => m !== model)
                          onFiltersChange({ ...filters, models: newModels })
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

            {/* Branch filter — checkbox list with search */}
            {(filters.availableBranches ?? []).length > 0 && (
              <div>
                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Branch
                </label>
                {(filters.availableBranches ?? []).length > 5 && (
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
                <div className="max-h-[120px] overflow-y-auto border border-gray-200 dark:border-gray-700 rounded">
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
                              : filters.branches.filter((b) => b !== branch)
                            onFiltersChange({ ...filters, branches: newBranches })
                          }}
                          className="w-3.5 h-3.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                        />
                        <span className="ml-2 text-xs text-gray-700 dark:text-gray-300 truncate">
                          {branch}
                        </span>
                      </label>
                    ))
                  )}
                </div>
              </div>
            )}

            {/* Has Paste — 3-button toggle */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Has Paste
              </label>
              <div className="flex gap-2">
                {(['any', 'yes', 'no'] as const).map((option) => (
                  <button
                    key={option}
                    type="button"
                    onClick={() => onFiltersChange({ ...filters, hasPaste: option })}
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.hasPaste === option
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600',
                    )}
                  >
                    {option === 'any' ? 'Any' : option === 'yes' ? 'Yes' : 'No'}
                  </button>
                ))}
              </div>
            </div>

            {/* Complexity — pill selector */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Complexity
              </label>
              <div className="flex flex-wrap gap-1.5">
                {COMPLEXITY_OPTIONS.map((option) => (
                  <button
                    key={option.label}
                    type="button"
                    onClick={() => onFiltersChange({ ...filters, complexity: option.value })}
                    className={cn(
                      'px-2 py-1 text-xs rounded border cursor-pointer transition-colors',
                      filters.complexity === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600',
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Template Match — toggle */}
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
                Template Match
              </label>
              <div className="flex gap-2">
                {TEMPLATE_OPTIONS.map((option) => (
                  <button
                    key={option.label}
                    type="button"
                    onClick={() => onFiltersChange({ ...filters, templateMatch: option.value })}
                    className={cn(
                      'flex-1 px-2 py-1.5 text-xs rounded border cursor-pointer transition-colors',
                      filters.templateMatch === option.value
                        ? 'bg-blue-50 dark:bg-blue-950/30 border-blue-500 dark:border-blue-600 text-blue-700 dark:text-blue-300'
                        : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-gray-300 dark:hover:border-gray-600',
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
              onClick={() => {
                onFiltersChange({
                  ...defaultPromptFilters,
                  availableBranches: filters.availableBranches,
                  availableModels: filters.availableModels,
                })
              }}
              disabled={!hasAnyFiltersSet}
              className={cn(
                'px-3 py-1.5 text-xs font-medium rounded transition-colors',
                hasAnyFiltersSet
                  ? 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 hover:bg-red-100 dark:hover:bg-red-950/50 cursor-pointer'
                  : 'text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 cursor-not-allowed',
              )}
            >
              Reset all
            </button>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
