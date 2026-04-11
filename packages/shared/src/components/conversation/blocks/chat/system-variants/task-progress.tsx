import type { TaskProgressEvent } from '../../../../../types/sidecar-protocol'
import { Activity } from 'lucide-react'

interface Props {
  data: TaskProgressEvent
}

export function TaskProgressPill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Activity className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{data.summary ?? data.description}</span>
    </div>
  )
}
