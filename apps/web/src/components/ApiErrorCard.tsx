import { useState } from 'react'
import { AlertTriangle, ChevronRight, ChevronDown } from 'lucide-react'

interface ApiErrorCardProps {
  error: Record<string, unknown>
  retryAttempt: number
  maxRetries: number
  retryInMs?: number
  verboseMode?: boolean
}

export function ApiErrorCard({ error, retryAttempt, maxRetries, retryInMs, verboseMode }: ApiErrorCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  const errorCode = error.code ?? null
  const errorMessage = error.message ?? null
  const hasErrorInfo = errorCode !== null || errorMessage !== null
  const retriesExhausted = retryAttempt > maxRetries

  const summaryText = hasErrorInfo
    ? `${errorCode ? `${errorCode} — ` : ''}${errorMessage || 'Error'}`
    : 'Unknown error'

  return (
    <div className="py-0.5 border-l-2 border-l-red-400 pl-1 my-1">
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-expanded={expanded}
      >
        <AlertTriangle className="w-3 h-3 text-red-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-red-600 dark:text-red-400 truncate flex-1">
          {summaryText}
        </span>
        {retriesExhausted && (
          <span className="text-[9px] font-mono text-red-600 dark:text-red-400 bg-red-500/10 dark:bg-red-500/20 px-1 py-0.5 rounded flex-shrink-0">
            Retries exhausted
          </span>
        )}
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded error details */}
      {expanded && (
        <div className="ml-4 mt-0.5 space-y-0.5">
          {errorCode !== null && (
            <div className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
              Code: {String(errorCode)}
            </div>
          )}
          {errorMessage !== null && (
            <div className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
              Message: {String(errorMessage)}
            </div>
          )}
          <div className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
            Retry: {retryAttempt}/{maxRetries}
          </div>
          {retryInMs !== undefined && (
            <div className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
              Backoff: {retryInMs}ms
            </div>
          )}
        </div>
      )}
    </div>
  )
}
