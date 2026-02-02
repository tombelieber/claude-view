import { useState } from 'react'
import { GitBranch, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface HookProgressCardProps {
  hookEvent: string
  hookName: string
  command: string
  output?: string
}

export function HookProgressCard({
  hookEvent,
  hookName,
  command,
  output,
}: HookProgressCardProps) {
  const [expanded, setExpanded] = useState(false)

  const hasOutput = output !== undefined

  return (
    <div
      className={cn(
        'rounded-lg border border-amber-200 dark:border-amber-800 border-l-4 border-l-amber-400 bg-amber-50 dark:bg-amber-950/30 my-2 overflow-hidden'
      )}
    >
      {hasOutput ? (
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-amber-100 dark:hover:bg-amber-900/30 transition-colors"
          aria-label="Hook event"
          aria-expanded={expanded}
        >
          <GitBranch className="w-4 h-4 text-amber-600 flex-shrink-0" aria-hidden="true" />
          <span className="text-sm text-amber-900 dark:text-amber-200 truncate flex-1">
            Hook: {hookEvent} → {command}
          </span>
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-amber-400" data-testid="hook-expand-icon" />
          ) : (
            <ChevronRight className="w-4 h-4 text-amber-400" data-testid="hook-expand-icon" />
          )}
        </button>
      ) : (
        <div className="flex items-center gap-2 px-3 py-2">
          <GitBranch className="w-4 h-4 text-amber-600 flex-shrink-0" aria-hidden="true" />
          <span className="text-sm text-amber-900 dark:text-amber-200 truncate flex-1">
            Hook: {hookEvent} → {command}
          </span>
        </div>
      )}

      {expanded && hasOutput && (
        <div className="px-3 py-2 border-t border-amber-100 dark:border-amber-800 bg-amber-50/50 dark:bg-amber-950/20">
          <pre className="text-xs text-amber-800 dark:text-amber-300 font-mono whitespace-pre-wrap break-all">
            {output}
          </pre>
        </div>
      )}
    </div>
  )
}
