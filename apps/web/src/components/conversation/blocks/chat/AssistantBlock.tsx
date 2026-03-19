import type { AssistantBlock as AssistantBlockType } from '@claude-view/shared/types/blocks'
import { Markdown } from '../shared/Markdown'
import { MessageTimestamp } from '../shared/MessageTimestamp'
import { ToolChip } from '../shared/ToolChip'

interface AssistantBlockProps {
  block: AssistantBlockType
}

export function ChatAssistantBlock({ block }: AssistantBlockProps) {
  return (
    <div data-testid="assistant-message" className="space-y-2">
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
