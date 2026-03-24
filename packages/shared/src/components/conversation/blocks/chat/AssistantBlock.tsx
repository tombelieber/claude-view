import type { AssistantBlock as AssistantBlockType } from '../../../../types/blocks'
import { Bot, GitBranch } from 'lucide-react'
import { Markdown } from '../shared/Markdown'
import { MessageTimestamp } from '../shared/MessageTimestamp'
import { ToolChip } from '../shared/ToolChip'

interface AssistantBlockProps {
  block: AssistantBlockType
}

export function ChatAssistantBlock({ block }: AssistantBlockProps) {
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
            <span className="inline-flex items-center gap-0.5 text-[10px] text-purple-500 dark:text-purple-400">
              <GitBranch className="w-2.5 h-2.5" />
              sidechain
            </span>
          )}
          {block.agentId && (
            <span className="inline-flex items-center gap-0.5 text-[10px] font-mono text-indigo-500 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/30 px-1.5 py-0.5 rounded-full">
              <Bot className="w-2.5 h-2.5" />
              {block.agentId}
            </span>
          )}
        </div>
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
