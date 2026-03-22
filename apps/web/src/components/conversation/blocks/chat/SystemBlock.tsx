import type { SystemBlock as SystemBlockType } from '@claude-view/shared/types/blocks'
import type {
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
} from '@claude-view/shared/types/sidecar-protocol'
import { Activity, Bell, Play } from 'lucide-react'

/** Variants that ChatSystemBlock actually renders — used to filter items before Virtuoso. */
export const CHAT_SYSTEM_VARIANTS = new Set(['task_started', 'task_progress', 'task_notification'])

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
    default:
      return null
  }
}
