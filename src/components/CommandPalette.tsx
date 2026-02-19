import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { Search, X, FolderOpen, Clock, Loader2 } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useAppStore } from '../store/app-store'
import { useSearch } from '../hooks/use-search'
import { SearchResultCard } from './SearchResultCard'
import { cn } from '../lib/utils'

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  projects: ProjectSummary[]
}

type SuggestionType = 'project' | 'recent'

interface Suggestion {
  type: SuggestionType
  label: string
  query: string
  count?: number
}

export function CommandPalette({ isOpen, onClose, projects }: CommandPaletteProps) {
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

  // Total navigable items: suggestions + search result sessions
  const searchSessionCount = searchResults?.sessions.length ?? 0
  const totalItems = suggestions.length + searchSessionCount

  const handleSelect = useCallback((searchQuery: string) => {
    addRecentSearch(searchQuery)
    onClose()
    navigate(`/search?q=${encodeURIComponent(searchQuery)}`)
  }, [addRecentSearch, onClose, navigate])

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
        if (selectedIndex < suggestions.length && suggestions[selectedIndex]) {
          handleSelect(suggestions[selectedIndex].query)
        } else if (selectedIndex >= suggestions.length && searchResults) {
          // Navigate to the selected search result session
          const sessionIdx = selectedIndex - suggestions.length
          const session = searchResults.sessions[sessionIdx]
          if (session) {
            onClose()
            navigate(`/sessions/${session.sessionId}`)
          }
        } else if (query.trim()) {
          handleSelect(query.trim())
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose, suggestions, selectedIndex, query, totalItems, searchResults, handleSelect, navigate])

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
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-xl bg-[#111113] rounded-xl shadow-2xl border border-[#2a2a2e] overflow-hidden">
        {/* Search input */}
        <form onSubmit={handleSubmit}>
          <div className="flex items-center gap-3 px-4 py-3 border-b border-[#2a2a2e]">
            <Search className="w-5 h-5 text-[#6e6e76]" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value)
                setSelectedIndex(0)
              }}
              placeholder="Search sessions..."
              className="flex-1 bg-transparent text-[#ececef] placeholder-[#6e6e76] outline-none font-mono text-sm"
              spellCheck={false}
              autoComplete="off"
            />
            {showLoading && (
              <Loader2 className="w-4 h-4 text-[#6e6e76] animate-spin" />
            )}
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-[#6e6e76] hover:text-[#ececef] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Suggestions (project autocomplete + recent searches) */}
        {suggestions.length > 0 && (
          <div className="py-2 border-b border-[#2a2a2e]">
            {suggestions.map((suggestion, i) => {
              const Icon = getIcon(suggestion.type)
              return (
                <button
                  key={`${suggestion.type}-${suggestion.label}-${i}`}
                  onClick={() => handleSelect(suggestion.query)}
                  className={cn(
                    'w-full flex items-center gap-3 px-4 py-2 text-sm transition-colors',
                    selectedIndex === i
                      ? 'bg-[#1c1c1f] text-[#ececef]'
                      : 'text-[#9b9ba0] hover:bg-[#1c1c1f] hover:text-[#ececef]'
                  )}
                >
                  <Icon className="w-4 h-4 text-[#6e6e76]" />
                  <span className="flex-1 truncate font-mono">{suggestion.label}</span>
                  {suggestion.count !== undefined && (
                    <span className="text-xs text-[#6e6e76] tabular-nums">{suggestion.count}</span>
                  )}
                </button>
              )
            })}
          </div>
        )}

        {/* Live search results from Tantivy */}
        {hasLiveResults && (
          <div className="py-2 border-b border-[#2a2a2e]">
            <p className="px-4 py-1 text-xs font-medium text-[#6e6e76] uppercase tracking-wider">
              {searchResults.totalSessions} {searchResults.totalSessions === 1 ? 'session' : 'sessions'}, {searchResults.totalMatches} {searchResults.totalMatches === 1 ? 'match' : 'matches'}
              <span className="ml-2 normal-case tracking-normal">({searchResults.elapsedMs}ms)</span>
            </p>
            <div className="px-3 py-1 space-y-1">
              {searchResults.sessions.map((hit, i) => {
                const itemIndex = suggestions.length + i
                return (
                  <SearchResultCard
                    key={hit.sessionId}
                    hit={hit}
                    isSelected={selectedIndex === itemIndex}
                    onSelect={() => {
                      onClose()
                      navigate(`/sessions/${hit.sessionId}`)
                    }}
                  />
                )
              })}
            </div>
            {searchResults.totalSessions > searchResults.sessions.length && (
              <button
                onClick={() => handleSelect(query.trim())}
                className="w-full px-4 py-2 text-xs text-blue-400 hover:text-blue-300 transition-colors text-center"
              >
                View all {searchResults.totalSessions} results
              </button>
            )}
          </div>
        )}

        {/* No results message */}
        {query.trim().length > 0 && !isSearching && !isDebouncing && searchResults && searchResults.sessions.length === 0 && (
          <div className="py-4 border-b border-[#2a2a2e] text-center">
            <p className="text-sm text-[#6e6e76]">No sessions match your search.</p>
          </div>
        )}

        {/* Filter hints */}
        <div className="px-4 py-3">
          <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
            Filters
          </p>
          <div className="flex flex-wrap gap-2">
            {['project:', 'path:', 'skill:', 'after:', 'before:', '"phrase"'].map(filter => (
              <button
                key={filter}
                onClick={() => insertFilter(filter)}
                className="px-2 py-1 text-xs font-mono text-[#7c9885] bg-[#1c1c1f] hover:bg-[#252525] rounded border border-[#2a2a2e] transition-colors"
              >
                {filter}
              </button>
            ))}
          </div>
        </div>

        {/* Keyboard hints */}
        <div className="px-4 py-2 border-t border-[#2a2a2e] flex items-center gap-4 text-xs text-[#6e6e76]">
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">↑↓</kbd>
            Navigate
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">Enter</kbd>
            Search
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">Esc</kbd>
            Close
          </span>
        </div>
      </div>
    </div>
  )
}
