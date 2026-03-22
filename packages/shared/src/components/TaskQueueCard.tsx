import { Clock } from 'lucide-react'

/**
 * TaskQueueCard — purpose-built for TaskQueueProgress schema.
 *
 * Schema fields: taskDescription, taskType
 * Every field is rendered. No phantom props.
 */

interface TaskQueueCardProps {
  /** Human-readable description of the queued task */
  taskDescription: string
  /** Task classification (e.g. "local_bash") */
  taskType: string
}

export function TaskQueueCard({ taskDescription, taskType }: TaskQueueCardProps) {
  return (
    <div className="py-0.5 border-l-2 border-l-orange-400 pl-1 my-1" aria-label="Task queue status">
      <div className="flex items-center gap-1.5">
        <Clock className="w-3 h-3 text-orange-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {taskDescription || 'Waiting for task'}
        </span>
        <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300 flex-shrink-0">
          {taskType}
        </span>
      </div>
    </div>
  )
}
