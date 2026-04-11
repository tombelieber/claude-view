import type { SessionInit } from '../../../../../types/sidecar-protocol'
import { Settings } from 'lucide-react'

interface Props {
  data: SessionInit
}

export function SessionInitPill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Settings className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">
        {data?.model} · {data?.permissionMode}
      </span>
    </div>
  )
}
