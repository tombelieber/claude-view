import {
  ArrowLeft,
  ArrowRight,
  Bot,
  ChevronDown,
  ChevronRight,
  FileText,
  MessageSquare,
  Terminal,
} from 'lucide-react'
import { useMemo, useState } from 'react'
import type { ProgressBlock } from '../../../../types/blocks'
import { formatToolSummary, summarizeAgentGroup } from '../../../../utils/agent-group'
import { cn } from '../../../../utils/cn'

interface DevAgentGroupRowProps {
  blocks: ProgressBlock[]
}

// ── Content item types from Claude API messages ─────────────────────────────

interface ToolUseItem {
  type: 'tool_use'
  id?: string
  name: string
  input?: Record<string, unknown>
}

interface ToolResultItem {
  type: 'tool_result'
  tool_use_id?: string
  content?: string | unknown[]
}

interface TextItem {
  type: 'text'
  text: string
}

type ContentItem = ToolUseItem | ToolResultItem | TextItem | Record<string, unknown>

// ── Per-content-type renderers ──────────────────────────────────────────────

const MAX_RESULT_CHARS = 120

function ToolUseRow({ item }: { item: ToolUseItem }) {
  const inputSummary = item.input
    ? Object.entries(item.input)
        .map(([k, v]) => {
          const val = typeof v === 'string' ? v : JSON.stringify(v)
          const short = val.length > 40 ? `${val.slice(0, 40)}…` : val
          return `${k}: ${short}`
        })
        .join(', ')
    : ''

  return (
    <div className="flex items-start gap-2 py-1 px-2 rounded hover:bg-indigo-50/30 dark:hover:bg-indigo-950/20 transition-colors">
      <ArrowRight className="w-3 h-3 text-indigo-500 dark:text-indigo-400 mt-0.5 flex-shrink-0" />
      <span className="text-xs font-mono font-semibold text-indigo-700 dark:text-indigo-300 flex-shrink-0">
        {item.name}
      </span>
      {inputSummary && (
        <span
          className="text-xs font-mono text-gray-500 dark:text-gray-500 truncate"
          title={inputSummary}
        >
          {inputSummary}
        </span>
      )}
      {item.id && (
        <span className="text-xs font-mono text-gray-400 dark:text-gray-600 ml-auto flex-shrink-0">
          {item.id.slice(0, 12)}
        </span>
      )}
    </div>
  )
}

function ToolResultRow({ item }: { item: ToolResultItem }) {
  const text =
    typeof item.content === 'string'
      ? item.content
      : Array.isArray(item.content)
        ? item.content.map((c) => (typeof c === 'string' ? c : JSON.stringify(c))).join('\n')
        : ''

  const truncated = text.length > MAX_RESULT_CHARS
  const display = truncated ? `${text.slice(0, MAX_RESULT_CHARS)}…` : text

  return (
    <div className="flex items-start gap-2 py-1 px-2 rounded hover:bg-gray-50/30 dark:hover:bg-gray-800/20 transition-colors">
      <ArrowLeft className="w-3 h-3 text-emerald-500 dark:text-emerald-400 mt-0.5 flex-shrink-0" />
      {item.tool_use_id && (
        <span className="text-xs font-mono text-gray-400 dark:text-gray-600 flex-shrink-0">
          {item.tool_use_id.slice(0, 12)}
        </span>
      )}
      <pre
        className="text-xs font-mono text-gray-600 dark:text-gray-400 whitespace-pre-wrap break-all flex-1 max-h-16 overflow-hidden"
        title={text}
      >
        {display || '(empty)'}
      </pre>
    </div>
  )
}

function TextRow({ item }: { item: TextItem }) {
  const truncated = item.text.length > 200
  const display = truncated ? `${item.text.slice(0, 200)}…` : item.text

  return (
    <div className="flex items-start gap-2 py-1 px-2">
      <MessageSquare className="w-3 h-3 text-gray-400 dark:text-gray-500 mt-0.5 flex-shrink-0" />
      <span className="text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap break-words">
        {display}
      </span>
    </div>
  )
}

function ContentItemRenderer({ item }: { item: ContentItem }) {
  if (!item || typeof item !== 'object' || !('type' in item)) return null

  switch (item.type) {
    case 'tool_use':
      return <ToolUseRow item={item as ToolUseItem} />
    case 'tool_result':
      return <ToolResultRow item={item as ToolResultItem} />
    case 'text':
      return <TextRow item={item as TextItem} />
    default:
      return null
  }
}

// ── Agent message row ───────────────────────────────────────────────────────

function AgentMessageRow({ block }: { block: ProgressBlock }) {
  if (block.data.type !== 'agent') return null

  const msg = block.data.message as Record<string, unknown> | undefined
  if (!msg) return null

  const isAssistant = msg.type === 'assistant'
  const inner = msg.message as Record<string, unknown> | undefined
  const content = inner?.content

  if (!Array.isArray(content) || content.length === 0) {
    // First block often has the prompt text
    if (block.data.prompt && msg.type === 'user') {
      return (
        <div className="flex items-start gap-2 py-1.5 px-2">
          <FileText className="w-3 h-3 text-indigo-400 dark:text-indigo-500 mt-0.5 flex-shrink-0" />
          <span className="text-xs text-gray-600 dark:text-gray-400 italic truncate">
            {block.data.prompt.length > 100
              ? `${block.data.prompt.slice(0, 100)}…`
              : block.data.prompt}
          </span>
        </div>
      )
    }
    return null
  }

  return (
    <div
      className={cn(
        'rounded-md border',
        isAssistant
          ? 'border-indigo-200/20 dark:border-indigo-800/20 bg-indigo-50/30 dark:bg-indigo-950/10'
          : 'border-gray-200/20 dark:border-gray-700/20 bg-gray-50/30 dark:bg-gray-800/10',
      )}
    >
      {/* Role indicator */}
      <div className="flex items-center gap-1.5 px-2 py-0.5">
        {isAssistant ? (
          <Bot className="w-2.5 h-2.5 text-indigo-500 dark:text-indigo-400" />
        ) : (
          <Terminal className="w-2.5 h-2.5 text-gray-400 dark:text-gray-500" />
        )}
        <span
          className={cn(
            'text-[10px] font-mono uppercase tracking-wider',
            isAssistant
              ? 'text-indigo-500 dark:text-indigo-400'
              : 'text-gray-400 dark:text-gray-500',
          )}
        >
          {isAssistant ? 'call' : 'result'}
        </span>
      </div>

      {/* Content items */}
      <div className="divide-y divide-gray-200/10 dark:divide-gray-700/10">
        {(content as ContentItem[]).map((item, i) => (
          <ContentItemRenderer
            // biome-ignore lint/suspicious/noArrayIndexKey: content array is static per render
            key={i}
            item={item}
          />
        ))}
      </div>
    </div>
  )
}

// ── Main component ──────────────────────────────────────────────────────────

/** Developer mode: agent progress group — expanded by default for debugging visibility. */
export function DevAgentGroupRow({ blocks }: DevAgentGroupRowProps) {
  const [expanded, setExpanded] = useState(true)
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
          {toolStr && (
            <span className="font-mono text-xs text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 dark:bg-indigo-500/20 px-1.5 py-0.5 rounded">
              {toolStr}
            </span>
          )}

          {summary.agentId && (
            <span className="text-xs font-mono text-gray-500 dark:text-gray-500">
              #{summary.agentId.slice(0, 8)}
            </span>
          )}

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

      {/* Expanded body — rich per-message renderers, no JSON dumps */}
      {expanded && (
        <div className="px-3 pb-2 space-y-1 border-t border-indigo-200/30 dark:border-indigo-800/20">
          {blocks.map((block) => (
            <AgentMessageRow key={block.id} block={block} />
          ))}
        </div>
      )}
    </div>
  )
}
