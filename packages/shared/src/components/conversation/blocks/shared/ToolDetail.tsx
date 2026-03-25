import type { ToolExecution } from '../../../../types/blocks'
import { Check, ChevronDown, ChevronRight, Loader2, Wrench, X } from 'lucide-react'
import { useState } from 'react'

interface ToolDetailProps {
  execution: ToolExecution
}

function StatusBadge({ status }: { status: ToolExecution['status'] }) {
  switch (status) {
    case 'running':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400">
          <Loader2 className="w-3 h-3 animate-spin" />
          Running
        </span>
      )
    case 'complete':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-green-600 dark:text-green-400">
          <Check className="w-3 h-3" />
          Complete
        </span>
      )
    case 'error':
      return (
        <span className="inline-flex items-center gap-1 text-xs text-red-600 dark:text-red-400">
          <X className="w-3 h-3" />
          Error
        </span>
      )
  }
}

export function ToolDetail({ execution }: ToolDetailProps) {
  const [inputExpanded, setInputExpanded] = useState(false)

  return (
    <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-2.5 py-1.5 bg-gray-50 dark:bg-gray-800/40 border-b border-gray-200/50 dark:border-gray-700/50">
        <div className="flex items-center gap-2">
          <Wrench className="w-3.5 h-3.5 text-purple-500 dark:text-purple-400" />
          <span className="font-mono text-xs font-medium text-purple-700 dark:text-purple-300">
            {execution.toolName}
          </span>
          {execution.toolUseId && (
            <span className="text-xs font-mono text-gray-400 dark:text-gray-500">
              {execution.toolUseId.slice(0, 8)}
            </span>
          )}
        </div>
        <StatusBadge status={execution.status} />
      </div>

      {/* Input (collapsible) */}
      <div className="border-b border-gray-200/30 dark:border-gray-700/30">
        <button
          type="button"
          onClick={() => setInputExpanded(!inputExpanded)}
          className="flex items-center gap-1 w-full px-2.5 py-1 text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer"
        >
          {inputExpanded ? (
            <ChevronDown className="w-3 h-3" />
          ) : (
            <ChevronRight className="w-3 h-3" />
          )}
          Input
        </button>
        {inputExpanded && (
          <pre className="px-2.5 pb-2 text-xs text-gray-600 dark:text-gray-400 font-mono overflow-x-auto max-h-48 whitespace-pre-wrap">
            {JSON.stringify(execution.toolInput, null, 2)}
          </pre>
        )}
      </div>

      {/* Result output */}
      {execution.result && (
        <div className="px-2.5 py-1.5">
          <pre
            className={`text-xs font-mono overflow-x-auto max-h-48 whitespace-pre-wrap ${
              execution.result.isError
                ? 'text-red-600 dark:text-red-400'
                : 'text-gray-600 dark:text-gray-400'
            }`}
          >
            {execution.result.output}
          </pre>
        </div>
      )}

      {/* Progress */}
      {execution.progress && execution.status === 'running' && (
        <div className="px-2.5 py-1 text-xs text-gray-500 dark:text-gray-400 border-t border-gray-200/30 dark:border-gray-700/30">
          {execution.progress.elapsedSeconds.toFixed(1)}s elapsed
        </div>
      )}
    </div>
  )
}
