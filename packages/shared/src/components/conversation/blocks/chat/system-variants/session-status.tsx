import type { SessionStatus } from '../../../../../types/sidecar-protocol'
import { Activity } from 'lucide-react'
import { StatusBadge } from '../../shared/StatusBadge'

interface Props {
  data: SessionStatus
}

export function SessionStatusPill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Activity className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{data?.status ?? 'idle'}</span>
      {data?.permissionMode && <StatusBadge label={data.permissionMode} color="gray" />}
    </div>
  )
}
