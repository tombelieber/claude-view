import type { UserBlock as UserBlockType } from '@claude-view/shared/types/blocks'
import * as Tooltip from '@radix-ui/react-tooltip'
import { Check, Loader2, X } from 'lucide-react'
import { formatFullTimestamp } from '../shared/MessageTimestamp'
import { RENDERED_KEYS as LINEAGE_KEYS, MessageLineageDetail } from './details/MessageLineageDetail'
import { RawEnvelopeDetail } from './details/RawEnvelopeDetail'

const USER_RENDERED_KEYS = [...LINEAGE_KEYS, 'imagePasteIds'] as string[]

interface UserBlockProps {
  block: UserBlockType
}

function formatTime(ts: number): string {
  if (ts <= 0) return ''
  return new Date(ts * 1000).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
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
    <div className="flex gap-3 px-4 py-3">
      <div className="w-0.5 shrink-0 rounded-full bg-blue-500 dark:bg-blue-400" />
      <div className="flex-1 min-w-0">
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words">
          {block.text}
        </p>
        <div className="flex items-center gap-2 mt-1">
          <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
            {block.id.slice(0, 8)}
          </span>
          {block.timestamp > 0 && (
            <Tooltip.Provider delayDuration={200}>
              <Tooltip.Root>
                <Tooltip.Trigger asChild>
                  <span className="text-[10px] text-gray-400 dark:text-gray-500 cursor-default">
                    {formatTime(block.timestamp)}
                  </span>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    side="bottom"
                    sideOffset={4}
                    className="z-50 rounded-md px-2.5 py-1.5 text-xs bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
                  >
                    {formatFullTimestamp(block.timestamp)}
                    <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
                  </Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </Tooltip.Provider>
          )}
          <StatusDot status={block.status} />
        </div>
        {block.rawJson && (
          <>
            {block.rawJson.imagePasteIds && Array.isArray(block.rawJson.imagePasteIds) && (
              <span className="text-[10px] text-gray-500 dark:text-gray-400">
                {(block.rawJson.imagePasteIds as unknown[]).length} image(s) pasted
              </span>
            )}
            <MessageLineageDetail rawJson={block.rawJson} />
            <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={USER_RENDERED_KEYS} />
          </>
        )}
      </div>
    </div>
  )
}
