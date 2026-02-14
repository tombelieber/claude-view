import type { LiveSession } from '../../hooks/use-live-sessions'
import type { LiveDisplayStatus } from '../../types/live'
import { SessionCard } from '../../components/live/SessionCard'
import { cn } from '../../lib/utils'

interface KanbanColumnProps {
  title: string
  status: LiveDisplayStatus
  sessions: LiveSession[]
  accentColor: string
  selectedId: string | null
  onSelect: (id: string) => void
  emptyMessage: string
}

export function KanbanColumn({
  title,
  status,
  sessions,
  accentColor,
  selectedId,
  onSelect,
  emptyMessage,
}: KanbanColumnProps) {
  const sorted = [...sessions].sort(
    (a, b) => b.lastActivityAt - a.lastActivityAt
  )

  return (
    <div className="flex flex-col min-w-[280px] w-[320px] xl:flex-1">
      <div
        className={cn(
          'bg-slate-900/50 rounded-lg border border-slate-800 flex flex-col',
          'overflow-hidden'
        )}
      >
        {/* Accent border */}
        <div className={cn('h-0.5', accentColor)} />

        {/* Header */}
        <div className="px-3 py-2 flex items-center justify-between">
          <span className="text-sm font-medium text-slate-300">{title}</span>
          <span className="text-xs text-slate-500">({sessions.length})</span>
        </div>

        {/* Cards */}
        <div className="space-y-3 p-3 max-h-[calc(100vh-220px)] overflow-y-auto">
          {sorted.length === 0 ? (
            <p className="text-xs text-slate-500 py-8 text-center">
              {emptyMessage}
            </p>
          ) : (
            sorted.map((session) => (
              <div
                key={session.id}
                data-session-id={session.id}
                onClick={() => onSelect(session.id)}
                className={cn(
                  'cursor-pointer rounded-lg',
                  session.id === selectedId && 'ring-2 ring-indigo-500 rounded-lg'
                )}
              >
                <SessionCard session={session} />
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
