import type {
  AssistantBlock as AssistantBlockType,
  AssistantSegment,
} from '@claude-view/shared/types/blocks'
import { ThinkingBlock } from '../../../chat/ThinkingBlock'
import { Markdown } from '../shared/Markdown'
import { ToolDetail } from '../shared/ToolDetail'

interface AssistantBlockProps {
  block: AssistantBlockType
}

function SegmentRenderer({ segment }: { segment: AssistantSegment }) {
  if (segment.kind === 'text') {
    const isNested = segment.parentToolUseId != null
    return (
      <div
        className={isNested ? 'ml-4 pl-3 border-l-2 border-purple-200 dark:border-purple-800' : ''}
      >
        <Markdown content={segment.text} />
      </div>
    )
  }

  const isNested = segment.execution.parentToolUseId != null
  return (
    <div
      className={isNested ? 'ml-4 pl-3 border-l-2 border-purple-200 dark:border-purple-800' : ''}
    >
      <ToolDetail execution={segment.execution} />
    </div>
  )
}

export function DevAssistantBlock({ block }: AssistantBlockProps) {
  return (
    <div className="space-y-2">
      {block.thinking && <ThinkingBlock content={block.thinking} defaultExpanded />}

      {block.segments.map((seg, i) => (
        <SegmentRenderer
          key={seg.kind === 'tool' ? seg.execution.toolUseId : `${block.id}-text-${i}`}
          segment={seg}
        />
      ))}

      {block.streaming && (
        <span className="inline-block w-2 h-4 bg-gray-800 dark:bg-gray-200 animate-pulse rounded-sm" />
      )}
    </div>
  )
}
