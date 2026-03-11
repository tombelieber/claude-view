import { AlertCircle, Loader2, X } from 'lucide-react'
import { useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useSearch } from '../hooks/use-search'
import { useAppStore } from '../store/app-store'
import { SearchResultCard } from './SearchResultCard'

export function SearchResults() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { addRecentSearch } = useAppStore()

  const query = searchParams.get('q') || ''

  // Add to recent searches when query changes
  useEffect(() => {
    if (query) {
      addRecentSearch(query)
    }
  }, [query, addRecentSearch])

  const {
    data: searchResults,
    isLoading,
    error,
    isDebouncing,
  } = useSearch(query, {
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
  const hasGrepResults = sessions.some((s) => s.engines.includes('grep'))

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
              Search Results
            </h1>
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {sessions.length < totalSessions
                ? `${sessions.length} of ${totalSessions}`
                : `${totalSessions}`}{' '}
              {totalSessions === 1 ? 'session' : 'sessions'}, {totalMatches}{' '}
              {totalMatches === 1 ? 'match' : 'matches'} for &ldquo;
              <span className="font-mono">{query}</span>&rdquo;
              <span className="ml-1 text-gray-400 dark:text-gray-500">({elapsedMs}ms)</span>
            </p>
            {hasGrepResults && (
              <span className="ml-2 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-500/10 px-1.5 py-0.5 rounded">
                Substring matches
              </span>
            )}
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
