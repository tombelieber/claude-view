import type { ToolExecution } from '@claude-view/shared/types/blocks'
import { Check, ChevronDown, ChevronRight, Loader2, Wrench, XCircle } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../../../lib/utils'
import { ContentRenderer } from './ContentRenderer'

interface ToolCardProps {
  execution: ToolExecution
}

function StatusIcon({ status }: { status: ToolExecution['status'] }) {
  switch (status) {
    case 'running':
      return <Loader2 className="h-3 w-3 animate-spin text-blue-500" data-testid="status-running" />
    case 'complete':
      return <Check className="h-3 w-3 text-green-500" data-testid="status-complete" />
    case 'error':
      return <XCircle className="h-3 w-3 text-red-500" data-testid="status-error" />
  }
}

export function ToolCard({ execution }: ToolCardProps) {
  const hasContent = !!execution.result || Object.keys(execution.toolInput ?? {}).length > 0
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className={cn(
        'overflow-hidden rounded border',
        execution.status === 'error'
          ? 'border-red-500/50'
          : 'border-gray-200/50 dark:border-gray-700/50',
      )}
    >
      {/* Header bar — always visible, clickable */}
      <button
        type="button"
        onClick={() => hasContent && setExpanded(!expanded)}
        className={cn(
          'flex w-full items-center gap-2 px-2.5 py-1.5',
          'bg-gray-50 dark:bg-gray-800/40',
          hasContent && 'cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800/60',
        )}
      >
        <Wrench className="h-3 w-3 text-gray-400" />
        <span className="font-mono text-xs font-medium text-gray-700 dark:text-gray-300">
          {execution.toolName}
        </span>
        <StatusIcon status={execution.status} />

        {execution.category && (
          <span className="rounded-full bg-gray-200/60 px-1.5 py-0.5 text-[9px] text-gray-500 dark:bg-gray-700/60 dark:text-gray-400">
            {execution.category}
          </span>
        )}

        {execution.duration != null && (
          <span className="text-[10px] text-gray-400">
            {(execution.duration / 1000).toFixed(1)}s
          </span>
        )}

        {execution.status === 'running' && execution.progress && (
          <span className="text-[10px] text-blue-400">
            {execution.progress.elapsedSeconds.toFixed(1)}s
          </span>
        )}

        <span className="ml-auto">
          {hasContent &&
            (expanded ? (
              <ChevronDown className="h-3 w-3 text-gray-400" />
            ) : (
              <ChevronRight className="h-3 w-3 text-gray-400" />
            ))}
        </span>
      </button>

      {/* Body — visible when expanded */}
      {expanded && (
        <div className="space-y-2 border-t border-gray-200/30 px-2.5 py-2 dark:border-gray-700/30">
          {/* Input section */}
          {Object.keys(execution.toolInput ?? {}).length > 0 && (
            <div>
              <div className="mb-1 text-[10px] font-medium text-gray-500 dark:text-gray-400">
                Input
              </div>
              <ContentRenderer content={JSON.stringify(execution.toolInput, null, 2)} />
            </div>
          )}

          {/* Output / Error section */}
          {execution.result && (
            <div>
              <div
                className={cn(
                  'mb-1 text-[10px] font-medium',
                  execution.result.isError ? 'text-red-500' : 'text-gray-500 dark:text-gray-400',
                )}
              >
                {execution.result.isError ? 'Error' : 'Output'}
              </div>
              <ContentRenderer content={execution.result.output} />
            </div>
          )}

          {/* Summary */}
          {execution.summary && (
            <p className="text-[10px] italic text-gray-500 dark:text-gray-400">
              {execution.summary}
            </p>
          )}
        </div>
      )}
    </div>
  )
}
