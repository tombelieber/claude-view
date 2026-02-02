import { useState } from 'react'
import { Terminal, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface BashProgressCardProps {
  command: string
  output?: string
  exitCode?: number
  duration?: number
}

export function BashProgressCard({
  command,
  output,
  exitCode,
  duration,
}: BashProgressCardProps) {
  const [expanded, setExpanded] = useState(false)

  const isSuccess = exitCode === 0
  const hasExitCode = exitCode !== undefined
  const borderColor = !hasExitCode
    ? 'border-l-gray-400'
    : isSuccess
      ? 'border-l-green-500'
      : 'border-l-red-500'
  const iconColor = !hasExitCode
    ? 'text-gray-500'
    : isSuccess
      ? 'text-green-600'
      : 'text-red-600'

  const statusParts: string[] = []
  if (hasExitCode) statusParts.push(`exit ${exitCode}`)
  if (duration !== undefined) statusParts.push(`${duration}ms`)
  const statusText = statusParts.length > 0 ? ` → ${statusParts.join(', ')}` : ''

  const displayOutput = output === '' ? 'No output' : output

  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 bg-white dark:bg-gray-900 my-2 overflow-hidden',
        borderColor
      )}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
        aria-label="Bash command"
        aria-expanded={expanded}
      >
        <Terminal className={cn('w-4 h-4 flex-shrink-0', iconColor)} aria-hidden="true" />
        <span className="text-sm font-mono text-gray-800 dark:text-gray-200 truncate flex-1">
          $ {command}
          {statusText && (
            <span className="text-gray-500">{statusText}</span>
          )}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-gray-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-gray-400" />
        )}
      </button>

      {expanded && (
        <div className="px-3 py-2 border-t border-gray-100 dark:border-gray-700 bg-gray-900">
          <pre className="text-xs text-green-300 font-mono whitespace-pre-wrap break-all">
            {displayOutput ?? 'No output'}
          </pre>
        </div>
      )}
    </div>
  )
}
