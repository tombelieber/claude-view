import type {
  AssistantBlock as AssistantBlockType,
  AssistantSegment,
} from '@claude-view/shared/types/blocks'
import { cn } from '../../../../lib/utils'
import { ThinkingBlock } from '../../../chat/ThinkingBlock'
import { Markdown } from '../shared/Markdown'
import { MessageTimestamp } from '../shared/MessageTimestamp'
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
  const durationMs = block.rawJson?.durationMs as number | undefined

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

      <div className="flex items-center gap-2">
        <MessageTimestamp timestamp={block.timestamp} />
        {durationMs != null && (
          <span
            className={cn(
              'text-[9px] font-mono tabular-nums px-1.5 py-0.5 rounded',
              durationMs > 30000
                ? 'text-red-400 bg-red-500/10'
                : durationMs > 5000
                  ? 'text-amber-400 bg-amber-500/10'
                  : 'text-gray-400 bg-gray-500/10',
            )}
          >
            {(durationMs / 1000).toFixed(1)}s
          </span>
        )}
      </div>

      {block.rawJson && (
        <div className="space-y-1">
          {block.rawJson.permissionMode != null && (
            <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300">
              {String(block.rawJson.permissionMode)}
            </span>
          )}
          <ThinkingMetadataDetail rawJson={block.rawJson} />
          <StopReasonDetail rawJson={block.rawJson} />
          <MessageLineageDetail rawJson={block.rawJson} />
          <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={ASSISTANT_RENDERED_KEYS} />
        </div>
      )}
    </div>
  )
}
