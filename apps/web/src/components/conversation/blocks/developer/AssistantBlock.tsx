import type {
  AssistantBlock as AssistantBlockType,
  AssistantSegment,
} from '@claude-view/shared/types/blocks'
import { ThinkingBlock } from '../../../chat/ThinkingBlock'
import { Markdown } from '../shared/Markdown'
import { ToolCard } from './ToolCard'
import { RENDERED_KEYS as LINEAGE_KEYS, MessageLineageDetail } from './details/MessageLineageDetail'
import { RawEnvelopeDetail } from './details/RawEnvelopeDetail'
import { RENDERED_KEYS as STOP_KEYS, StopReasonDetail } from './details/StopReasonDetail'
import {
  RENDERED_KEYS as THINKING_META_KEYS,
  ThinkingMetadataDetail,
} from './details/ThinkingMetadataDetail'

const ASSISTANT_RENDERED_KEYS = [
  ...THINKING_META_KEYS,
  ...STOP_KEYS,
  ...LINEAGE_KEYS,
  'permissionMode',
  'durationMs',
] as string[]

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
      <ToolCard execution={segment.execution} />
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

      {block.rawJson && (
        <>
          {block.rawJson.permissionMode && (
            <span className="font-mono text-[10px] px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300">
              {String(block.rawJson.permissionMode)}
            </span>
          )}
          {block.rawJson.durationMs != null && (
            <span className="text-[10px] text-gray-500 dark:text-gray-400">
              {String(block.rawJson.durationMs)}ms
            </span>
          )}
          <ThinkingMetadataDetail rawJson={block.rawJson} />
          <StopReasonDetail rawJson={block.rawJson} />
          <MessageLineageDetail rawJson={block.rawJson} />
          <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={ASSISTANT_RENDERED_KEYS} />
        </>
      )}
    </div>
  )
}
