import { ChevronDown, ChevronRight, FileText } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { GrepResponse, GrepSessionHit } from '../types/generated'

interface GrepResultsProps {
  data: GrepResponse
}

export function GrepResults({ data }: GrepResultsProps) {
  const navigate = useNavigate()

  if (data.results.length === 0) {
    return (
      <div className="p-4 text-center text-slate-500 dark:text-slate-400">
        No regex matches found
      </div>
    )
  }

  return (
    <div className="py-2">
      <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
        {data.totalSessions} {data.totalSessions === 1 ? 'session' : 'sessions'},{' '}
        {data.totalMatches} {data.totalMatches === 1 ? 'match' : 'matches'}
        <span className="ml-2 normal-case tracking-normal">({data.elapsedMs.toFixed(1)}ms)</span>
        {data.truncated && <span className="ml-2 text-amber-500">(truncated)</span>}
      </p>
      <p className="px-4 py-1 text-xs text-slate-400 dark:text-slate-500">Showing regex matches</p>
      <div className="px-3 py-1 space-y-1">
        {data.results.map((hit) => (
          <GrepSessionCard
            key={hit.sessionId}
            hit={hit}
            onNavigate={(sessionId) => navigate(`/sessions/${sessionId}`)}
          />
        ))}
      </div>
    </div>
  )
}

function GrepSessionCard({
  hit,
  onNavigate,
}: {
  hit: GrepSessionHit
  onNavigate: (sessionId: string) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const previewMatches = useMemo(() => hit.matches.slice(0, 3), [hit.matches])
  const hasMore = hit.matches.length > 3

  return (
    <div className="rounded-lg border border-slate-200/80 dark:border-white/[0.06] bg-white dark:bg-white/[0.02] overflow-hidden">
      <button
        type="button"
        onClick={() => onNavigate(hit.sessionId)}
        className="w-full px-3 py-2 flex items-center gap-2 text-left hover:bg-slate-50 dark:hover:bg-white/[0.03] transition-colors"
      >
        <FileText className="w-4 h-4 text-slate-400 flex-shrink-0" />
        <span className="text-sm font-medium text-slate-700 dark:text-slate-200 truncate">
          {hit.project}
        </span>
        <span className="text-xs text-slate-400 dark:text-slate-500 ml-auto flex-shrink-0">
          {hit.matches.length} {hit.matches.length === 1 ? 'match' : 'matches'}
        </span>
      </button>

      <div className="border-t border-slate-100 dark:border-white/[0.04]">
        {(expanded ? hit.matches : previewMatches).map((match, i) => (
          <div
            key={`${match.lineNumber}-${i}`}
            className="px-3 py-1 text-xs font-mono text-slate-600 dark:text-slate-300 border-b border-slate-50 dark:border-white/[0.02] last:border-b-0 hover:bg-slate-50/50 dark:hover:bg-white/[0.02]"
          >
            <span className="text-slate-400 dark:text-slate-500 mr-2 select-none">
              {match.lineNumber}:
            </span>
            <HighlightedLine
              content={match.content}
              start={match.matchStart}
              end={match.matchEnd}
            />
          </div>
        ))}

        {hasMore && (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="w-full px-3 py-1.5 text-xs text-emerald-600 dark:text-emerald-400 hover:bg-slate-50 dark:hover:bg-white/[0.03] flex items-center gap-1 transition-colors"
          >
            {expanded ? (
              <>
                <ChevronDown className="w-3 h-3" /> Show fewer
              </>
            ) : (
              <>
                <ChevronRight className="w-3 h-3" /> Show {hit.matches.length - 3} more
              </>
            )}
          </button>
        )}
      </div>
    </div>
  )
}

function HighlightedLine({ content, start, end }: { content: string; start: number; end: number }) {
  if (start === end || start >= content.length) {
    return <span>{content}</span>
  }
  const safeEnd = Math.min(end, content.length)
  return (
    <span>
      {content.slice(0, start)}
      <mark className="bg-amber-200 dark:bg-amber-900/50 text-amber-900 dark:text-amber-200 rounded-sm px-0.5">
        {content.slice(start, safeEnd)}
      </mark>
      {content.slice(safeEnd)}
    </span>
  )
}
