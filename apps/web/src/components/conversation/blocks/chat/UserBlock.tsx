import type { UserBlock as UserBlockType } from '@claude-view/shared/types/blocks'
import { Check, Loader2, X } from 'lucide-react'

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

export function ChatUserBlock({ block }: UserBlockProps) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[80%]">
        <div className="px-3.5 py-2.5 rounded-2xl rounded-br-md bg-blue-500 dark:bg-blue-600 text-white">
          <p className="text-sm whitespace-pre-wrap break-words">{block.text}</p>
        </div>
        <div className="flex items-center justify-end gap-1.5 mt-1 px-1">
          <StatusDot status={block.status} />
          {block.status === 'failed' && (
            <span className="text-[10px] text-red-500 dark:text-red-400">Failed</span>
          )}
        </div>
      </div>
    </div>
  )
}
