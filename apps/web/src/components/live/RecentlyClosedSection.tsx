import { ChevronDown, ChevronRight, Copy, X } from 'lucide-react'
import { useCallback, useState } from 'react'
import { formatRelativeTime } from '../../lib/format-utils'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

const COLLAPSE_KEY = 'claude-view:recently-closed-collapsed'

interface RecentlyClosedSectionProps {
  sessions: LiveSession[]
  onDismiss: (sessionId: string) => void
  onDismissAll: () => void
  onSelect: (sessionId: string) => void
  currentTime: number
}

export function RecentlyClosedSection({
  sessions,
  onDismiss,
  onDismissAll,
  onSelect,
  currentTime,
}: RecentlyClosedSectionProps) {
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(COLLAPSE_KEY) === 'true'
    } catch {
      return false
    }
  })

  const toggleCollapse = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev
      try {
        localStorage.setItem(COLLAPSE_KEY, String(next))
      } catch {
        // localStorage unavailable (private browsing, quota exceeded)
      }
      return next
    })
  }, [])

  if (sessions.length === 0) return null

  return (
    <div className="mt-6">
      {/* Header */}
      <div className="flex items-center justify-between px-2 py-1.5 rounded-md bg-zinc-100 dark:bg-zinc-800/50">
        <button
          type="button"
          onClick={toggleCollapse}
          className="flex items-center gap-1.5 text-sm font-medium text-zinc-500 dark:text-zinc-400 cursor-pointer hover:text-zinc-700 dark:hover:text-zinc-300 transition-colors"
        >
          {collapsed ? <ChevronRight className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          Recently Closed ({sessions.length})
        </button>
        <button
          type="button"
          onClick={onDismissAll}
          className="text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 cursor-pointer transition-colors"
        >
          Dismiss All
        </button>
      </div>

      {/* Cards */}
      {!collapsed && (
        <div className="mt-2 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
          {sessions.map((session) => (
            <div key={session.id} className="relative group opacity-60">
              {/* Dismiss button */}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  onDismiss(session.id)
                }}
                className="absolute top-2 right-2 z-10 p-0.5 rounded hover:bg-zinc-200 dark:hover:bg-zinc-700 opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                title="Dismiss"
              >
                <X className="w-3.5 h-3.5 text-zinc-400" />
              </button>
              {/* Resume button */}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  navigator.clipboard.writeText(`claude --resume ${session.id}`).catch(() => {})
                }}
                className="absolute bottom-2 left-2 z-10 flex items-center gap-1 px-1.5 py-0.5 text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 bg-zinc-100 dark:bg-zinc-800 rounded border border-zinc-200 dark:border-zinc-700 opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                title="Copy resume command"
              >
                <Copy className="w-3 h-3" />
                Resume
              </button>
              {/* Closed-time label */}
              {session.closedAt && (
                <div className="absolute bottom-2 right-2 z-10 text-xs text-zinc-400">
                  closed {formatRelativeTime(session.closedAt)}
                </div>
              )}
              <div
                className="border border-dashed border-zinc-300 dark:border-zinc-700 rounded-lg cursor-pointer"
                onClick={() => onSelect(session.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') onSelect(session.id)
                }}
                role="button"
                tabIndex={0}
              >
                <SessionCard
                  session={session}
                  currentTime={currentTime}
                  onClickOverride={() => onSelect(session.id)}
                />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
