import type { ElicitationComplete } from '../../../../../types/sidecar-protocol'
import { CheckCircle2 } from 'lucide-react'

interface Props {
  data: ElicitationComplete
}

export function ElicitationCompletePill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <CheckCircle2 className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">
        {data?.mcpServerName} / {data?.elicitationId}
      </span>
    </div>
  )
}
