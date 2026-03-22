import type { UserBlock as UserBlockType } from '@claude-view/shared/types/blocks'
import { Check, Loader2, X } from 'lucide-react'
import { MessageTimestamp } from '../shared/MessageTimestamp'
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
    <div className="rounded-lg bg-blue-500/5 dark:bg-blue-400/5 border border-blue-500/15 dark:border-blue-400/15 px-4 py-3">
      <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words">
        {block.text}
      </p>
      <div className="flex items-center gap-2 mt-1.5">
        <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
          {block.id.slice(0, 8)}
        </span>
        <MessageTimestamp timestamp={block.timestamp} />
        <StatusDot status={block.status} />
      </div>
      {block.rawJson != null && (
        <div className="mt-1.5 space-y-1">
          {Array.isArray(block.rawJson.imagePasteIds) && (
            <span className="text-[10px] text-gray-500 dark:text-gray-400">
              {(block.rawJson.imagePasteIds as unknown[]).length} image(s) pasted
            </span>
          )}
          <MessageLineageDetail rawJson={block.rawJson} />
          <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={USER_RENDERED_KEYS} />
        </div>
      )}
    </div>
  )
}
