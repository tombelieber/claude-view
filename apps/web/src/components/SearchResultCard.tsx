import { useState } from 'react'
import { Link } from 'react-router-dom'
import type { SessionHit, MatchHit } from '../types/generated'

/**
 * Render snippet HTML from Tantivy search results.
 * The snippet contains only <mark> tags for highlighting matched terms.
 * These come from our own backend (Tantivy highlight), not from user input.
 */
function SnippetHighlight({ html }: { html: string }) {
  // Only allow <mark> tags from Tantivy — strip anything else as a safety measure
  const sanitized = html.replace(/<(?!\/?mark\b)[^>]*>/gi, '')
  return <span dangerouslySetInnerHTML={{ __html: sanitized }} />
}

function RoleIcon({ role }: { role: string }) {
  switch (role) {
    case 'user': return <span className="text-blue-500 dark:text-blue-400" title="User">U</span>
    case 'assistant': return <span className="text-green-500 dark:text-green-400" title="Assistant">A</span>
    case 'tool': return <span className="text-orange-500 dark:text-orange-400" title="Tool">T</span>
    default: return null
  }
}

function formatDate(ts: number): string {
  if (ts <= 0) return '--'
  return new Date(ts * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

interface SearchResultCardProps {
  hit: SessionHit
  isSelected?: boolean
  onSelect?: () => void
}

export function SearchResultCard({ hit, isSelected, onSelect }: SearchResultCardProps) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className={`p-3 rounded-lg border transition-colors cursor-pointer ${
        isSelected
          ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
          : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
      }`}
      onClick={onSelect}
    >
      {/* Header row */}
      <div className="flex items-center justify-between text-sm">
        <div className="flex items-center gap-2">
          <span className="font-medium text-gray-900 dark:text-gray-100">{hit.project}</span>
          {hit.branch && (
            <span className="text-gray-500 dark:text-gray-400">&middot; {hit.branch}</span>
          )}
          <span className="text-gray-400 dark:text-gray-500">&middot; {hit.matchCount} {hit.matchCount === 1 ? 'match' : 'matches'}</span>
        </div>
        <span className="text-gray-400 dark:text-gray-500 text-xs">{formatDate(hit.modifiedAt)}</span>
      </div>

      {/* Top match snippet */}
      <div className="mt-1.5 text-sm text-gray-700 dark:text-gray-300 line-clamp-2">
        <SnippetHighlight html={hit.topMatch.snippet} />
      </div>

      {/* Expand/collapse matches */}
      {hit.matchCount > 1 && (
        <div className="mt-2">
          <button
            onClick={(e) => { e.stopPropagation(); setExpanded(!expanded) }}
            className="text-xs text-blue-600 dark:text-blue-400 hover:underline"
          >
            {expanded ? 'Hide matches' : `Show all ${hit.matchCount} matches`}
          </button>

          {expanded && (
            <div className="mt-2 space-y-2 border-l-2 border-gray-200 dark:border-gray-700 pl-3">
              {hit.matches.map((match, i) => (
                <MatchRow key={i} match={match} sessionId={hit.sessionId} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function MatchRow({ match, sessionId }: { match: MatchHit; sessionId: string }) {
  return (
    <Link
      to={`/sessions/${sessionId}?turn=${match.turnNumber}`}
      className="block text-sm hover:bg-gray-50 dark:hover:bg-gray-800 rounded p-1.5 -ml-1.5"
      onClick={(e) => e.stopPropagation()}
    >
      <div className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400">
        <RoleIcon role={match.role} />
        <span>{match.role === 'user' ? 'User' : match.role === 'assistant' ? 'Assistant' : 'Tool'}</span>
        <span>&middot; turn {match.turnNumber}</span>
      </div>
      <div className="mt-0.5 text-gray-700 dark:text-gray-300 line-clamp-2">
        <SnippetHighlight html={match.snippet} />
      </div>
    </Link>
  )
}
