import { Terminal } from 'lucide-react'
import { cn } from '../lib/utils'
import { CompactCodeBlock } from './live/CompactCodeBlock'

interface BashProgressCardProps {
  command: string
  output?: string
  exitCode?: number
  duration?: number
  blockId?: string
}

export function BashProgressCard({
  command,
  output,
  exitCode,
  duration,
  blockId,
}: BashProgressCardProps) {
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

  const hasOutput = output !== undefined && output !== ''

  return (
    <div
      data-testid="bash-progress-card"
      className={cn(
        'py-0.5 border-l-2 pl-1 my-1',
        borderColor
      )}
    >
      {/* Status line: icon + exit code + duration */}
      <div className="flex items-center gap-1.5 mb-0.5">
        <Terminal className={cn('w-3 h-3 flex-shrink-0', iconColor)} aria-hidden="true" />
        {statusText && (
          <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">{statusText.replace(' → ', '')}</span>
        )}
      </div>

      {/* Command — CompactCodeBlock just like BashRenderer */}
      {command && (
        <CompactCodeBlock code={command} language="bash" blockId={blockId ? `${blockId}-cmd` : undefined} />
      )}

      {/* Output — visible by default, collapses at 12 lines */}
      {hasOutput && (
        <CompactCodeBlock code={output} language="bash" blockId={blockId ? `${blockId}-out` : undefined} />
      )}
      {output === '' && (
        <div className="px-2 py-1.5 text-[11px] text-gray-500 dark:text-gray-400 font-mono">
          No output
        </div>
      )}
    </div>
  )
}
