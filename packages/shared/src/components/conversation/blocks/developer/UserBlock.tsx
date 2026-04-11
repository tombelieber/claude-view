import { Check, Loader2, X } from 'lucide-react'
import type { UserBlock as UserBlockType } from '../../../../types/blocks'
import { StatusBadge } from '../shared/StatusBadge'
import { MessageTimestamp } from '../shared/MessageTimestamp'
import { EventCard } from './EventCard'
import { RENDERED_KEYS as LINEAGE_KEYS, MessageLineageDetail } from './details/MessageLineageDetail'
import { RawEnvelopeDetail } from './details/RawEnvelopeDetail'

const USER_RENDERED_KEYS = [...LINEAGE_KEYS, 'imagePasteIds'] as string[]

interface UserBlockProps {
  block: UserBlockType
}

function StatusDot({ status }: { status: UserBlockType['status'] }) {
  switch (status) {
    case 'optimistic':
      return (
        <span className="inline-block w-1.5 h-1.5 rounded-full bg-gray-400 dark:bg-gray-500 animate-pulse" />
      )
    case 'sending':
      return <Loader2 className="w-3 h-3 text-gray-400 dark:text-gray-500 animate-spin" />
    case 'sent':
      return <Check className="w-3 h-3 text-gray-400 dark:text-gray-500" />
    case 'failed':
      return <X className="w-3 h-3 text-red-500 dark:text-red-400" />
    default:
      return null
  }
}

export function DevUserBlock({ block }: UserBlockProps) {
  return (
    <EventCard
      dot={block.status === 'failed' ? 'red' : 'blue'}
      chip="User"
      label={block.text?.slice(0, 40) || block.id.slice(0, 8)}
      rawData={block}
      error={block.status === 'failed'}
      pulse={block.status === 'sending' || block.status === 'optimistic'}
      meta={
        <div className="flex items-center gap-1.5">
          {block.agentId && <StatusBadge label={`Agent: ${block.agentId}`} color="indigo" />}
          <StatusDot status={block.status} />
        </div>
      }
    >
      <div className="space-y-1.5">
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words">
          {block.text}
        </p>
        <MessageTimestamp timestamp={block.timestamp} />
        {block.images && block.images.length > 0 && (
          <div className="flex flex-wrap gap-2 mt-2">
            {block.images.map((img, i) => (
              <img
                // biome-ignore lint/suspicious/noArrayIndexKey: images have no stable id
                key={i}
                src={img.url ?? `data:${img.mediaType};base64,${img.data}`}
                alt={`Pasted ${i + 1}`}
                className="max-w-xs max-h-48 rounded border border-gray-200 dark:border-gray-700"
              />
            ))}
          </div>
        )}
        {block.rawJson != null && (
          <div className="space-y-1">
            {Array.isArray(block.rawJson.imagePasteIds) && (
              <span className="text-xs text-gray-500 dark:text-gray-400">
                {(block.rawJson.imagePasteIds as unknown[]).length} image(s) pasted
              </span>
            )}
            <MessageLineageDetail rawJson={block.rawJson} />
            <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={USER_RENDERED_KEYS} />
          </div>
        )}
      </div>
    </EventCard>
  )
}
