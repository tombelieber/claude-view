import { ArrowRight, Clock, FolderOpen, GitBranch } from 'lucide-react'
import { useMemo } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import type { DayActivity } from '../../lib/activity-utils'
import { projectDisplayName, sessionStartTime } from '../../lib/activity-utils'
import { formatHumanDuration } from '../../lib/format-utils'
import { buildSessionUrl } from '../../lib/url-utils'
import { cn } from '../../lib/utils'
import type { SessionInfo } from '../../types/generated/SessionInfo'

/** Extract a short branch/worktree label for display. Returns null for main/master. */
function branchLabel(session: SessionInfo): string | null {
  const branch = session.gitBranch
  if (!branch || branch === 'main' || branch === 'master' || branch === 'HEAD') return null
  // Strip common prefixes for compact display
  return branch
    .replace(/^feature\//, '')
    .replace(/^feat\//, '')
    .replace(/^fix\//, 'fix/')
    .replace(/^worktree-/, '')
}

function formatTime(unixSeconds: number): string {
  if (unixSeconds <= 0) return '--'
  return new Date(unixSeconds * 1000).toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

function formatDayHeader(dateStr: string): string {
  const date = new Date(dateStr + 'T12:00:00')
  const today = new Date()
  const yesterday = new Date()
  yesterday.setDate(yesterday.getDate() - 1)

  const isToday = date.toDateString() === today.toDateString()
  const isYesterday = date.toDateString() === yesterday.toDateString()

  const dayName = date.toLocaleDateString('en-US', {
    weekday: 'long',
    month: 'short',
    day: 'numeric',
  })
  if (isToday) return `Today — ${dayName}`
  if (isYesterday) return `Yesterday — ${dayName}`
  return dayName
}

interface DailyTimelineProps {
  days: DayActivity[]
  selectedDate?: string | null
  selectedProject?: string | null
  /** Display cap. When omitted, shows all days (data is already time-bounded by the API). */
  maxDays?: number
}

export function DailyTimeline({
  days,
  selectedDate,
  selectedProject,
  maxDays,
}: DailyTimelineProps) {
  const [searchParams] = useSearchParams()

  const filteredDays = useMemo(() => {
    let result = days

    // Filter by selected date
    if (selectedDate) {
      result = result.filter((d) => d.date === selectedDate)
    }

    // Filter sessions within days by project
    if (selectedProject) {
      result = result
        .map((day) => {
          const filtered = day.sessions.filter(
            (s) => ((s.gitRoot || null) ?? s.projectPath ?? s.project) === selectedProject,
          )
          return {
            ...day,
            sessions: filtered,
            totalSeconds: filtered.reduce((sum, s) => sum + s.durationSeconds, 0),
            sessionCount: filtered.length,
          }
        })
        .filter((day) => day.sessions.length > 0)
    }

    return maxDays != null ? result.slice(0, maxDays) : result
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    days.map((d) => `${d.date}:${d.sessionCount}`).join(','),
    selectedDate,
    selectedProject,
    maxDays,
  ])

  if (filteredDays.length === 0) {
    return (
      <div className="text-center py-8 text-sm text-gray-400 dark:text-gray-500">
        No sessions for this period
      </div>
    )
  }

  return (
    <div>
      <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-200 mb-3">
        Session Timeline
      </h2>
      <div className="space-y-4">
        {filteredDays.map((day) => (
          <div key={day.date}>
            {/* Day header */}
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {formatDayHeader(day.date)}
              </h3>
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {day.sessionCount} {day.sessionCount === 1 ? 'session' : 'sessions'} —{' '}
                {formatHumanDuration(day.totalSeconds)}
              </span>
            </div>

            {/* Session rows */}
            <div className="space-y-1 ml-2 border-l-2 border-gray-200 dark:border-gray-800 pl-3">
              {day.sessions.map((session: SessionInfo) => {
                const start = sessionStartTime(session)
                const end = session.modifiedAt
                const title = session.summary || session.preview || '(untitled)'
                const truncatedTitle = title.length > 60 ? title.slice(0, 57) + '...' : title

                return (
                  <Link
                    key={session.id}
                    to={buildSessionUrl(session.id, searchParams)}
                    aria-label={`${title}, ${formatHumanDuration(session.durationSeconds)}, ${projectDisplayName((session.gitRoot || null) ?? session.projectPath ?? session.project)}`}
                    className={cn(
                      'flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors duration-150',
                      'hover:bg-gray-100 dark:hover:bg-gray-900 cursor-pointer group',
                    )}
                  >
                    {/* Time range */}
                    <span className="text-xs text-gray-400 dark:text-gray-500 font-mono whitespace-nowrap min-w-[110px]">
                      {formatTime(start)} — {formatTime(end)}
                    </span>

                    {/* Project badge + branch tag */}
                    <span className="flex items-center gap-1 shrink-0">
                      <span className="inline-flex items-center gap-1 text-xs text-gray-700 dark:text-gray-300 px-1.5 py-0.5 rounded whitespace-nowrap">
                        <FolderOpen className="w-3 h-3 text-amber-500 dark:text-amber-400 shrink-0" />
                        {projectDisplayName(
                          (session.gitRoot || null) ?? session.projectPath ?? session.project,
                        )}
                      </span>
                      {branchLabel(session) && (
                        <span className="inline-flex items-center gap-0.5 text-xs font-mono bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 px-1.5 py-0.5 rounded whitespace-nowrap max-w-35">
                          <GitBranch className="w-3 h-3 shrink-0" />
                          <span className="truncate">{branchLabel(session)}</span>
                        </span>
                      )}
                    </span>

                    {/* Title */}
                    <span className="flex-1 text-gray-700 dark:text-gray-300 truncate text-xs">
                      {truncatedTitle}
                    </span>

                    {/* Duration */}
                    <span className="text-xs font-medium text-gray-500 dark:text-gray-400 whitespace-nowrap flex items-center gap-1">
                      <Clock className="w-3 h-3" />
                      {formatHumanDuration(session.durationSeconds)}
                    </span>

                    {/* Arrow */}
                    <ArrowRight className="w-3 h-3 text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100 transition-opacity" />
                  </Link>
                )
              })}
            </div>
          </div>
        ))}
      </div>
      {!selectedDate && maxDays != null && days.length > maxDays && (
        <p className="text-xs text-gray-400 dark:text-gray-500 text-center pt-2">
          Showing last {maxDays} days — click a date on the calendar to filter
        </p>
      )}
    </div>
  )
}
