import type { HookEvent } from '../../../../../types/sidecar-protocol'
import { GitBranch } from 'lucide-react'

interface Props {
  data: HookEvent
}

export function HookEventPill({ data }: Props) {
  const isError = data?.outcome === 'error'
  return (
    <div
      className={`flex items-center gap-2 px-3 py-1 text-xs ${
        isError ? 'text-red-500 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'
      }`}
    >
      <GitBranch className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">
        {data?.hookName} ({data?.phase})
      </span>
    </div>
  )
}
