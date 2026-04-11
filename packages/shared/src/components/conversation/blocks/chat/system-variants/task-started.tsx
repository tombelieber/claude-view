import type { TaskStarted } from '../../../../../types/sidecar-protocol'
import { Play } from 'lucide-react'

interface Props {
  data: TaskStarted
}

export function TaskStartedPill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Play className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">Task: {data.description}</span>
    </div>
  )
}
