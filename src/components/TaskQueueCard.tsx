import { Clock } from 'lucide-react'

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
      className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1"
      aria-label="Task queue status"
    >
      <div className="flex items-center gap-1.5">
        <Clock className="w-3 h-3 text-gray-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
          Waiting for task{suffix}
        </span>
      </div>
    </div>
  )
}
