import { useMemo, useEffect } from 'react'
import { useSearchParams, useOutletContext, Link, useNavigate } from 'react-router-dom'
import { X } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { parseQuery, filterSessions } from '../lib/search'
import { SessionCard } from './SessionCard'
import { useAppStore } from '../store/app-store'

interface OutletContext {
  projects: ProjectInfo[]
}

export function SearchResults() {
  const [searchParams] = useSearchParams()
  const { projects } = useOutletContext<OutletContext>()
  const navigate = useNavigate()
  const { addRecentSearch } = useAppStore()

  const query = searchParams.get('q') || ''

  // Add to recent searches when query changes
  useEffect(() => {
    if (query) {
      addRecentSearch(query)
    }
  }, [query, addRecentSearch])

  const results = useMemo(() => {
    if (!query) return []
    const allSessions = projects.flatMap(p => p.sessions)
    const parsed = parseQuery(query)
    return filterSessions(allSessions, projects, parsed)
  }, [projects, query])

  const handleClearSearch = () => {
    navigate('/')
  }

  // Find project for each session (for linking)
  const getSessionProject = (sessionId: string) => {
    return projects.find(p => p.sessions.some(s => s.id === sessionId))
  }

  return (
    <div className="p-6 max-w-3xl mx-auto">
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
          {results.map((session) => {
            const project = getSessionProject(session.id)
            return (
              <Link
                key={session.id}
                to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
              >
                <SessionCard
                  session={session}
                  isSelected={false}
                  isActive={false}
                  onClick={() => {}}
                />
              </Link>
            )
          })}
        </div>
      ) : (
        <div className="text-center py-12 text-gray-500">
          <p>No sessions match your search.</p>
          <p className="text-sm mt-1">Try different keywords or filters.</p>
        </div>
      )}
    </div>
  )
}
