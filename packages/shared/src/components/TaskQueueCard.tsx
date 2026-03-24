import { Clock } from 'lucide-react'

/**
 * TaskQueueCard — follows inline stats pattern.
 *
 * Schema: taskDescription, taskType
 */

interface TaskQueueCardProps {
  taskDescription: string
  taskType: string
}

export function TaskQueueCard({ taskDescription, taskType }: TaskQueueCardProps) {
  return (
    <div className="flex items-center gap-2 text-[10px] font-mono" aria-label="Task queue status">
      <Clock className="w-3 h-3 text-orange-500 flex-shrink-0" aria-hidden="true" />
      <span className="text-gray-700 dark:text-gray-300 truncate flex-1">
        {taskDescription || 'Waiting for task'}
      </span>
      <span className="px-1.5 py-0.5 rounded bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300 flex-shrink-0">
        {taskType}
      </span>
    </div>
  )
}
