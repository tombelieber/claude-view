import { useMemo, useEffect } from 'react'
import { useSearchParams, Link, useNavigate } from 'react-router-dom'
import { X, Loader2 } from 'lucide-react'
import { useProjectSummaries, useAllSessions } from '../hooks/use-projects'
import { parseQuery, filterSessions } from '../lib/search'
import { SessionCard } from './SessionCard'
import { useAppStore } from '../store/app-store'

export function SearchResults() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { addRecentSearch } = useAppStore()

  const { data: summaries } = useProjectSummaries()
  const projectIds = useMemo(() => (summaries ?? []).map(s => s.name), [summaries])
  const { sessions: allSessions, isLoading } = useAllSessions(projectIds)

  const query = searchParams.get('q') || ''

  // Add to recent searches when query changes
  useEffect(() => {
    if (query) {
      addRecentSearch(query)
    }
  }, [query, addRecentSearch])

  // Build a ProjectInfo-compatible structure for filterSessions
  const projectsForFilter = useMemo(() => {
    if (!summaries) return []
    // Group sessions by project to build a lookup structure
    const sessionsByProject = new Map<string, typeof allSessions>()
    for (const s of allSessions) {
      const list = sessionsByProject.get(s.project) || []
      list.push(s)
      sessionsByProject.set(s.project, list)
    }
    return summaries.map(p => ({
      name: p.name,
      displayName: p.displayName,
      path: p.path,
      sessions: sessionsByProject.get(p.name) ?? [],
    }))
  }, [summaries, allSessions])

  const results = useMemo(() => {
    if (!query || allSessions.length === 0) return []
    const parsed = parseQuery(query)
    return filterSessions(allSessions, projectsForFilter, parsed)
  }, [allSessions, projectsForFilter, query])

  const handleClearSearch = () => {
    navigate('/')
  }

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="flex items-center gap-3 text-gray-500">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Searching...</span>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-semibold text-gray-900">Search Results</h1>
          <p className="text-sm text-gray-500 mt-1">
            {results.length} sessions matching "<span className="font-mono">{query}</span>"
          </p>
        </div>
        <button
          onClick={handleClearSearch}
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors"
        >
          <X className="w-4 h-4" />
          Clear
        </button>
      </div>

      {results.length > 0 ? (
        <div className="space-y-3">
          {results.map((session) => (
            <Link
              key={session.id}
              to={`/session/${encodeURIComponent(session.id)}`}
              className="block focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-lg"
            >
              <SessionCard
                session={session}
                isSelected={false}
              />
            </Link>
          ))}
        </div>
      ) : (
        <div className="text-center py-12 text-gray-500">
          <p>No sessions match your search.</p>
          <p className="text-sm mt-1">Try different keywords or filters.</p>
        </div>
      )}
      </div>
    </div>
  )
}
