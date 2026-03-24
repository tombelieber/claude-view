import type { SystemBlock as SystemBlockType } from '../../../../types/blocks'
import type {
  QueueOperation,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
} from '../../../../types/sidecar-protocol'
import {
  Activity,
  Bell,
  CheckCircle2,
  Clock,
  ExternalLink,
  FileText,
  Play,
  Tag,
  XCircle,
} from 'lucide-react'
import { Markdown } from '../shared/Markdown'

/** Variants that ChatSystemBlock actually renders — used to filter items before Virtuoso. */
export const CHAT_SYSTEM_VARIANTS = new Set([
  'task_started',
  'task_progress',
  'task_notification',
  'queue_operation',
  'pr_link',
  'custom_title',
  'plan_content',
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
      const isCompleted = data.status === 'completed'
      const isFailed = data.status === 'failed'
      const NotifIcon = isCompleted ? CheckCircle2 : isFailed ? XCircle : Bell
      const colorClass = isCompleted
        ? 'text-green-500 dark:text-green-400'
        : isFailed
          ? 'text-red-500 dark:text-red-400'
          : 'text-gray-500 dark:text-gray-400'
      return (
        <div className={`flex items-center gap-2 px-3 py-1 text-xs ${colorClass}`}>
          <NotifIcon className="w-3 h-3 flex-shrink-0" />
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
    case 'pr_link': {
      const data = block.data as Record<string, unknown>
      const prUrl = data.prUrl as string | undefined
      const prNumber = data.prNumber as number | undefined
      const prRepo = data.prRepository as string | undefined
      return (
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700">
          <ExternalLink className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400 flex-shrink-0" />
          <a
            href={prUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-blue-600 dark:text-blue-400 hover:underline truncate"
          >
            {prRepo ? `${prRepo}#${prNumber}` : `PR #${prNumber}`}
          </a>
        </div>
      )
    }
    case 'custom_title': {
      const data = block.data as Record<string, unknown>
      return (
        <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400">
          <Tag className="w-3 h-3 flex-shrink-0" />
          <span className="font-medium truncate">{data.customTitle as string}</span>
        </div>
      )
    }
    case 'plan_content': {
      const data = block.data as Record<string, unknown>
      const content = (data.planContent as string) || ''
      return (
        <div className="px-3 py-2 rounded-lg bg-violet-50 dark:bg-violet-900/15 border border-violet-200 dark:border-violet-800/40">
          <div className="flex items-center gap-1.5 mb-1.5">
            <FileText className="w-3 h-3 text-violet-500 dark:text-violet-400" />
            <span className="text-[10px] font-medium uppercase tracking-wider text-violet-500 dark:text-violet-400">
              Plan
            </span>
          </div>
          <div className="text-xs">
            <Markdown content={content} />
          </div>
        </div>
      )
    }
    default:
      return null
  }
}
