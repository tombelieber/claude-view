import { useState } from 'react'
import { Plug, ChevronRight, ChevronDown } from 'lucide-react'
import { CompactCodeBlock } from './live/CompactCodeBlock'

interface McpProgressCardProps {
  server: string
  method: string
  params?: object
  result?: object
  blockId?: string
  verboseMode?: boolean
}

export function McpProgressCard({
  server,
  method,
  params,
  result,
  blockId,
  verboseMode,
}: McpProgressCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  const paramsLabel = params ? '' : ' (no params)'

  return (
    <div className="py-0.5 border-l-2 border-l-purple-400 pl-1 my-1">
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-label="MCP tool call"
        aria-expanded={expanded}
      >
        <Plug className="w-3 h-3 text-purple-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {server}.{method}{paramsLabel}
        </span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded params/result via CompactCodeBlock */}
      {expanded && (
        <div className="mt-0.5 space-y-0.5">
          {params && (
            <CompactCodeBlock
              code={JSON.stringify(params, null, 2)}
              language="json"
              blockId={blockId ? `${blockId}-params` : `mcp-${server}-${method}-params`}
            />
          )}
          {result && (
            <CompactCodeBlock
              code={JSON.stringify(result, null, 2)}
              language="json"
              blockId={blockId ? `${blockId}-result` : `mcp-${server}-${method}-result`}
            />
          )}
        </div>
      )}
    </div>
  )
}
