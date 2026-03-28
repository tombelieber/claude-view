import { Check } from 'lucide-react'
import { useMemo } from 'react'
import { cn } from '../../lib/utils'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

interface HarnessNeedsYouSidebarProps {
  sessions: LiveSession[]
  collapsed: boolean
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}

export function HarnessNeedsYouSidebar({
  sessions,
  collapsed,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: HarnessNeedsYouSidebarProps) {
  if (collapsed) {
    return (
      <div className="h-full flex items-start justify-center pt-4">
        <div className="w-8 h-8 rounded-full bg-green-100 dark:bg-green-900/30 flex items-center justify-center">
          <Check className="w-4 h-4 text-green-600 dark:text-green-400" />
        </div>
      </div>
    )
  }

  return (
    <ExpandedSidebar
      sessions={sessions}
      selectedId={selectedId}
      onSelect={onSelect}
      stalledSessions={stalledSessions}
      currentTime={currentTime}
      onCardClick={onCardClick}
    />
  )
}

function ExpandedSidebar({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: Omit<HarnessNeedsYouSidebarProps, 'collapsed'>) {
  const urgentCount = useMemo(
    () =>
      sessions.filter(
        (s) => s.agentState.state === 'awaiting_input' || s.agentState.state === 'needs_permission',
      ).length,
    [sessions],
  )

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Header — amber stripe matching Board view */}
      <div className="shrink-0 mb-3">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-amber-500" />
          <div className="px-3 py-2 flex items-center gap-1.5">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Needs You</span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({sessions.length})</span>
            {urgentCount > 0 && (
              <span className="ml-1.5 text-xs text-amber-500 font-normal">
                {urgentCount} urgent
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Scrollable card list */}
      <div className="flex-1 min-h-0 overflow-y-auto space-y-2">
        {sessions.map((session) => (
          <div
            key={session.id}
            role="button"
            tabIndex={0}
            data-session-id={session.id}
            onClick={() => onSelect(session.id)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                onSelect(session.id)
              }
            }}
            className={cn(
              'cursor-pointer rounded-lg transition-opacity',
              session.id === selectedId && 'ring-2 ring-indigo-500 rounded-lg',
              session.cacheStatus !== 'warm' && 'opacity-70',
            )}
          >
            <SessionCard
              session={session}
              stalledSessions={stalledSessions}
              currentTime={currentTime}
              onClickOverride={onCardClick ? () => onCardClick(session.id) : undefined}
              showStateBadge
              hideProjectBranch={false}
            />
          </div>
        ))}
      </div>
    </div>
  )
}
