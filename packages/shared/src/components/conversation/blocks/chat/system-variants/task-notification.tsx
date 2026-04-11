import type { TaskNotification } from '../../../../../types/sidecar-protocol'
import { Bell, CheckCircle2, XCircle } from 'lucide-react'

interface Props {
  data: TaskNotification
}

export function TaskNotificationPill({ data }: Props) {
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
