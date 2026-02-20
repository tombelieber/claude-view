import { useState, useCallback, type ReactNode } from 'react'
import {
  Loader2,
  Pause,
  Maximize2,
  X,
  Pin,
  GitBranch,
  Bell,
} from 'lucide-react'
import { sessionTotalCost, type LiveSession } from './use-live-sessions'
import type { AgentStateGroup } from './types'
import { cn } from '../../lib/utils'
import { SubAgentPills } from './SubAgentPills'
import { cleanPreviewText } from '../../utils/get-session-title'

// --- Helpers ---

/** Extract the last path segment as the project display name. */
function projectName(session: LiveSession): string {
  if (session.projectDisplayName) return session.projectDisplayName
  if (session.projectPath) {
    const segments = session.projectPath.split('/')
    return segments[segments.length - 1] || session.project
  }
  return session.project
}

/** Format cost as $X.XX (or $X.XXXX for sub-cent values). */
function formatCost(session: LiveSession): string {
  const usd = sessionTotalCost(session)
  const formatted = usd === 0 ? '$0.00' : usd < 0.01 ? `$${usd.toFixed(4)}` : `$${usd.toFixed(2)}`
  return `${session.cost?.isEstimated ? '~' : ''}${formatted}`
}

/** Compute context window percentage from tokens + model. */
function contextPercent(session: LiveSession): number {
  // Use the same 200k default the ContextGauge uses
  const limit = 200_000
  return Math.min(Math.round((session.contextWindowTokens / limit) * 100), 100)
}

/** Color class for context percentage text. */
function contextColor(pct: number): string {
  if (pct > 80) return 'text-red-500 dark:text-red-400'
  if (pct >= 50) return 'text-amber-500 dark:text-amber-400'
  return 'text-green-600 dark:text-green-400'
}

/** Status icon component based on agentState.group. */
function GroupIcon({ group, className }: { group: AgentStateGroup; className?: string }) {
  switch (group) {
    case 'autonomous':
      return <Loader2 className={cn('animate-spin', className)} />
    case 'needs_you':
      return <Bell className={className} />
    default:
      return <Pause className={className} />
  }
}

// --- Props ---

export interface MonitorPaneProps {
  session: LiveSession
  isSelected: boolean
  isExpanded: boolean
  isPinned: boolean
  compactHeader: boolean
  isVisible: boolean
  onSelect: () => void
  onExpand: () => void
  onPin: () => void
  onHide: () => void
  onContextMenu: (e: React.MouseEvent) => void
  children?: ReactNode
}

// --- Component ---

export function MonitorPane({
  session,
  isSelected,
  isExpanded,
  isPinned,
  compactHeader,
  isVisible,
  onSelect,
  onExpand,
  onPin,
  onHide,
  onContextMenu,
  children,
}: MonitorPaneProps) {
  const [isHovered, setIsHovered] = useState(false)

  const handleDoubleClick = useCallback(() => {
    onExpand()
  }, [onExpand])

  const handleHeaderClick = useCallback(
    (e: React.MouseEvent) => {
      // Don't select if clicking a button
      if ((e.target as HTMLElement).closest('button')) return
      onSelect()
    },
    [onSelect]
  )

  if (!isVisible) return null

  const name = projectName(session)
  const ctxPct = contextPercent(session)
  const cost = formatCost(session)

  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border overflow-hidden bg-white dark:bg-[#0D1117] transition-all duration-150 h-full',
        isSelected
          ? 'ring-2 ring-blue-500 dark:ring-[#1F6FEB] border-blue-500 dark:border-[#1F6FEB] shadow-lg shadow-blue-500/10 dark:shadow-[#1F6FEB]/10'
          : isHovered
            ? 'border-gray-300 dark:border-[#30363D] shadow-md shadow-black/10 dark:shadow-black/30'
            : 'border-gray-200 dark:border-[#21262D]'
      )}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onDoubleClick={handleDoubleClick}
      onContextMenu={onContextMenu}
    >
      {/* Header */}
      {compactHeader ? (
        <CompactHeader
          session={session}
          name={name}
          cost={cost}
          ctxPct={ctxPct}
          isPinned={isPinned}
          onExpand={onExpand}
          onClick={handleHeaderClick}
        />
      ) : (
        <FullHeader
          session={session}
          name={name}
          cost={cost}
          ctxPct={ctxPct}
          isPinned={isPinned}
          onExpand={onExpand}
          onPin={onPin}
          onHide={onHide}
          onClick={handleHeaderClick}
        />
      )}

      {/* Content area */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {children ?? (
          <div className="flex items-center justify-center h-full min-h-[120px] text-sm text-gray-400 dark:text-[#8B949E]">
            <Loader2 className="w-4 h-4 mr-2 animate-spin text-blue-500 dark:text-[#79C0FF]" />
            Connecting...
          </div>
        )}
      </div>

      {/* Footer */}
      <Footer session={session} onExpand={onExpand} />
    </div>
  )
}

// --- Full Header ---

function FullHeader({
  session,
  name,
  cost,
  ctxPct,
  isPinned,
  onExpand,
  onPin,
  onHide,
  onClick,
}: {
  session: LiveSession
  name: string
  cost: string
  ctxPct: number
  isPinned: boolean
  onExpand: () => void
  onPin: () => void
  onHide: () => void
  onClick: (e: React.MouseEvent) => void
}) {
  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 bg-gray-50 dark:bg-[#161B22] border-b border-gray-200 dark:border-[#21262D] cursor-pointer select-none"
      onClick={onClick}
    >
      {/* Project name */}
      <span
        className="text-xs font-medium text-gray-800 dark:text-[#C9D1D9] truncate max-w-[20ch]"
        title={session.projectPath || name}
      >
        {name}
      </span>

      {/* Branch */}
      {session.gitBranch && (
        <span className="inline-flex items-center gap-0.5 text-[10px] font-mono text-gray-400 dark:text-[#6E7681] truncate max-w-[15ch]">
          <GitBranch className="w-2.5 h-2.5 flex-shrink-0" />
          <span className="truncate" title={session.gitBranch}>
            {session.gitBranch}
          </span>
        </span>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Metrics */}
      <span className="text-[10px] font-mono text-gray-500 dark:text-[#8B949E] tabular-nums flex-shrink-0">
        {cost}
      </span>
      <span className={cn('text-[10px] font-mono tabular-nums flex-shrink-0', contextColor(ctxPct))}>
        {ctxPct}% ctx
      </span>

      {/* Status icon */}
      <GroupIcon group={session.agentState.group} className="w-3 h-3 text-gray-400 dark:text-[#8B949E] flex-shrink-0" />

      {/* Divider */}
      <div className="w-px h-3.5 bg-gray-200 dark:bg-[#30363D]" />

      {/* Pin indicator */}
      {isPinned && (
        <Pin className="w-3 h-3 text-blue-500 dark:text-[#79C0FF] flex-shrink-0" />
      )}

      {/* Action buttons */}
      <button
        onClick={(e) => {
          e.stopPropagation()
          onPin()
        }}
        className={cn(
          'p-0.5 rounded hover:bg-gray-200 dark:hover:bg-[#30363D] transition-colors',
          isPinned ? 'text-blue-500 dark:text-[#79C0FF] hover:text-blue-600 dark:hover:text-[#A5D6FF]' : 'text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9]'
        )}
        title={isPinned ? 'Unpin pane' : 'Pin pane'}
      >
        <Pin className="w-3 h-3" />
      </button>

      <button
        onClick={(e) => {
          e.stopPropagation()
          onExpand()
        }}
        className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-[#30363D] text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9] transition-colors"
        title="Expand pane"
      >
        <Maximize2 className="w-3 h-3" />
      </button>

      <button
        onClick={(e) => {
          e.stopPropagation()
          onHide()
        }}
        className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-[#30363D] text-gray-400 dark:text-[#6E7681] hover:text-red-500 dark:hover:text-[#FF7B72] transition-colors"
        title="Hide pane"
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )
}

// --- Compact Header ---

function CompactHeader({
  session,
  name,
  cost,
  ctxPct,
  isPinned,
  onExpand,
  onClick,
}: {
  session: LiveSession
  name: string
  cost: string
  ctxPct: number
  isPinned: boolean
  onExpand: () => void
  onClick: (e: React.MouseEvent) => void
}) {
  return (
    <div
      className="flex items-center gap-1.5 px-2 py-1 bg-gray-50 dark:bg-[#161B22] border-b border-gray-200 dark:border-[#21262D] cursor-pointer select-none"
      onClick={onClick}
    >
      {/* Project name (shorter truncation) */}
      <span className="text-[10px] font-medium text-gray-700 dark:text-[#C9D1D9] truncate max-w-[14ch]">
        {name}
      </span>

      {/* Cost */}
      <span className="text-[10px] font-mono text-gray-500 dark:text-[#8B949E] tabular-nums flex-shrink-0">
        {cost}
      </span>

      {/* Context % */}
      <span className={cn('text-[10px] font-mono tabular-nums flex-shrink-0', contextColor(ctxPct))}>
        {ctxPct}%
      </span>

      {/* Turn count */}
      <span className="text-[10px] font-mono text-gray-400 dark:text-[#6E7681] tabular-nums flex-shrink-0">
        T{session.turnCount}
      </span>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Pin indicator */}
      {isPinned && <Pin className="w-2.5 h-2.5 text-blue-500 dark:text-[#79C0FF] flex-shrink-0" />}

      {/* Expand only in compact mode */}
      <button
        onClick={(e) => {
          e.stopPropagation()
          onExpand()
        }}
        className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-[#30363D] text-gray-400 dark:text-[#6E7681] hover:text-gray-600 dark:hover:text-[#C9D1D9] transition-colors"
        title="Expand pane"
      >
        <Maximize2 className="w-2.5 h-2.5" />
      </button>
    </div>
  )
}

// --- Footer ---

function Footer({ session, onExpand }: { session: LiveSession; onExpand?: () => void }) {
  const activity = session.currentActivity || (session.lastUserMessage ? cleanPreviewText(session.lastUserMessage) : '') || ''
  const truncatedActivity = activity.length > 40 ? activity.slice(0, 37) + '...' : activity

  return (
    <div className="flex items-center gap-2 px-3 py-1 bg-gray-50 dark:bg-[#161B22] border-t border-gray-200 dark:border-[#21262D] text-[10px] text-gray-400 dark:text-[#6E7681]">
      {/* Current activity */}
      <span className="truncate flex-1 min-w-0" title={activity}>
        {truncatedActivity || 'Idle'}
      </span>

      {/* Turn count */}
      <span className="font-mono tabular-nums flex-shrink-0">
        Turn {session.turnCount}
      </span>

      {/* Sub-agent pills */}
      {session.subAgents && session.subAgents.length > 0 && (
        <SubAgentPills
          subAgents={session.subAgents}
          onExpand={onExpand}
        />
      )}
    </div>
  )
}
