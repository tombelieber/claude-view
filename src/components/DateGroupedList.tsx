import { Link } from 'react-router-dom'
import { groupSessionsByDate } from '../lib/date-groups'
import { SessionCard } from './SessionCard'
import type { SessionInfo } from '../hooks/use-projects'

interface DateGroupedListProps {
  sessions: SessionInfo[]
  showProjectBadge?: boolean
}

export function DateGroupedList({ sessions, showProjectBadge }: DateGroupedListProps) {
  const groups = groupSessionsByDate(sessions)

  return (
    <div>
      {groups.map((group, i) => (
        <div key={group.label}>
          {/* Sticky date header */}
          <div className="sticky top-0 z-10 bg-gray-50/95 backdrop-blur-sm py-2.5 flex items-center gap-3">
            <span className="text-[13px] font-semibold text-gray-700 tracking-tight whitespace-nowrap">
              {group.label}
            </span>
            <div className="flex-1 h-px bg-gray-200" />
            <span className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap">
              {group.sessions.length} {group.sessions.length === 1 ? 'session' : 'sessions'}
            </span>
          </div>

          {/* Session cards */}
          <div className="space-y-2 pb-4">
            {group.sessions.map((session) => (
              <Link
                key={session.id}
                to={`/session/${encodeURIComponent(session.project)}/${session.id}`}
                className="block"
              >
                <SessionCard
                  session={session}
                  isSelected={false}
                  onClick={() => {}}
                  projectDisplayName={showProjectBadge ? session.project : undefined}
                />
              </Link>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
