import type { UserBlock as UserBlockType } from '@claude-view/shared/types/blocks'
import { Check, Loader2, X } from 'lucide-react'

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
            <span className="text-[10px] text-gray-400 dark:text-gray-500">
              {formatTime(block.timestamp)}
            </span>
          )}
          <StatusDot status={block.status} />
        </div>
      </div>
    </div>
  )
}
