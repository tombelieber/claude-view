import type { SystemBlock as SystemBlockType } from '@claude-view/shared/types/blocks'
import type {
  QueueOperation,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
} from '@claude-view/shared/types/sidecar-protocol'
import { Activity, Bell, Clock, Play } from 'lucide-react'

/** Variants that ChatSystemBlock actually renders — used to filter items before Virtuoso. */
export const CHAT_SYSTEM_VARIANTS = new Set([
  'task_started',
  'task_progress',
  'task_notification',
  'queue_operation',
])

/** Returns true if a queue_operation system block should render in chat mode. */
export function isChatVisibleQueueOp(block: SystemBlockType): boolean {
  if (block.variant !== 'queue_operation') return false
  const data = block.data as QueueOperation
  return data.operation === 'enqueue' && !!data.content?.trim()
}

interface SystemBlockProps {
  block: SystemBlockType
}

export function ChatSystemBlock({ block }: SystemBlockProps) {
  switch (block.variant) {
    case 'task_started': {
      const data = block.data as TaskStarted
      return (
        <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
          <Play className="w-3 h-3 flex-shrink-0" />
          <span className="truncate">Task: {data.description}</span>
        </div>
      )
    }
    case 'task_progress': {
      const data = block.data as TaskProgressEvent
      return (
        <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
          <Activity className="w-3 h-3 flex-shrink-0" />
          <span className="truncate">{data.summary ?? data.description}</span>
        </div>
      )
    }
    case 'task_notification': {
      const data = block.data as TaskNotification
      return (
        <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
          <Bell className="w-3 h-3 flex-shrink-0" />
          <span className="truncate">
            Task {data.status}: {data.summary}
          </span>
        </div>
      )
    }
    case 'queue_operation': {
      const data = block.data as QueueOperation
      // Render user-typed enqueue as a chat bubble (right-aligned, like user messages)
      return (
        <div data-testid="queued-user-message" className="flex justify-end">
          <div className="max-w-[80%]">
            <div className="px-3.5 py-2.5 rounded-2xl rounded-br-md bg-blue-500/80 dark:bg-blue-600/80 text-white">
              <p className="text-sm whitespace-pre-wrap break-words">{data.content}</p>
            </div>
            <div className="flex items-center justify-end gap-1 mt-1 px-1">
              <Clock className="w-2.5 h-2.5 text-gray-400 dark:text-gray-500" />
              <span className="text-[10px] text-gray-400 dark:text-gray-500">Queued</span>
            </div>
          </div>
        </div>
      )
    }
    default:
      return null
  }
}
