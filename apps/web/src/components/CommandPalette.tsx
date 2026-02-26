import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { Search, X, FolderOpen, Clock, Loader2, LayoutGrid, List, Columns3, Monitor, Filter, Trash2, HelpCircle } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useAppStore } from '../store/app-store'
import { useSearch } from '../hooks/use-search'
import { SearchResultCard } from './SearchResultCard'
import { cn } from '../lib/utils'
import { cleanPreviewText } from '../utils/get-session-title'
import type { LiveSession } from './live/use-live-sessions'
import type { LiveViewMode } from './live/types'
import type { LiveSortField } from './live/live-filter'

interface LiveMonitorContext {
  sessions: LiveSession[]
  viewMode: LiveViewMode
  onViewModeChange: (mode: LiveViewMode) => void
  onFilterStatus: (statuses: string[]) => void
  onClearFilters: () => void
  onSort: (field: LiveSortField) => void
  onSelectSession: (id: string) => void
  onToggleHelp: () => void
}

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  projects: ProjectSummary[]
  liveContext?: LiveMonitorContext
}

type SuggestionType = 'project' | 'recent'

interface Suggestion {
  type: SuggestionType
  label: string
  query: string
  count?: number
}

type LiveCommandActionType =
  | 'switch-view'
  | 'filter-status'
  | 'sort-by'
  | 'select-session'
  | 'clear-filters'
  | 'toggle-help'

interface LiveCommandItem {
  id: string
  label: string
  description?: string
  icon: React.ComponentType<{ className?: string }> | null
  actionType: LiveCommandActionType
  actionPayload?: any
  keywords: string[]
  shortcut?: string
}

export function CommandPalette({ isOpen, onClose, projects, liveContext }: CommandPaletteProps) {
  const location = useLocation()
  const isLiveMonitor = location.pathname === '/' && !!liveContext
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const navigate = useNavigate()
  const { recentSearches, addRecentSearch } = useAppStore()

  // Live search results from Tantivy backend
  const { data: searchResults, isLoading: isSearching, isDebouncing } = useSearch(query, {
    enabled: isOpen,
    limit: 5,
  })

  // Reset when opened
  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus()
      setQuery('')
      setSelectedIndex(0)
    }
  }, [isOpen])

  // Generate Live Monitor commands
  const liveCommands = useMemo<LiveCommandItem[]>(() => {
    if (!isLiveMonitor || !liveContext) return []
    const items: LiveCommandItem[] = []

    // View modes
    const viewModes: {
      mode: LiveViewMode
      label: string
      icon: React.ComponentType<{ className?: string }>
      shortcut: string
      extraKeywords: string[]
    }[] = [
      { mode: 'grid', label: 'Grid view', icon: LayoutGrid, shortcut: '1', extraKeywords: ['grid'] },
      { mode: 'list', label: 'List view', icon: List, shortcut: '2', extraKeywords: ['list'] },
      { mode: 'kanban', label: 'Board view', icon: Columns3, shortcut: '3', extraKeywords: ['board', 'kanban'] },
      { mode: 'monitor', label: 'Monitor view', icon: Monitor, shortcut: '4', extraKeywords: ['monitor'] },
    ]

    for (const vm of viewModes) {
      items.push({
        id: `view-${vm.mode}`,
        label: vm.label,
        icon: vm.icon,
        actionType: 'switch-view',
        actionPayload: vm.mode,
        keywords: ['view', 'switch', ...vm.extraKeywords],
        shortcut: vm.shortcut,
      })
    }

    // Filter actions
    items.push(
      { id: 'filter-needs-you', label: 'Show sessions needing you', icon: Filter, actionType: 'filter-status', actionPayload: 'needs_you', keywords: ['filter', 'show', 'needs', 'attention'] },
      { id: 'filter-autonomous', label: 'Show autonomous sessions', icon: Filter, actionType: 'filter-status', actionPayload: 'autonomous', keywords: ['filter', 'show', 'autonomous'] },
      { id: 'clear-filters', label: 'Clear all filters', icon: Trash2, actionType: 'clear-filters', keywords: ['clear', 'reset', 'filter', 'remove'] }
    )

    // Sort actions
    items.push(
      { id: 'sort-last-active', label: 'Sort by last active', icon: null, actionType: 'sort-by', actionPayload: 'last_active', keywords: ['sort', 'order', 'active'] },
      { id: 'sort-cost', label: 'Sort by cost', icon: null, actionType: 'sort-by', actionPayload: 'cost', keywords: ['sort', 'order', 'cost'] },
      { id: 'sort-turns', label: 'Sort by turns', icon: null, actionType: 'sort-by', actionPayload: 'turns', keywords: ['sort', 'order', 'turns'] }
    )

    // Help
    items.push({ id: 'toggle-help', label: 'Keyboard shortcuts', icon: HelpCircle, actionType: 'toggle-help', keywords: ['help', 'keyboard', 'shortcuts', 'keys'] })

    // Sessions (if query matches)
    if (query.trim()) {
      const q = query.toLowerCase()
      for (const session of liveContext.sessions.slice(0, 5)) {
        const branchLabel = session.gitBranch ?? 'no branch'
        const projectLabel = session.projectDisplayName || session.project
        if (projectLabel.toLowerCase().includes(q) || branchLabel.toLowerCase().includes(q)) {
          items.push({
            id: `session-${session.id}`,
            label: `${projectLabel} — ${branchLabel}`,
            description: cleanPreviewText(session.lastUserMessage).slice(0, 50),
            icon: null,
            actionType: 'select-session',
            actionPayload: session.id,
            keywords: [session.project, session.gitBranch ?? '', session.id].filter(Boolean),
          })
        }
      }
    }

    return items
  }, [isLiveMonitor, liveContext, query])

  // Filter live commands by query
  const filteredLiveCommands = useMemo(() => {
    if (!query.trim()) return liveCommands
    const q = query.toLowerCase()
    return liveCommands.filter(item => {
      const allText = [item.label, item.description ?? '', ...item.keywords].join(' ').toLowerCase()
      return allText.includes(q)
    }).slice(0, 8)
  }, [liveCommands, query])

  // Generate suggestions based on query (project autocomplete + recent searches)
  const suggestions = useMemo((): Suggestion[] => {
    const results: Suggestion[] = []
    const q = query.toLowerCase().trim()

    if (!q) {
      // Show recent searches when no query
      for (const recent of recentSearches.slice(0, 3)) {
        results.push({ type: 'recent', label: recent, query: recent })
      }
      return results
    }

    // Autocomplete project names
    if (q.startsWith('project:') || !q.includes(':')) {
      const searchTerm = q.startsWith('project:') ? q.slice(8) : q
      for (const project of projects) {
        if (project.displayName.toLowerCase().includes(searchTerm)) {
          results.push({
            type: 'project',
            label: project.displayName,
            query: `project:${project.displayName}`,
            count: project.sessionCount,
          })
        }
        if (results.length >= 5) break
      }
    }

    return results.slice(0, 8)
  }, [query, projects, recentSearches])

  // Total navigable items: live commands + suggestions + search result sessions
  const searchSessionCount = searchResults?.sessions.length ?? 0
  const liveCommandsCount = filteredLiveCommands.length
  const suggestionsStartIndex = liveCommandsCount
  const searchResultsStartIndex = liveCommandsCount + suggestions.length
  const totalItems = liveCommandsCount + suggestions.length + searchSessionCount

  const handleSelect = useCallback((searchQuery: string) => {
    addRecentSearch(searchQuery)
    onClose()
    // Build search URL, inheriting active sidebar project/branch filters as scope
    const searchUrl = new URLSearchParams()
    searchUrl.set('q', searchQuery)
    const currentParams = new URLSearchParams(location.search)
    const project = currentParams.get('project')
    const branch = currentParams.get('branch')
    const scopeParts: string[] = []
    if (project) scopeParts.push(`project:${project}`)
    if (branch) scopeParts.push(`branch:${branch}`)
    if (scopeParts.length > 0) searchUrl.set('scope', scopeParts.join(' '))
    navigate(`/search?${searchUrl}`)
  }, [addRecentSearch, onClose, navigate, location.search])

  const handleSelectSearchResult = useCallback((sessionId: string) => {
    onClose()
    navigate(`/sessions/${sessionId}`)
  }, [onClose, navigate])

  const executeLiveCommand = useCallback((item: LiveCommandItem) => {
    if (!liveContext) return
    switch (item.actionType) {
      case 'switch-view':
        liveContext.onViewModeChange(item.actionPayload as LiveViewMode)
        break
      case 'filter-status':
        liveContext.onFilterStatus([item.actionPayload as string])
        break
      case 'sort-by':
        liveContext.onSort(item.actionPayload as LiveSortField)
        break
      case 'select-session':
        liveContext.onSelectSession(item.actionPayload as string)
        break
      case 'clear-filters':
        liveContext.onClearFilters()
        break
      case 'toggle-help':
        liveContext.onToggleHelp()
        break
    }
    onClose()
  }, [liveContext, onClose])

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (query.trim()) {
      handleSelect(query.trim())
    }
  }, [query, handleSelect])

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      } else if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIndex(i => Math.min(i + 1, totalItems - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIndex(i => Math.max(i - 1, 0))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        // Live commands section
        if (selectedIndex < liveCommandsCount && filteredLiveCommands[selectedIndex]) {
          executeLiveCommand(filteredLiveCommands[selectedIndex])
        }
        // Suggestions section
        else if (selectedIndex >= suggestionsStartIndex && selectedIndex < searchResultsStartIndex) {
          const suggestionIdx = selectedIndex - suggestionsStartIndex
          if (suggestions[suggestionIdx]) {
            handleSelect(suggestions[suggestionIdx].query)
          }
        }
        // Search results section
        else if (selectedIndex >= searchResultsStartIndex && searchResults) {
          const sessionIdx = selectedIndex - searchResultsStartIndex
          const session = searchResults.sessions[sessionIdx]
          if (session) {
            handleSelectSearchResult(session.sessionId)
          }
        }
        // Fallback: search with query
        else if (query.trim()) {
          handleSelect(query.trim())
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose, filteredLiveCommands, suggestions, selectedIndex, query, totalItems, searchResults, liveCommandsCount, suggestionsStartIndex, searchResultsStartIndex, executeLiveCommand, handleSelect, handleSelectSearchResult])

  const insertFilter = useCallback((filter: string) => {
    setQuery(prev => {
      const trimmed = prev.trim()
      return trimmed ? `${trimmed} ${filter}` : filter
    })
    inputRef.current?.focus()
  }, [])

  const getIcon = (type: SuggestionType) => {
    switch (type) {
      case 'project': return FolderOpen
      case 'recent': return Clock
    }
  }

  if (!isOpen) return null

  const hasLiveResults = query.trim().length > 0 && searchResults && searchResults.sessions.length > 0
  const showLoading = query.trim().length > 0 && (isSearching || isDebouncing)

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[12vh]">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-md"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-xl bg-white/90 dark:bg-[#0c0e16]/90 backdrop-blur-xl rounded-xl shadow-2xl shadow-black/20 dark:shadow-black/50 border border-slate-200/80 dark:border-white/[0.06] ring-1 ring-black/5 dark:ring-white/[0.05] overflow-hidden">
        {/* Search input */}
        <form onSubmit={handleSubmit}>
          <div className="flex items-center gap-3 px-4 py-3 border-b border-slate-200/80 dark:border-white/[0.06]">
            <Search className="w-5 h-5 text-slate-400 dark:text-slate-500" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value)
                setSelectedIndex(0)
              }}
              placeholder={isLiveMonitor ? "Search or type a command..." : "Search sessions..."}
              className="flex-1 bg-transparent text-slate-900 dark:text-slate-100 placeholder-slate-400 dark:placeholder-slate-500 outline-none font-mono text-sm"
              spellCheck={false}
              autoComplete="off"
            />
            {showLoading && (
              <Loader2 className="w-4 h-4 text-slate-400 dark:text-slate-500 animate-spin" />
            )}
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-slate-400 dark:text-slate-500 hover:text-slate-700 dark:hover:text-slate-200 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Live Monitor commands section */}
        {isLiveMonitor && filteredLiveCommands.length > 0 && (
          <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
            <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider flex items-center gap-2">
              <span className="inline-block h-2 w-2 rounded-full bg-green-500" />
              Live Monitor
            </p>
            {filteredLiveCommands.map((item, i) => {
              const Icon = item.icon
              const itemIndex = i
              return (
                <button
                  key={item.id}
                  onClick={() => executeLiveCommand(item)}
                  onMouseEnter={() => setSelectedIndex(itemIndex)}
                  className={cn(
                    'w-full flex items-center gap-3 px-4 py-2 text-sm transition-colors',
                    selectedIndex === itemIndex
                      ? 'bg-emerald-50 dark:bg-emerald-500/[0.08] text-slate-900 dark:text-slate-100'
                      : 'text-slate-500 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-white/[0.04] hover:text-slate-900 dark:hover:text-slate-100'
                  )}
                >
                  {Icon ? (
                    <Icon className="w-4 h-4 text-slate-400 dark:text-slate-500" />
                  ) : (
                    <div className="w-4 h-4" />
                  )}
                  <span className="flex-1 truncate text-left">{item.label}</span>
                  {item.description && (
                    <span className="text-xs text-slate-400 dark:text-slate-500 truncate max-w-[150px]">{item.description}</span>
                  )}
                  {item.shortcut && (
                    <span className="text-[10px] font-mono text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-white/[0.06] px-1.5 py-0.5 rounded border border-slate-200 dark:border-white/[0.08]">
                      {item.shortcut}
                    </span>
                  )}
                </button>
              )
            })}
          </div>
        )}

        {/* Global section header (when Live Monitor context is active) */}
        {isLiveMonitor && (suggestions.length > 0 || hasLiveResults) && (
          <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider border-b border-slate-200/80 dark:border-white/[0.06]">
            Global
          </p>
        )}

        {/* Suggestions (project autocomplete + recent searches) */}
        {suggestions.length > 0 && (
          <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
            {suggestions.map((suggestion, i) => {
              const Icon = getIcon(suggestion.type)
              const itemIndex = suggestionsStartIndex + i
              return (
                <button
                  key={`${suggestion.type}-${suggestion.label}-${i}`}
                  onClick={() => handleSelect(suggestion.query)}
                  onMouseEnter={() => setSelectedIndex(itemIndex)}
                  className={cn(
                    'w-full flex items-center gap-3 px-4 py-2 text-sm transition-colors',
                    selectedIndex === itemIndex
                      ? 'bg-emerald-50 dark:bg-emerald-500/[0.08] text-slate-900 dark:text-slate-100'
                      : 'text-slate-500 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-white/[0.04] hover:text-slate-900 dark:hover:text-slate-100'
                  )}
                >
                  <Icon className="w-4 h-4 text-slate-400 dark:text-slate-500" />
                  <span className="flex-1 truncate font-mono">{suggestion.label}</span>
                  {suggestion.count !== undefined && (
                    <span className="text-xs text-slate-400 dark:text-slate-500 tabular-nums">{suggestion.count}</span>
                  )}
                </button>
              )
            })}
          </div>
        )}

        {/* Live search results from Tantivy */}
        {hasLiveResults && (
          <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
            <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
              {searchResults.totalSessions} {searchResults.totalSessions === 1 ? 'session' : 'sessions'}, {searchResults.totalMatches} {searchResults.totalMatches === 1 ? 'match' : 'matches'}
              <span className="ml-2 normal-case tracking-normal">({searchResults.elapsedMs}ms)</span>
            </p>
            <div className="px-3 py-1 space-y-1">
              {searchResults.sessions.map((hit, i) => {
                const itemIndex = searchResultsStartIndex + i
                return (
                  <SearchResultCard
                    key={hit.sessionId}
                    hit={hit}
                    isSelected={selectedIndex === itemIndex}
                    onSelect={() => handleSelectSearchResult(hit.sessionId)}
                  />
                )
              })}
            </div>
            {searchResults.totalSessions > searchResults.sessions.length && (
              <button
                onClick={() => handleSelect(query.trim())}
                className="w-full px-4 py-2 text-xs text-emerald-600 dark:text-emerald-400 hover:text-emerald-500 dark:hover:text-emerald-300 transition-colors text-center"
              >
                View all {searchResults.totalSessions} results
              </button>
            )}
          </div>
        )}

        {/* No results message */}
        {query.trim().length > 0 && !isSearching && !isDebouncing && searchResults && searchResults.sessions.length === 0 && filteredLiveCommands.length === 0 && suggestions.length === 0 && (
          <div className="py-4 border-b border-slate-200/80 dark:border-white/[0.06] text-center">
            <p className="text-sm text-slate-400 dark:text-slate-500">No results found.</p>
          </div>
        )}

        {/* Filter hints */}
        <div className="px-4 py-3">
          <p className="text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider mb-2">
            Filters
          </p>
          <div className="flex flex-wrap gap-2">
            {['project:', 'path:', 'skill:', 'after:', 'before:', '"phrase"'].map(filter => (
              <button
                key={filter}
                onClick={() => insertFilter(filter)}
                className="px-2 py-1 text-xs font-mono text-emerald-700 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-500/10 hover:bg-emerald-100 dark:hover:bg-emerald-500/[0.15] rounded-md border border-emerald-200/80 dark:border-emerald-500/20 transition-colors"
              >
                {filter}
              </button>
            ))}
          </div>
        </div>

        {/* Keyboard hints */}
        <div className="px-4 py-2 border-t border-slate-200/80 dark:border-white/[0.06] flex items-center gap-4 text-xs text-slate-400 dark:text-slate-500">
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-slate-100 dark:bg-white/[0.06] rounded border border-slate-200 dark:border-white/[0.08]">↑↓</kbd>
            Navigate
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-slate-100 dark:bg-white/[0.06] rounded border border-slate-200 dark:border-white/[0.08]">Enter</kbd>
            Search
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-slate-100 dark:bg-white/[0.06] rounded border border-slate-200 dark:border-white/[0.08]">Esc</kbd>
            Close
          </span>
        </div>
      </div>
    </div>
  )
}
