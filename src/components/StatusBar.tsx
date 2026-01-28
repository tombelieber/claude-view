import { RefreshCw } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useStatus, formatRelativeTime } from '../hooks/use-status'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const { data: status, isLoading: isStatusLoading, refetch } = useStatus()
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)

  // Format sessions count from status (index metadata) or fallback to project count
  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Format last synced time
  const lastSyncedText = status?.lastIndexedAt
    ? formatRelativeTime(status.lastIndexedAt)
    : null

  return (
    <footer
      className="h-8 bg-white border-t border-gray-200 px-4 flex items-center justify-between text-xs text-gray-500"
      role="contentinfo"
      aria-label="Data freshness status"
    >
      <div className="flex items-center gap-1.5">
        {isStatusLoading ? (
          <span className="animate-pulse">Loading status...</span>
        ) : lastSyncedText ? (
          <>
            <span>Last synced: {lastSyncedText}</span>
            <span aria-hidden="true">·</span>
            <span aria-label={`${sessionsIndexed} sessions indexed`}>
              {sessionsIndexed.toLocaleString()} sessions
            </span>
          </>
        ) : (
          <span>{projects.length} projects · {totalSessions} sessions</span>
        )}
      </div>

      <button
        type="button"
        onClick={() => refetch()}
        disabled={isStatusLoading}
        className="p-1 -mr-1 rounded hover:bg-gray-100 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 disabled:opacity-50 cursor-pointer"
        aria-label="Refresh status"
        title="Refresh status"
      >
        <RefreshCw
          className={`w-3.5 h-3.5 ${isStatusLoading ? 'animate-spin' : ''}`}
          aria-hidden="true"
        />
      </button>
    </footer>
  )
}
