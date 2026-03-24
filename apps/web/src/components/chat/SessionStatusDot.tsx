import { cn } from '../../lib/utils'

export type ChatSessionStatus = 'active' | 'idle' | 'watching' | 'error' | 'ended'

interface Props {
  status: ChatSessionStatus
  permissionPending?: boolean
}

export function SessionStatusDot({ status, permissionPending }: Props) {
  if (permissionPending) {
    return (
      <span className="relative flex h-2 w-2">
        <span className="absolute inline-flex h-full w-full rounded-full bg-amber-500 opacity-75 animate-ping" />
        <span className="relative inline-flex h-2 w-2 rounded-full bg-amber-500" />
      </span>
    )
  }

  const styles: Record<ChatSessionStatus, string> = {
    active: 'bg-green-500 animate-pulse',
    idle: 'bg-green-500',
    watching: 'bg-slate-400 ring-1 ring-slate-400/50',
    error: 'bg-red-500',
    ended: 'bg-slate-500',
  }

  return <span className={cn('inline-flex h-2 w-2 rounded-full flex-shrink-0', styles[status])} />
}
