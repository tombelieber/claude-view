import { Clock } from 'lucide-react'
import { cn } from '../lib/utils'

interface TaskQueueCardProps {
  waitDuration?: number
  position?: number
  queueLength?: number
}

export function TaskQueueCard({
  waitDuration,
  position,
  queueLength,
}: TaskQueueCardProps) {
  const details: string[] = []

  if (position !== undefined) {
    details.push(
      queueLength !== undefined
        ? `position ${position}/${queueLength}`
        : `position ${position}`
    )
  }

  if (waitDuration !== undefined) {
    details.push(`${waitDuration}s`)
  }

  const hasDetails = details.length > 0
  const suffix = hasDetails ? ` (${details.join(', ')})` : '...'

  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 dark:border-gray-700 border-l-4 border-l-gray-400 bg-gray-50 dark:bg-gray-800 my-2 overflow-hidden'
      )}
      aria-label="Task queue status"
    >
      <div className="flex items-center gap-2 px-3 py-2">
        <Clock className="w-4 h-4 text-gray-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-sm text-gray-700 dark:text-gray-300">
          Waiting for task{suffix}
        </span>
      </div>
    </div>
  )
}
