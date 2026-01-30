import { useState } from 'react'
import { GitBranch, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface HookSummaryCardProps {
  hookCount: number
  hookInfos: string[]
  hookErrors?: string[]
  durationMs?: number
  preventedContinuation?: boolean
}

export function HookSummaryCard({
  hookCount,
  hookInfos,
  hookErrors,
  durationMs,
  preventedContinuation,
}: HookSummaryCardProps) {
  const [expanded, setExpanded] = useState(false)

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
    <div
      className={cn(
        'border border-gray-200 border-l-4 border-l-amber-300 rounded-lg overflow-hidden bg-white my-2'
      )}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-amber-50/50 transition-colors focus:outline-none focus:ring-2 focus:ring-amber-300 focus:ring-inset"
        aria-expanded={expanded}
      >
        <GitBranch className="w-4 h-4 text-amber-600 flex-shrink-0" aria-hidden="true" />
        <span className="text-sm font-medium text-amber-800 flex-1">
          {summaryParts.join(' ')}
        </span>
        {preventedContinuation && (
          <span className="text-xs font-medium text-amber-700 bg-amber-100 px-1.5 py-0.5 rounded">
            Prevented continuation
          </span>
        )}
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-amber-500" aria-hidden="true" />
        ) : (
          <ChevronRight className="w-4 h-4 text-amber-500" aria-hidden="true" />
        )}
      </button>

      {expanded && (
        <div className="px-3 py-2 border-t border-gray-100 bg-amber-50/30 text-sm space-y-2">
          {hookInfos.length > 0 && (
            <ul className="list-disc pl-4 space-y-0.5 text-amber-800">
              {hookInfos.map((hook, i) => (
                <li key={i}>{hook}</li>
              ))}
            </ul>
          )}
          {hookErrors && hookErrors.length > 0 && (
            <div>
              <p className="text-xs text-red-600 font-medium mb-1">Errors:</p>
              <ul className="list-disc pl-4 space-y-0.5 text-red-600">
                {hookErrors.map((err, i) => (
                  <li key={i}>{err}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
