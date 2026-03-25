import type { ProgressBlock } from '../../../../types/blocks'
import { Bot, ChevronDown, ChevronRight } from 'lucide-react'
import { useMemo, useState } from 'react'
import { summarizeAgentGroup, formatToolSummary } from '../../../../utils/agent-group'
import { cn } from '../../../../utils/cn'
import { AgentProgressCard } from '../../../AgentProgressCard'

interface DevAgentGroupRowProps {
  blocks: ProgressBlock[]
}

/** Developer mode: collapsed agent progress group with expandable detail. */
export function DevAgentGroupRow({ blocks }: DevAgentGroupRowProps) {
  const [expanded, setExpanded] = useState(false)
  const summary = useMemo(() => summarizeAgentGroup(blocks), [blocks])

  const toolStr = formatToolSummary(summary.tools)
  const promptLabel = summary.prompt
    ? summary.prompt.length > 80
      ? `${summary.prompt.slice(0, 80)}…`
      : summary.prompt
    : 'Agent'

  return (
    <div className="rounded-lg border border-indigo-200/50 dark:border-indigo-800/30 bg-white dark:bg-gray-900">
      {/* Header */}
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className={cn(
          'flex items-center gap-2 px-3 py-2 w-full text-left cursor-pointer',
          'hover:bg-indigo-50/50 dark:hover:bg-indigo-950/30 rounded-lg transition-colors',
        )}
      >
        <span className="w-2 h-2 rounded-full bg-indigo-500 animate-pulse flex-shrink-0" />
        <span className="inline-flex items-center gap-1 text-xs font-mono bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300 px-1.5 py-0.5 rounded flex-shrink-0">
          <Bot className="w-3 h-3" />
          Agent
        </span>
        <span className="text-xs text-gray-700 dark:text-gray-300 font-medium truncate">
          {promptLabel}
        </span>

        <div className="flex items-center gap-2 ml-auto flex-shrink-0">
          {/* Tool summary */}
          {toolStr && (
            <span className="font-mono text-xs text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 dark:bg-indigo-500/20 px-1.5 py-0.5 rounded">
              {toolStr}
            </span>
          )}

          {/* Agent ID */}
          {summary.agentId && (
            <span className="text-xs font-mono text-gray-500 dark:text-gray-500">
              #{summary.agentId.slice(0, 8)}
            </span>
          )}

          {/* Count badge */}
          <span className="font-mono text-xs text-gray-500 dark:text-gray-500 tabular-nums">
            {blocks.length} msgs
          </span>

          {expanded ? (
            <ChevronDown className="w-3.5 h-3.5 text-gray-400" />
          ) : (
            <ChevronRight className="w-3.5 h-3.5 text-gray-400" />
          )}
        </div>
      </button>

      {/* Expanded body */}
      {expanded && (
        <div className="px-3 pb-2 space-y-1 border-t border-indigo-200/30 dark:border-indigo-800/20">
          {blocks.map((block) => {
            if (block.data.type !== 'agent') return null
            return (
              <div key={block.id} className="py-1">
                <AgentProgressCard
                  agentId={block.data.agentId}
                  prompt={block.data.prompt}
                  message={block.data.message}
                  blockId={block.id}
                />
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
