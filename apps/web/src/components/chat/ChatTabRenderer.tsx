import type { IDockviewPanelHeaderProps } from 'dockview-react'
import { X } from 'lucide-react'
import type { LiveStatus } from '../../lib/derive-panel-mode'
import { cn } from '../../lib/utils'
import { ChatTabContextMenu } from './ChatTabContextMenu'

/**
 * Dot color aligned with sidebar SessionListItem.getStatusDotColor:
 * - needs_you → amber (matches Live Monitor)
 * - autonomous / other active → green
 * - inactive → gray
 */
function getTabDotColor(agentStateGroup: string | null, liveStatus: LiveStatus): string {
  if (liveStatus === 'inactive') return 'bg-gray-300 dark:bg-gray-600'
  if (agentStateGroup === 'needs_you') return 'bg-amber-500'
  return 'bg-green-500'
}

export function ChatTabRenderer({ api, params, containerApi }: IDockviewPanelHeaderProps) {
  const agentStateGroup = (params.agentStateGroup as string | null) ?? null
  const liveStatus = (params.liveStatus as LiveStatus) ?? 'inactive'

  const dotColor = getTabDotColor(agentStateGroup, liveStatus)
  const isAutonomous = agentStateGroup === 'autonomous'
  const showPulse = isAutonomous && liveStatus !== 'inactive'

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    api.close()
  }

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault()
      e.stopPropagation()
      api.close()
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

  if (panel) {
    return (
      <ChatTabContextMenu panel={panel} api={containerApi}>
        {tabContent}
      </ChatTabContextMenu>
    )
  }

  return tabContent
}
