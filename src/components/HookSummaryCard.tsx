import { useState } from 'react'
import { GitBranch, ChevronRight, ChevronDown } from 'lucide-react'

interface HookSummaryCardProps {
  hookCount: number
  hookInfos: string[]
  hookErrors?: string[]
  durationMs?: number
  preventedContinuation?: boolean
  verboseMode?: boolean
}

export function HookSummaryCard({
  hookCount,
  hookInfos,
  hookErrors,
  durationMs,
  preventedContinuation,
  verboseMode,
}: HookSummaryCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  const errorCount = hookErrors?.length ?? 0
  const isEmpty = hookInfos.length === 0

  const summaryParts: string[] = []
  if (isEmpty) {
    summaryParts.push('No hooks')
  } else {
    summaryParts.push(`${hookCount} hooks executed`)
    if (errorCount > 0) {
      summaryParts.push(`(${errorCount} error${errorCount > 1 ? 's' : ''})`)
    }
  }
  if (durationMs !== undefined) {
    summaryParts.push(`in ${durationMs}ms`)
  }

  return (
    <div className="py-0.5 border-l-2 border-l-amber-400 pl-1 my-1">
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-expanded={expanded}
      >
        <GitBranch className="w-3 h-3 text-amber-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {summaryParts.join(' ')}
        </span>
        {preventedContinuation && (
          <span className="text-[9px] font-mono text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20 px-1 py-0.5 rounded flex-shrink-0">
            prevented
          </span>
        )}
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded hook list */}
      {expanded && (
        <div className="ml-4 mt-0.5 space-y-0.5">
          {hookInfos.length > 0 && (
            <ul className="space-y-0.5">
              {hookInfos.map((hook, i) => (
                <li key={i} className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
                  {hook}
                </li>
              ))}
            </ul>
          )}
          {hookErrors && hookErrors.length > 0 && (
            <ul className="space-y-0.5">
              {hookErrors.map((err, i) => (
                <li key={i} className="text-[10px] font-mono text-red-500 dark:text-red-400">
                  {err}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  )
}
