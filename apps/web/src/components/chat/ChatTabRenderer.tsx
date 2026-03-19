import type { IDockviewPanelHeaderProps } from 'dockview-react'
import { X } from 'lucide-react'
import { cn } from '../../lib/utils'
import { type ChatSessionStatus, SessionStatusDot } from './SessionStatusDot'

export function ChatTabRenderer({ api, params }: IDockviewPanelHeaderProps) {
  const status = (params.status as ChatSessionStatus) ?? 'idle'
  const hasPermissionPending = (params.permissionPending as boolean) ?? false

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (status === 'active') {
      // TODO: Show confirmation dialog before closing active session
      return
    }
    api.close()
  }

  return (
    <div className="group flex items-center gap-1.5 px-3 h-full text-xs cursor-pointer">
      <SessionStatusDot status={status} permissionPending={hasPermissionPending} />
      <span className="truncate max-w-[120px]">{api.title}</span>
      <button
        type="button"
        onClick={handleClose}
        className={cn(
          'ml-auto w-4 h-4 flex items-center justify-center rounded-sm',
          'text-[#8B949E] hover:text-[#F0F6FC] hover:bg-[#30363D]',
          'opacity-0 group-hover:opacity-100',
          api.isActive && 'opacity-100',
        )}
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )
}
