import { Loader2, MessageSquareText, Search, X } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { PromptCard } from '../components/prompts/PromptCard'
import { PromptProfile } from '../components/prompts/PromptProfile'
import { PromptTemplates } from '../components/prompts/PromptTemplates'
import { PromptToolbar } from '../components/prompts/PromptToolbar'
import { DateRangePicker, TimeRangeSelector } from '../components/ui'
import { useDebounce } from '../hooks/use-debounce'
import { usePromptFilters } from '../hooks/use-prompt-filters'
import { type PromptsQueryParams, usePromptsInfinite } from '../hooks/use-prompts-infinite'
import { useTimeRange } from '../hooks/use-time-range'

export function PromptsPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const sidebarProject = searchParams.get('project')

  const [searchText, setSearchText] = useState('')
  const debouncedSearch = useDebounce(searchText, 300)
  const sentinelRef = useRef<HTMLDivElement>(null)

  const { state: timeRange, setPreset, setCustomRange } = useTimeRange()
  const [filters, setFilters] = usePromptFilters(searchParams, setSearchParams)

  const queryParams: PromptsQueryParams = {
    search: debouncedSearch || undefined,
    project: sidebarProject || undefined,
    intents: filters.intents.length > 0 ? filters.intents : undefined,
    branches: filters.branches.length > 0 ? filters.branches : undefined,
    models: filters.models.length > 0 ? filters.models : undefined,
    hasPaste: filters.hasPaste,
    complexity: filters.complexity || undefined,
    templateMatch: filters.templateMatch || undefined,
    sort: filters.sort,
    timeAfter: timeRange.fromTimestamp || undefined,
    timeBefore: timeRange.toTimestamp || undefined,
  }

  const { data, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage } =
    usePromptsInfinite(queryParams)

  const prompts = data?.prompts ?? []
  const total = data?.total ?? 0

  // Infinite scroll observer
  const handleObserver = useCallback(
    (entries: IntersectionObserverEntry[]) => {
      const [target] = entries
      if (target.isIntersecting && hasNextPage && !isFetchingNextPage) {
        fetchNextPage()
      }
    },
    [hasNextPage, isFetchingNextPage, fetchNextPage],
  )

  useEffect(() => {
    const el = sentinelRef.current
    if (!el) return
    const observer = new IntersectionObserver(handleObserver, { threshold: 0.1 })
    observer.observe(el)
    return () => observer.disconnect()
  }, [handleObserver])

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-6 pb-2 flex items-center justify-between flex-wrap gap-2">
        <div className="flex items-center gap-2">
          <MessageSquareText className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Prompts</h1>
        </div>
        <div className="flex items-center gap-2">
          <TimeRangeSelector
            value={timeRange.preset}
            onChange={setPreset}
            options={[
              { value: 'today', label: 'Today' },
              { value: '7d', label: '7d' },
              { value: '30d', label: '30d' },
              { value: '90d', label: '90d' },
              { value: 'all', label: 'All' },
              { value: 'custom', label: 'Custom' },
            ]}
          />
          {timeRange.preset === 'custom' && (
            <DateRangePicker value={timeRange.customRange} onChange={setCustomRange} />
          )}
        </div>
      </div>

      <div className="px-6 pb-6 space-y-4">
        {/* Search input */}
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            placeholder="Search prompts..."
            aria-label="Search prompts"
            className="w-full pl-9 pr-9 py-2.5 text-sm bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg outline-none transition-colors focus:bg-white dark:focus:bg-gray-900 focus:border-gray-400 dark:focus:border-gray-500 focus:ring-1 focus:ring-gray-400/20 dark:focus:ring-gray-500/20 placeholder:text-gray-400 text-gray-900 dark:text-gray-100"
          />
          {searchText && (
            <button
              type="button"
              onClick={() => setSearchText('')}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Toolbar */}
        <PromptToolbar filters={filters} onFiltersChange={setFilters} totalCount={total} />

        {/* Collapsible sections */}
        <PromptProfile />
        <PromptTemplates />

        {/* Prompt feed */}
        {isLoading ? (
          <div className="flex items-center justify-center py-12 text-gray-400">
            <Loader2 className="w-5 h-5 animate-spin mr-2" />
            Loading prompts...
          </div>
        ) : prompts.length === 0 ? (
          <div className="text-center py-12 text-gray-400 dark:text-gray-500">
            {debouncedSearch ? 'No prompts match your search.' : 'No prompt history found.'}
          </div>
        ) : (
          <div className="space-y-2">
            {prompts.map((prompt) => (
              <PromptCard key={prompt.id} prompt={prompt} />
            ))}
          </div>
        )}

        {/* Infinite scroll sentinel */}
        <div ref={sentinelRef} className="h-4" />
        {isFetchingNextPage && (
          <div className="flex items-center justify-center py-4 text-gray-400">
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
            Loading more...
          </div>
        )}
      </div>
    </div>
  )
}
