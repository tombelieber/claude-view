import { useState } from 'react'
import { AlertTriangle, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface ApiErrorCardProps {
  error: Record<string, unknown>
  retryAttempt: number
  maxRetries: number
  retryInMs?: number
}

export function ApiErrorCard({ error, retryAttempt, maxRetries, retryInMs }: ApiErrorCardProps) {
  const [expanded, setExpanded] = useState(false)

  const errorCode = error.code ?? null
  const errorMessage = error.message ?? null
  const hasErrorInfo = errorCode !== null || errorMessage !== null
  const retriesExhausted = retryAttempt > maxRetries

  const summaryText = hasErrorInfo
    ? `${errorCode ? `${errorCode} — ` : ''}${errorMessage || 'Error'}`
    : 'Unknown error'

  return (
    <div
      className={cn(
        'border border-gray-200 dark:border-gray-700 border-l-4 border-l-red-400 rounded-lg overflow-hidden bg-white dark:bg-gray-900 my-2'
      )}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-red-50/50 dark:hover:bg-red-950/30 transition-colors focus:outline-none focus:ring-2 focus:ring-red-300 focus:ring-inset"
        aria-expanded={expanded}
      >
        <AlertTriangle className="w-4 h-4 text-red-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-sm font-medium text-red-700 dark:text-red-400 flex-1 truncate">
          {summaryText}
        </span>
        {retriesExhausted && (
          <span className="text-xs font-medium text-red-600 dark:text-red-400 bg-red-100 dark:bg-red-950/40 px-1.5 py-0.5 rounded">
            Retries exhausted
          </span>
        )}
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-red-400" aria-hidden="true" />
        ) : (
          <ChevronRight className="w-4 h-4 text-red-400" aria-hidden="true" />
        )}
      </button>

      {expanded && (
        <div className="px-3 py-2 border-t border-gray-100 dark:border-gray-700 bg-red-50 dark:bg-red-950/20 text-sm space-y-1">
          {errorCode !== null && (
            <div className="text-red-700 dark:text-red-400">
              <span className="font-medium">Code:</span> {String(errorCode)}
            </div>
          )}
          {errorMessage !== null && (
            <div className="text-red-700 dark:text-red-400">
              <span className="font-medium">Message:</span> {String(errorMessage)}
            </div>
          )}
          <div className="text-red-700">
            <span className="font-medium">Retry:</span> {retryAttempt}/{maxRetries}
          </div>
          {retryInMs !== undefined && (
            <div className="text-red-700 dark:text-red-400">
              <span className="font-medium">Backoff:</span> {retryInMs}ms
            </div>
          )}
        </div>
      )}
    </div>
  )
}
