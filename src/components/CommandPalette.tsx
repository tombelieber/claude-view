import { useState, useEffect, useCallback, useRef } from 'react'
import { Search, X } from 'lucide-react'
import { cn } from '../lib/utils'

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  onSearch: (query: string) => void
  recentSearches: string[]
}

export function CommandPalette({
  isOpen,
  onClose,
  onSearch,
  recentSearches
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  // Focus input when opened
  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus()
      setQuery('')
    }
  }, [isOpen])

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Escape to close
      if (e.key === 'Escape' && isOpen) {
        onClose()
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose])

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (query.trim()) {
      onSearch(query.trim())
    }
  }, [query, onSearch])

  const handleRecentClick = useCallback((search: string) => {
    setQuery(search)
    onSearch(search)
  }, [onSearch])

  const insertFilter = useCallback((filter: string) => {
    setQuery(prev => {
      const trimmed = prev.trim()
      return trimmed ? `${trimmed} ${filter}` : filter
    })
    inputRef.current?.focus()
  }, [])

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
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
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search sessions..."
              className="flex-1 bg-transparent text-[#ececef] placeholder-[#6e6e76] outline-none font-mono text-sm"
              spellCheck={false}
              autoComplete="off"
            />
            <kbd className="hidden sm:inline-flex items-center gap-1 px-2 py-0.5 text-xs text-[#6e6e76] bg-[#1c1c1f] rounded border border-[#2a2a2e]">
              Enter
            </kbd>
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-[#6e6e76] hover:text-[#ececef] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Recent searches */}
        {recentSearches.length > 0 && (
          <div className="px-4 py-3 border-b border-[#2a2a2e]">
            <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
              Recent
            </p>
            <div className="space-y-1">
              {recentSearches.slice(0, 3).map((search, i) => (
                <button
                  key={i}
                  onClick={() => handleRecentClick(search)}
                  className="w-full flex items-center gap-2 px-2 py-1.5 text-sm text-[#ececef] hover:bg-[#1c1c1f] rounded transition-colors text-left font-mono"
                >
                  <span className="text-[#6e6e76]">○</span>
                  {search}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Filter hints */}
        <div className="px-4 py-3">
          <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
            Filters
          </p>
          <div className="flex flex-wrap gap-2">
            {['project:', 'path:', 'skill:', 'after:', '"phrase"', '/regex/'].map(filter => (
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
