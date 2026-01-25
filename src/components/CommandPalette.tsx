import { useState, useEffect, useCallback, useRef, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { Search, X, Zap, FolderOpen, FileText, Clock } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { parseQuery, filterSessions } from '../lib/search'
import { useAppStore } from '../store/app-store'
import { cn } from '../lib/utils'

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  projects: ProjectInfo[]
}

type SuggestionType = 'project' | 'skill' | 'file' | 'recent' | 'session'

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

  // Reset when opened
  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus()
      setQuery('')
      setSelectedIndex(0)
    }
  }, [isOpen])

  // Generate suggestions based on query
  const suggestions = useMemo((): Suggestion[] => {
    const results: Suggestion[] = []
    const q = query.toLowerCase().trim()

    if (!q) {
      // Show recent searches and quick filters
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
            count: project.sessions.length,
          })
        }
        if (results.length >= 3) break
      }
    }

    // Autocomplete skills
    if (q.startsWith('skill:') || !q.includes(':')) {
      const searchTerm = q.startsWith('skill:') ? q.slice(6) : q
      const skillCounts = new Map<string, number>()
      for (const project of projects) {
        for (const session of project.sessions) {
          for (const skill of session.skillsUsed ?? []) {
            if (skill.toLowerCase().includes(searchTerm)) {
              skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
            }
          }
        }
      }
      const topSkills = Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3)
      for (const [skill, count] of topSkills) {
        results.push({
          type: 'skill',
          label: skill,
          query: `skill:${skill.replace('/', '')}`,
          count,
        })
      }
    }

    // Show matching sessions (preview)
    if (q.length >= 2) {
      const allSessions = projects.flatMap(p => p.sessions)
      const parsed = parseQuery(q)
      const matches = filterSessions(allSessions, projects, parsed).slice(0, 3)
      for (const session of matches) {
        results.push({
          type: 'session',
          label: session.preview.slice(0, 60) + (session.preview.length > 60 ? '...' : ''),
          query: q, // Full search
        })
      }
    }

    return results.slice(0, 8)
  }, [query, projects, recentSearches])

  // Handle keyboard navigation
  useEffect(() => {
    if (!isOpen) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      } else if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIndex(i => Math.min(i + 1, suggestions.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIndex(i => Math.max(i - 1, 0))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        if (suggestions[selectedIndex]) {
          handleSelect(suggestions[selectedIndex].query)
        } else if (query.trim()) {
          handleSelect(query.trim())
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose, suggestions, selectedIndex, query])

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
      case 'skill': return Zap
      case 'file': return FileText
      case 'recent': return Clock
      case 'session': return Search
    }
  }

  if (!isOpen) return null

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
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-[#6e6e76] hover:text-[#ececef] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Suggestions */}
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
