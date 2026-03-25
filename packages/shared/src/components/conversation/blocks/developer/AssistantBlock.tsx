import type {
  AssistantBlock as AssistantBlockType,
  AssistantSegment,
} from '../../../../types/blocks'
import { ThinkingBlock } from '../../../ThinkingBlock'
import { Markdown } from '../shared/Markdown'
import { MessageTimestamp } from '../shared/MessageTimestamp'
import { DurationBadge } from './DurationBadge'
import { EventCard } from './EventCard'
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
    <EventCard
      dot={block.streaming ? 'amber' : 'green'}
      chip="Assistant"
      label={
        block.segments.find((s) => s.kind === 'text')?.text?.slice(0, 40) || block.id.slice(0, 8)
      }
      rawData={block}
      pulse={block.streaming}
      meta={
        <div className="flex items-center gap-1.5">
          {block.rawJson?.permissionMode != null && (
            <span className="font-mono text-xs px-1.5 py-0.5 rounded bg-gray-500/10 dark:bg-gray-500/20 text-gray-600 dark:text-gray-300">
              {String(block.rawJson.permissionMode)}
            </span>
          )}
          {durationMs != null && <DurationBadge ms={durationMs} />}
        </div>
      }
    >
      <div className="space-y-2">
        {block.thinking && <ThinkingBlock thinking={block.thinking} />}

        {block.segments.map((seg, i) => (
          <SegmentRenderer
            key={seg.kind === 'tool' ? seg.execution.toolUseId : `${block.id}-text-${i}`}
            segment={seg}
          />
        ))}

        {block.streaming && (
          <span className="inline-block w-[3px] h-[18px] bg-gray-800 dark:bg-gray-200 rounded-sm animate-pulse" />
        )}

        <MessageTimestamp timestamp={block.timestamp} />

        {block.rawJson && (
          <div className="space-y-1">
            <ThinkingMetadataDetail rawJson={block.rawJson} />
            <StopReasonDetail rawJson={block.rawJson} />
            <MessageLineageDetail rawJson={block.rawJson} />
            <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={ASSISTANT_RENDERED_KEYS} />
          </div>
        )}
      </div>
    </EventCard>
  )
}
