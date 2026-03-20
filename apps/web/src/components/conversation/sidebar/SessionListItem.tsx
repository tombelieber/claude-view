import type { LiveSession } from '@claude-view/shared/types/generated'
import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { Clock, FolderOpen, GitBranch, MoreVertical } from 'lucide-react'
import { forwardRef, useCallback, useState } from 'react'
import type { SessionInfo } from '../../../types/generated/SessionInfo'

interface Props {
  session: SessionInfo & {
    isActive?: boolean
    liveData?: LiveSession | null
    isSidecarManaged?: boolean
  }
  isSelected: boolean
  isKeyboardActive?: boolean
  onSelect: (sessionId: string) => void
  onResume?: (sessionId: string) => void
  onFork?: (sessionId: string) => void
  onDelete?: (sessionId: string) => void
}

function projectNameFromCwd(cwd: string): string {
  const parts = cwd.split('/')
  return parts[parts.length - 1] || cwd
}

function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000
  const diff = now - timestamp
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

// Aligned with Live Monitor StatusDot: needs_you → amber, autonomous → green
function getStatusDotColor(session: Props['session']): string {
  if (!session.liveData) return 'bg-gray-300 dark:bg-gray-600'
  const group = session.liveData.agentState?.group
  if (group === 'needs_you') return 'bg-amber-500'
  return 'bg-green-500'
}

function getStatusBadge(session: Props['session']): { text: string; className: string } | null {
  if (!session.liveData) return null
  const group = session.liveData.agentState?.group
  const isNeedsYou = group === 'needs_you'

  if (session.isSidecarManaged) {
    // Sidecar-managed: "Live" badge, color matches agent state
    return isNeedsYou
      ? {
          text: 'Live',
          className: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
        }
      : {
          text: 'Live',
          className: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
        }
  }
  // Watching: external session, color matches agent state
  return isNeedsYou
    ? {
        text: 'Watching',
        className: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
      }
    : {
        text: 'Watching',
        className: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
      }
}

export const SessionListItem = forwardRef<HTMLDivElement, Props>(function SessionListItem(
  { session, isSelected, isKeyboardActive, onSelect, onResume, onFork, onDelete },
  ref,
) {
  const [menuOpen, setMenuOpen] = useState(false)

  const handleClick = useCallback(() => onSelect(session.id), [onSelect, session.id])

  const title = session.slug || session.preview?.slice(0, 60) || session.id.slice(0, 8)

  const projectName = session.projectPath ? projectNameFromCwd(session.projectPath) : null
  const dotColor = getStatusDotColor(session)
  const isAutonomous = session.liveData?.agentState?.group === 'autonomous'
  const showPulse = isAutonomous && session.liveData != null
  const badge = getStatusBadge(session)

  return (
    <div
      ref={ref}
      className={[
        'group relative flex items-start gap-2 px-3 py-2 rounded-md cursor-pointer transition-colors',
        isSelected
          ? 'bg-blue-500/10 dark:bg-blue-400/10 text-blue-700 dark:text-blue-300'
          : isKeyboardActive
            ? 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300'
            : 'hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300',
      ].join(' ')}
      onClick={handleClick}
    >
      {/* Status dot — aligned with Live Monitor StatusDot colors */}
      <div className="mt-1 flex-shrink-0 relative inline-flex">
        {showPulse && (
          <span
            className={`absolute inline-flex w-2 h-2 rounded-full opacity-60 motion-safe:animate-live-ring ${dotColor}`}
          />
        )}
        <span
          className={`relative inline-flex w-2 h-2 rounded-full ${dotColor} ${showPulse ? 'motion-safe:animate-live-breathe' : ''}`}
        />
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <p className="text-sm font-medium truncate">{title}</p>
          {badge && (
            <span
              className={`flex-shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded-full leading-none ${badge.className}`}
            >
              {badge.text}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 mt-0.5 flex-wrap">
          {projectName && (
            <span className="flex items-center gap-0.5 text-xs text-gray-400 dark:text-gray-500">
              <FolderOpen size={10} />
              <span className="truncate max-w-20">{projectName}</span>
            </span>
          )}
          {session.gitBranch && (
            <span className="flex items-center gap-0.5 text-xs text-gray-400 dark:text-gray-500">
              <GitBranch size={10} />
              <span className="truncate max-w-20">{session.gitBranch}</span>
            </span>
          )}
          <span className="flex items-center gap-0.5 text-xs text-gray-400 dark:text-gray-500">
            <Clock size={10} />
            {formatRelativeTime(session.modifiedAt)}
          </span>
        </div>
        {session.liveData?.currentActivity && (
          <p className="text-xs text-gray-400 dark:text-gray-500 truncate mt-0.5">
            {session.liveData.currentActivity}
          </p>
        )}
      </div>

      {/* Context menu */}
      <DropdownMenu.Root open={menuOpen} onOpenChange={setMenuOpen}>
        <DropdownMenu.Trigger asChild>
          <button
            className={[
              'flex-shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              menuOpen ? 'opacity-100' : '',
            ].join(' ')}
            onClick={(e) => e.stopPropagation()}
          >
            <MoreVertical size={14} />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className="z-50 min-w-32 rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg py-1 text-sm"
            sideOffset={4}
          >
            {onResume && (
              <DropdownMenu.Item
                className="px-3 py-1.5 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300 outline-none"
                onSelect={() => onResume(session.id)}
              >
                Resume
              </DropdownMenu.Item>
            )}
            {onFork && (
              <DropdownMenu.Item
                className="px-3 py-1.5 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300 outline-none"
                onSelect={() => onFork(session.id)}
              >
                Fork
              </DropdownMenu.Item>
            )}
            {onDelete && (
              <>
                <DropdownMenu.Separator className="my-1 border-t border-gray-200 dark:border-gray-700" />
                <DropdownMenu.Item
                  className="px-3 py-1.5 cursor-pointer hover:bg-red-50 dark:hover:bg-red-900/20 text-red-600 dark:text-red-400 outline-none"
                  onSelect={() => onDelete(session.id)}
                >
                  Delete
                </DropdownMenu.Item>
              </>
            )}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  )
})
