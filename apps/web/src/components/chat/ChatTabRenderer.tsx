import type { IDockviewPanelHeaderProps } from 'dockview-react'
import { X } from 'lucide-react'
import { cn } from '../../lib/utils'
import { ChatTabContextMenu } from './ChatTabContextMenu'

/** Session status → Tailwind dot color (working=green, paused=amber, done=gray). */
function statusDotColor(status: string | null): string {
  switch (status) {
    case 'working':
      return 'bg-green-500'
    case 'paused':
      return 'bg-amber-500'
    default:
      return 'bg-gray-300 dark:bg-gray-600'
  }
}

export function ChatTabRenderer({ api, params, containerApi }: IDockviewPanelHeaderProps) {
  const agentStateGroup = (params.agentStateGroup as string | null) ?? null
  const status = (params.status as string | null) ?? null
  const tmuxSessionId = (params.tmuxSessionId as string | undefined) ?? undefined
  const isTmux = !!tmuxSessionId

  const dotColor = statusDotColor(status)
  const isAutonomous = agentStateGroup === 'autonomous'
  const showPulse = isAutonomous && status === 'working'

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (tmuxSessionId) {
      fetch(`/api/cli-sessions/${tmuxSessionId}`, { method: 'DELETE' }).catch(() => {})
    }
    api.close()
  }

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) {
      handleClose(e)
    }
  }

  // Find this panel in the dockview API for context menu operations
  const panel = containerApi.panels.find((p) => p.id === api.id)

  const tabContent = (
    <div
      className="group flex items-center gap-1.5 px-3 h-full text-xs cursor-pointer"
      onMouseDown={handleMiddleClick}
    >
      {/* Status dot — aligned with sidebar SessionListItem colors */}
      <div className="flex-shrink-0 relative inline-flex">
        {showPulse && (
          <span
            className={`absolute inline-flex w-2 h-2 rounded-full opacity-60 motion-safe:animate-live-ring ${dotColor}`}
          />
        )}
        <span
          className={`relative inline-flex w-2 h-2 rounded-full ${dotColor} ${showPulse ? 'motion-safe:animate-live-breathe' : ''}`}
        />
      </div>
      <span className="truncate max-w-[120px]">{api.title}</span>
      <button
        type="button"
        onClick={handleClose}
        title={isTmux ? 'Kill CLI session' : undefined}
        className={cn(
          'ml-auto w-4 h-4 flex items-center justify-center rounded-sm',
          'text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700',
          isTmux ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
          !isTmux && api.isActive && 'opacity-100',
        )}
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )

  if (panel) {
    return (
      <ChatTabContextMenu panel={panel} api={containerApi}>
        {tabContent}
      </ChatTabContextMenu>
    )
  }

  return tabContent
}
