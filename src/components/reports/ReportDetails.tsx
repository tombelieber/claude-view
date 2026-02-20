import { useState, useMemo } from 'react'
import { ChevronRight } from 'lucide-react'

interface ContextDigest {
  report_type: string
  date_range: string
  summary_line: string
  total_input_tokens?: number
  total_output_tokens?: number
  projects: {
    name: string
    session_count: number
    commit_count: number
    total_duration_secs: number
    branches: {
      name: string
      sessions: { first_prompt: string; category: string | null; duration_secs: number }[]
    }[]
  }[]
  top_tools: string[]
  top_skills: string[]
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

interface ReportDetailsProps {
  contextDigestJson: string | null
  totalCostCents: number
}

export function ReportDetails({ contextDigestJson, totalCostCents }: ReportDetailsProps) {
  const [expanded, setExpanded] = useState(false)

  const digest = useMemo(() => {
    if (!contextDigestJson) return null
    try {
      return JSON.parse(contextDigestJson) as ContextDigest
    } catch {
      return null
    }
  }, [contextDigestJson])

  if (!digest) return null

  return (
    <div className="mt-4 border-t border-gray-100 dark:border-gray-800 pt-3">
      {/* Toggle button */}
      <button
        type="button"
        onClick={() => setExpanded(e => !e)}
        className="flex items-center gap-1.5 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors duration-200 cursor-pointer"
        aria-expanded={expanded}
      >
        <ChevronRight
          className={`w-3.5 h-3.5 transition-transform duration-200 ${expanded ? 'rotate-90' : ''}`}
        />
        <span>Details</span>
        {!expanded && (
          <span className="ml-1 text-gray-300 dark:text-gray-600">
            &middot; {totalCostCents > 0 ? `$${(totalCostCents / 100).toFixed(2)}` : ''}
            {digest.total_input_tokens ? ` · ${formatTokens(digest.total_input_tokens + (digest.total_output_tokens ?? 0))} tokens` : ''}
          </span>
        )}
      </button>

      {/* Expandable panel */}
      <div
        className={`grid transition-[grid-template-rows] duration-200 ease-out motion-reduce:transition-none ${expanded ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
      >
        <div className="overflow-hidden">
          <div className="pt-3 space-y-2.5 text-xs">

            {/* Row 1: Cost + Tokens */}
            <div className="flex flex-wrap gap-x-3 gap-y-1 text-gray-600 dark:text-gray-400">
              {totalCostCents > 0 && (
                <span>Cost: <span className="text-gray-900 dark:text-gray-200 font-medium">${(totalCostCents / 100).toFixed(2)}</span></span>
              )}
              {digest.total_input_tokens != null && digest.total_input_tokens > 0 && (
                <span>Tokens: <span className="text-gray-900 dark:text-gray-200 font-medium">{formatTokens(digest.total_input_tokens)}</span> in / <span className="text-gray-900 dark:text-gray-200 font-medium">{formatTokens(digest.total_output_tokens ?? 0)}</span> out</span>
              )}
            </div>

            {/* Row 2: Top tools */}
            {digest.top_tools.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-gray-500 dark:text-gray-500 shrink-0">Tools:</span>
                {digest.top_tools.map(tool => (
                  <span
                    key={tool}
                    className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 font-mono"
                  >
                    {tool}
                  </span>
                ))}
              </div>
            )}

            {/* Row 3: Top skills */}
            {digest.top_skills.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-gray-500 dark:text-gray-500 shrink-0">Skills:</span>
                {digest.top_skills.map(skill => (
                  <span
                    key={skill}
                    className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 font-mono"
                  >
                    /{skill}
                  </span>
                ))}
              </div>
            )}

            {/* Per-project breakdown */}
            {digest.projects.length > 0 && (
              <div className="pt-1 space-y-2">
                {digest.projects.map(proj => (
                  <div key={proj.name}>
                    <div className="flex items-baseline gap-1.5">
                      <span className="text-gray-900 dark:text-gray-200 font-medium truncate max-w-[200px]">{proj.name}</span>
                      <span className="text-gray-400 dark:text-gray-500">&mdash;</span>
                      <span className="text-gray-500 dark:text-gray-400">
                        {proj.session_count} sessions &middot; {formatDuration(proj.total_duration_secs)}
                        {proj.commit_count > 0 && ` · ${proj.commit_count} commits`}
                      </span>
                    </div>
                    {proj.branches.length > 0 && (
                      <div className="ml-3 mt-0.5 text-gray-400 dark:text-gray-500">
                        {proj.branches.map(b => (
                          <span key={b.name} className="mr-2.5">
                            <span className="text-gray-300 dark:text-gray-600 select-none">&lsaquo; </span>
                            <span className="font-mono">{b.name}</span>
                            <span className="ml-0.5">({b.sessions.length})</span>
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

          </div>
        </div>
      </div>
    </div>
  )
}
