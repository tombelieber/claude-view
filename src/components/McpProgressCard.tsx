import { useState } from 'react'
import { Plug, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'

interface McpProgressCardProps {
  server: string
  method: string
  params?: object
  result?: object
}

export function McpProgressCard({
  server,
  method,
  params,
  result,
}: McpProgressCardProps) {
  const [expanded, setExpanded] = useState(false)

  const paramsLabel = params ? '' : ' (no params)'

  return (
    <div
      className={cn(
        'rounded-lg border border-purple-200 dark:border-purple-800 border-l-4 border-l-purple-400 bg-purple-50 dark:bg-purple-950/30 my-2 overflow-hidden'
      )}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-purple-100 dark:hover:bg-purple-900/30 transition-colors"
        aria-label="MCP tool call"
        aria-expanded={expanded}
      >
        <Plug className="w-4 h-4 text-purple-600 flex-shrink-0" aria-hidden="true" />
        <span className="text-sm text-purple-900 dark:text-purple-200 truncate flex-1">
          MCP: {server}.{method}{paramsLabel}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-purple-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-purple-400" />
        )}
      </button>

      {expanded && (
        <div className="px-3 py-2 border-t border-purple-100 dark:border-purple-800 bg-purple-50/50 dark:bg-purple-950/20 space-y-2">
          {params && (
            <div>
              <div className="text-xs font-medium text-purple-700 dark:text-purple-400 mb-1">Params:</div>
              <pre className="text-xs text-purple-800 dark:text-purple-300 font-mono whitespace-pre-wrap break-all">
                {JSON.stringify(params, null, 2)}
              </pre>
            </div>
          )}
          {result && (
            <div>
              <div className="text-xs font-medium text-purple-700 dark:text-purple-400 mb-1">Result:</div>
              <pre className="text-xs text-purple-800 dark:text-purple-300 font-mono whitespace-pre-wrap break-all">
                {JSON.stringify(result, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
