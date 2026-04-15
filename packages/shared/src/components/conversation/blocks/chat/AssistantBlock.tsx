import { useState } from 'react'
import type { AssistantBlock as AssistantBlockType } from '../../../../types/blocks'
import { Bot, Brain, ChevronDown, GitBranch } from 'lucide-react'
import { cn } from '../../../../utils/cn'
import { Markdown } from '../shared/Markdown'
import { MessageTimestamp } from '../shared/MessageTimestamp'
import { ToolChip } from '../shared/ToolChip'

interface AssistantBlockProps {
  block: AssistantBlockType
}

/**
 * Should a ChatAssistantBlock be hidden in chat mode?
 *
 * The stream accumulator creates assistant blocks eagerly — `ensureAssistant()`
 * pre-allocates `segments: []`, and `finalizeCurrentAssistant()` commits the
 * block even if no content ever arrived. Three real producers of empty blocks:
 *   - `message_start` → `content_block_start` → `message_stop` with no deltas
 *   - `turn_complete` / `turn_error` firing after ensureAssistant() but before content
 *   - API error aborting the turn before the first text delta
 *
 * Without this guard, the empty block still renders a bare `<MessageTimestamp>`
 * row — visually, a blank line with a lone timestamp. Developer mode shows the
 * block via DevAssistantBlock (zero data loss), but chat mode treats it as
 * progressive-disclosure noise and hides it.
 *
 * A streaming cursor counts as content (liveness signal). A non-empty
 * `thinking` string counts. Any tool-call segment counts. Whitespace-only text
 * does NOT count. agentId / sidechain badges alone do NOT justify a row — if
 * there's no actual content to attribute, the badge is just noise.
 */
export function isChatAssistantBlockEmpty(block: AssistantBlockType): boolean {
  if (block.streaming) return false
  if (block.thinking && block.thinking.length > 0) return false
  for (const seg of block.segments) {
    if (seg.kind === 'tool') return false
    if (seg.kind === 'text' && seg.text.trim().length > 0) return false
  }
  return true
}

function ThinkingIndicator({ thinking }: { thinking: string }) {
  const [expanded, setExpanded] = useState(false)

  // Estimate thinking duration from content length (heuristic: ~15 chars/sec of thinking)
  const charCount = thinking.length
  const estimatedSeconds = Math.max(1, Math.round(charCount / 15))

  return (
    <div className="space-y-0">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="inline-flex items-center gap-1.5 px-2 py-1 rounded text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800/60 transition-colors cursor-pointer"
      >
        <Brain className="w-3 h-3 text-violet-400 dark:text-violet-500" />
        <span>Reasoned for {estimatedSeconds}s</span>
        <ChevronDown
          className={cn('w-3 h-3 transition-transform duration-150', expanded && 'rotate-180')}
        />
      </button>
      {expanded && (
        <div className="mt-1 ml-2 pl-3 border-l-2 border-violet-200 dark:border-violet-800/50 text-xs text-gray-500 dark:text-gray-400 max-h-60 overflow-y-auto">
          <Markdown content={thinking} />
        </div>
      )}
    </div>
  )
}

export function ChatAssistantBlock({ block }: AssistantBlockProps) {
  if (isChatAssistantBlockEmpty(block)) return null

  const isSidechain = block.isSidechain === true

  return (
    <div
      data-testid="assistant-message"
      className={`space-y-2 ${isSidechain ? 'opacity-75 pl-4 border-l-2 border-purple-300 dark:border-purple-600 border-dashed' : ''}`}
    >
      {/* Agent / sidechain indicator */}
      {(block.agentId || isSidechain) && (
        <div className="flex items-center gap-1.5">
          {isSidechain && (
            <span className="inline-flex items-center gap-0.5 text-xs text-purple-500 dark:text-purple-400">
              <GitBranch className="w-2.5 h-2.5" />
              sidechain
            </span>
          )}
          {block.agentId && (
            <span className="inline-flex items-center gap-0.5 text-xs font-mono text-indigo-500 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/30 px-1.5 py-0.5 rounded-full">
              <Bot className="w-2.5 h-2.5" />
              {block.agentId}
            </span>
          )}
        </div>
      )}

      {/* Thinking indicator — collapsed by default */}
      {block.thinking && block.thinking.length > 0 && (
        <ThinkingIndicator thinking={block.thinking} />
      )}

      {block.segments.map((seg, i) => {
        if (seg.kind === 'text') {
          return (
            <div key={`${block.id}-text-${i}`}>
              <Markdown content={seg.text} />
            </div>
          )
        }
        return (
          <div key={`${block.id}-tool-${seg.execution.toolUseId}`}>
            <ToolChip execution={seg.execution} />
          </div>
        )
      })}

      {block.streaming && (
        <span className="inline-block w-2 h-4 bg-gray-800 dark:bg-gray-200 animate-pulse rounded-sm" />
      )}

      <div className="mt-1 px-1">
        <MessageTimestamp timestamp={block.timestamp} />
      </div>
    </div>
  )
}
