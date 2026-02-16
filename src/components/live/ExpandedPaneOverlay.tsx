import { useEffect, useCallback, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import {
  Loader2,
  Pause,
  Check,
  Minimize2,
  X,
  GitBranch,
} from 'lucide-react'
import type { LiveSession } from './use-live-sessions'
import { cn } from '../../lib/utils'

// --- Helpers (shared logic with MonitorPane) ---

function projectName(session: LiveSession): string {
  if (session.projectDisplayName) return session.projectDisplayName
  if (session.projectPath) {
    const segments = session.projectPath.split('/')
    return segments[segments.length - 1] || session.project
  }
  return session.project
}

function formatCost(totalUsd: number): string {
  return `$${totalUsd.toFixed(2)}`
}

function contextPercent(session: LiveSession): number {
  const limit = 200_000
  return Math.min(Math.round((session.contextWindowTokens / limit) * 100), 100)
}

function contextColor(pct: number): string {
  if (pct > 80) return 'text-red-400'
  if (pct >= 50) return 'text-amber-400'
  return 'text-green-400'
}

function statusDotColor(status: LiveSession['status']): string {
  switch (status) {
    case 'working':
      return 'bg-green-500'
    case 'paused':
      return 'bg-amber-500'
    case 'done':
    default:
      return 'bg-zinc-500'
  }
}

function StatusIcon({ status, className }: { status: LiveSession['status']; className?: string }) {
  switch (status) {
    case 'working':
      return <Loader2 className={cn('animate-spin', className)} />
    case 'paused':
      return <Pause className={className} />
    case 'done':
      return <Check className={className} />
    default:
      return <Pause className={className} />
  }
}

// --- Props ---

export interface ExpandedPaneOverlayProps {
  session: LiveSession
  mode: 'raw' | 'rich'
  onClose: () => void
  children: ReactNode
}

// --- Component ---

export function ExpandedPaneOverlay({
  session,
  mode,
  onClose,
  children,
}: ExpandedPaneOverlayProps) {
  // ESC key handler — declared before any conditional returns
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
      }
    },
    [onClose]
  )

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  // Prevent body scroll while overlay is open
  useEffect(() => {
    const prev = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = prev
    }
  }, [])

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      // Only close if clicking the backdrop itself, not the pane
      if (e.target === e.currentTarget) {
        onClose()
      }
    },
    [onClose]
  )

  const name = projectName(session)
  const ctxPct = contextPercent(session)
  const cost = formatCost(session.cost.totalUsd)
  const activity = session.currentActivity || session.lastUserMessage || ''
  const truncatedActivity = activity.length > 60 ? activity.slice(0, 57) + '...' : activity

  const overlay = (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80"
      onClick={handleBackdropClick}
    >
      <div
        className="flex flex-col rounded-lg border border-gray-700 bg-gray-950 overflow-hidden shadow-2xl"
        style={{ width: '95vw', height: '90vh' }}
      >
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-2 bg-gray-900 border-b border-gray-800 select-none">
          {/* Status dot */}
          <span
            className={cn(
              'inline-block h-2.5 w-2.5 rounded-full flex-shrink-0',
              statusDotColor(session.status),
              session.status === 'working' && 'animate-pulse'
            )}
            title={session.status}
          />

          {/* Project name */}
          <span
            className="text-sm font-medium text-gray-200 truncate max-w-[30ch]"
            title={session.projectPath || name}
          >
            {name}
          </span>

          {/* Branch */}
          {session.gitBranch && (
            <span className="inline-flex items-center gap-0.5 text-xs font-mono text-gray-500 truncate max-w-[20ch]">
              <GitBranch className="w-3 h-3 flex-shrink-0" />
              <span className="truncate" title={session.gitBranch}>
                {session.gitBranch}
              </span>
            </span>
          )}

          {/* Spacer */}
          <div className="flex-1" />

          {/* Metrics */}
          <span className="text-xs font-mono text-gray-400 tabular-nums flex-shrink-0">
            {cost}
          </span>
          <span className={cn('text-xs font-mono tabular-nums flex-shrink-0', contextColor(ctxPct))}>
            {ctxPct}% ctx
          </span>

          {/* Status icon */}
          <StatusIcon status={session.status} className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />

          {/* Mode label */}
          <span className="text-[10px] text-gray-600 flex-shrink-0 uppercase tracking-wide">
            {mode}
          </span>

          {/* Divider */}
          <div className="w-px h-4 bg-gray-700" />

          {/* Turn count */}
          <span className="text-xs font-mono text-gray-500 tabular-nums flex-shrink-0">
            Turn {session.turnCount}
          </span>

          {/* Activity */}
          {truncatedActivity && (
            <>
              <div className="w-px h-4 bg-gray-700" />
              <span className="text-xs text-gray-500 truncate max-w-[40ch]" title={activity}>
                {truncatedActivity}
              </span>
            </>
          )}

          {/* Divider */}
          <div className="w-px h-4 bg-gray-700" />

          {/* Minimize button */}
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-gray-800 text-gray-400 hover:text-gray-200 transition-colors"
            title="Minimize (Esc)"
          >
            <Minimize2 className="w-4 h-4" />
          </button>

          {/* Close button */}
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-gray-800 text-gray-400 hover:text-red-400 transition-colors"
            title="Close (Esc)"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content area */}
        <div className="flex-1 min-h-0 overflow-hidden">
          {children}
        </div>
      </div>
    </div>
  )

  return createPortal(overlay, document.body)
}
