import { Flag } from 'lucide-react'

interface SessionResultCardProps {
  subtype: string
  durationMs?: number
  durationApiMs?: number
  numTurns?: number
  isError?: boolean
  sessionId?: string
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

export function SessionResultCard({
  subtype,
  durationMs,
  durationApiMs,
  numTurns,
  isError,
  sessionId,
}: SessionResultCardProps) {
  const parts: string[] = [subtype || 'unknown']
  if (durationMs !== undefined) parts.push(formatDuration(durationMs))
  if (numTurns !== undefined) parts.push(`${numTurns} turn${numTurns !== 1 ? 's' : ''}`)

  return (
    <div className="py-0.5 border-l-2 border-l-green-400 pl-1 my-1">
      <div className="flex items-center gap-1.5">
        <Flag className={`w-3 h-3 flex-shrink-0 ${isError ? 'text-red-500' : 'text-green-500'}`} aria-hidden="true" />
        <span className={`text-[10px] font-mono ${isError ? 'text-red-500 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'}`}>
          Session {isError ? 'error' : 'completed'}: {parts.join(' | ')}
        </span>
        {durationApiMs !== undefined && durationMs !== undefined && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600" title={`API: ${formatDuration(durationApiMs)} of ${formatDuration(durationMs)} total`}>
            (API {formatDuration(durationApiMs)})
          </span>
        )}
      </div>
      {sessionId && (
        <span className="ml-[18px] text-[9px] font-mono text-gray-400 dark:text-gray-600 select-all">
          {sessionId}
        </span>
      )}
    </div>
  )
}
