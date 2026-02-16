import type { LiveSession } from './use-live-sessions'
import type { AgentStateGroup } from './types'
import { SessionCard } from './SessionCard'
import { cn } from '../../lib/utils'

interface KanbanColumnProps {
  title: string
  group: AgentStateGroup
  sessions: LiveSession[]
  accentColor: string
  selectedId: string | null
  onSelect: (id: string) => void
  emptyMessage: string
  stalledSessions?: Set<string>
  currentTime: number
}

function NeedsYouSubCount({ sessions }: { sessions: LiveSession[] }) {
  const urgent = sessions.filter(
    (s) => s.agentState.state === 'awaiting_input' || s.agentState.state === 'needs_permission'
  ).length
  if (urgent === 0) return null
  return (
    <span className="ml-1.5 text-[10px] text-amber-500 font-normal">
      {urgent} urgent
    </span>
  )
}

export function KanbanColumn({
  title,
  group,
  sessions,
  accentColor,
  selectedId,
  onSelect,
  emptyMessage,
  stalledSessions,
  currentTime,
}: KanbanColumnProps) {
  return (
    <div className="flex flex-col min-w-[280px] w-[320px] xl:flex-1">
      <div className={cn(
        'bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800 flex flex-col',
        'overflow-hidden'
      )}>
        <div className={cn('h-0.5', accentColor)} />
        <div className="px-3 py-2 flex items-center justify-between">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {title}
            {group === 'needs_you' && <NeedsYouSubCount sessions={sessions} />}
          </span>
          <span className="text-xs text-gray-400 dark:text-gray-500">({sessions.length})</span>
        </div>
        <div className="space-y-3 p-3 max-h-[calc(100vh-220px)] overflow-y-auto">
          {sessions.length === 0 ? (
            <p className="text-xs text-gray-400 dark:text-gray-500 py-8 text-center">
              {emptyMessage}
            </p>
          ) : (
            sessions.map((session) => (
              <div
                key={session.id}
                data-session-id={session.id}
                onClick={() => onSelect(session.id)}
                className={cn(
                  'cursor-pointer rounded-lg transition-opacity',
                  session.id === selectedId && 'ring-2 ring-indigo-500 rounded-lg',
                  group === 'needs_you' && session.cacheStatus !== 'warm' && 'opacity-70'
                )}
              >
                <SessionCard session={session} stalledSessions={stalledSessions} currentTime={currentTime} />
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
