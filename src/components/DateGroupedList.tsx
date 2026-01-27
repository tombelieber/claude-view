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
    <div className="space-y-2">
      {groups.map((group) => (
        <div key={group.label}>
          {/* Sticky date header */}
          <div className="sticky top-0 bg-white/95 backdrop-blur-sm z-10 py-3 flex items-center">
            <span className="font-medium text-gray-900 text-sm">
              {group.label}
            </span>
            <div className="flex-1 border-b border-gray-200 mx-3" />
            <span className="text-gray-400 tabular-nums text-xs">
              {group.sessions.length}
            </span>
          </div>

          {/* Session cards */}
          <div className="space-y-2">
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
