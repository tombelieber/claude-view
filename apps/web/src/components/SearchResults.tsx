import { useEffect } from 'react'
import { useSearchParams, useNavigate } from 'react-router-dom'
import { X, Loader2, AlertCircle } from 'lucide-react'
import { useSearch } from '../hooks/use-search'
import { SearchResultCard } from './SearchResultCard'
import { useAppStore } from '../store/app-store'

export function SearchResults() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { addRecentSearch } = useAppStore()

  const query = searchParams.get('q') || ''
  const scope = searchParams.get('scope') || undefined

  // Add to recent searches when query changes
  useEffect(() => {
    if (query) {
      addRecentSearch(query)
    }
  }, [query, addRecentSearch])

  const { data: searchResults, isLoading, error, isDebouncing } = useSearch(query, {
    scope,
    limit: 50,
  })

  const handleClearSearch = () => {
    navigate('/')
  }

  if (isLoading || isDebouncing) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex items-center gap-3 text-gray-500">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Searching...</span>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex items-center gap-3 text-red-500 dark:text-red-400">
          <AlertCircle className="w-5 h-5" />
          <span>Search failed. Please try again.</span>
        </div>
      </div>
    )
  }

  const sessions = searchResults?.sessions ?? []
  const totalSessions = searchResults?.totalSessions ?? 0
  const totalMatches = searchResults?.totalMatches ?? 0
  const elapsedMs = searchResults?.elapsedMs ?? 0

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">Search Results</h1>
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {totalSessions} {totalSessions === 1 ? 'session' : 'sessions'}, {totalMatches} {totalMatches === 1 ? 'match' : 'matches'} for &ldquo;<span className="font-mono">{query}</span>&rdquo;
              <span className="ml-1 text-gray-400 dark:text-gray-500">({elapsedMs}ms)</span>
            </p>
          </div>
          <button
            onClick={handleClearSearch}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 dark:text-gray-400 dark:hover:text-gray-100 bg-gray-200 hover:bg-gray-300 dark:bg-gray-800 dark:hover:bg-gray-700 rounded-lg transition-colors"
          >
            <X className="w-4 h-4" />
            Clear
          </button>
        </div>

        {sessions.length > 0 ? (
          <div className="space-y-3">
            {sessions.map((hit) => (
              <SearchResultCard
                key={hit.sessionId}
                hit={hit}
                onSelect={() => navigate(`/sessions/${hit.sessionId}`)}
              />
            ))}
          </div>
        ) : (
          <div className="text-center py-12 text-gray-500 dark:text-gray-400">
            <p>No sessions match your search.</p>
            <p className="text-sm mt-1">Try different keywords or filters.</p>
          </div>
        )}
      </div>
    </div>
  )
}
