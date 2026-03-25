import type { ProgressBlock } from '../../../../types/blocks'
import { Bot, ChevronDown, ChevronRight } from 'lucide-react'
import { useMemo, useState } from 'react'
import { summarizeAgentGroup, formatToolSummary } from '../../../../utils/agent-group'
import { cn } from '../../../../utils/cn'

interface AgentGroupRowProps {
  blocks: ProgressBlock[]
}

/** Chat mode: collapsed agent progress group — shows description + tool summary. */
export function ChatAgentGroupRow({ blocks }: AgentGroupRowProps) {
  const [expanded, setExpanded] = useState(false)
  const summary = useMemo(() => summarizeAgentGroup(blocks), [blocks])

  const toolStr = formatToolSummary(summary.tools)
  const promptLabel = summary.prompt
    ? summary.prompt.length > 60
      ? `${summary.prompt.slice(0, 60)}…`
      : summary.prompt
    : 'Agent'

  return (
    <div className="text-xs">
      {/* Header row */}
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className={cn(
          'flex items-center gap-2 px-3 py-1.5 w-full text-left cursor-pointer',
          'hover:bg-indigo-50/50 dark:hover:bg-indigo-950/30 rounded transition-colors',
        )}
      >
        <span className="w-1.5 h-1.5 rounded-full bg-indigo-500 animate-pulse flex-shrink-0" />
        <Bot className="w-3.5 h-3.5 text-indigo-500 dark:text-indigo-400 flex-shrink-0" />
        <span className="text-gray-700 dark:text-gray-300 font-medium truncate">{promptLabel}</span>

        {/* Tool summary badge */}
        {toolStr && (
          <span className="font-mono text-xs text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 dark:bg-indigo-500/20 px-1.5 py-0.5 rounded-full flex-shrink-0 ml-auto">
            {toolStr}
          </span>
        )}

        {/* Op count */}
        <span className="font-mono text-xs text-gray-500 dark:text-gray-500 flex-shrink-0 tabular-nums">
          {blocks.length} msgs
        </span>

        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded: show individual operations */}
      {expanded && (
        <div className="ml-6 pl-2 border-l-2 border-indigo-200 dark:border-indigo-800/50 space-y-0.5 py-1">
          {blocks.map((block) => (
            <AgentOpLine key={block.id} block={block} />
          ))}
        </div>
      )}
    </div>
  )
}

/** Single operation line inside an expanded agent group. */
function AgentOpLine({ block }: { block: ProgressBlock }) {
  if (block.data.type !== 'agent') return null

  const msg = block.data.message as Record<string, unknown> | undefined
  if (!msg) return null

  const inner = msg.message as Record<string, unknown> | undefined
  const content = inner?.content
  if (!Array.isArray(content)) return null

  // Extract tool names or text
  const parts: string[] = []
  for (const c of content) {
    if (!c || typeof c !== 'object') continue
    const ct = (c as Record<string, unknown>).type
    if (ct === 'tool_use') {
      const name = (c as Record<string, unknown>).name
      if (typeof name === 'string') parts.push(name)
    } else if (ct === 'tool_result') {
      const resultContent = (c as Record<string, unknown>).content
      if (typeof resultContent === 'string') {
        parts.push(`← ${resultContent.slice(0, 60)}${resultContent.length > 60 ? '…' : ''}`)
      } else {
        parts.push('← result')
      }
    } else if (ct === 'text') {
      const text = (c as Record<string, unknown>).text
      if (typeof text === 'string' && text.length > 0) {
        parts.push(text.slice(0, 80) + (text.length > 80 ? '…' : ''))
      }
    }
  }

  if (parts.length === 0) return null

  const isAssistant = msg.type === 'assistant'
  const arrow = isAssistant ? '→' : '←'
  const color = isAssistant
    ? 'text-indigo-600 dark:text-indigo-400'
    : 'text-gray-500 dark:text-gray-500'

  return (
    <div className="flex items-center gap-1.5 px-1 py-0.5 text-xs font-mono">
      <span className={color}>{arrow}</span>
      <span className="text-gray-600 dark:text-gray-400 truncate">{parts.join(', ')}</span>
    </div>
  )
}
