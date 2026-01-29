import { RefreshCw } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useStatus, formatRelativeTime } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useQueryClient } from '@tanstack/react-query'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const { data: status, isLoading: isStatusLoading } = useStatus()
  const { triggerSync, isLoading: isSyncing } = useGitSync()
  const queryClient = useQueryClient()
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)

  // Format sessions count from status (index metadata) or fallback to project count
  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Format last synced time — show "Not yet synced" when null instead of hiding
  const lastSyncedText = status?.lastIndexedAt
    ? formatRelativeTime(status.lastIndexedAt)
    : null

  const isSpinning = isStatusLoading || isSyncing

  const handleRefresh = async () => {
    if (isSpinning) return

    const started = await triggerSync()
    if (started) {
      // Sync was accepted (202) — the background task is running.
      // Poll status after a delay to allow sync to complete, then refresh data queries.
      setTimeout(() => {
        queryClient.invalidateQueries({ queryKey: ['status'] })
        queryClient.invalidateQueries({ queryKey: ['dashboard-stats'] })
        queryClient.invalidateQueries({ queryKey: ['projects'] })
      }, 2000)
    }
  }

  return (
    <footer
      className="h-8 bg-white border-t border-gray-200 px-4 flex items-center justify-between text-xs text-gray-500"
      role="contentinfo"
      aria-label="Data freshness status"
    >
      <div className="flex items-center gap-1.5">
        {isStatusLoading ? (
          <span className="animate-pulse">Loading status...</span>
        ) : isSyncing ? (
          <span className="animate-pulse">Syncing...</span>
        ) : lastSyncedText ? (
          <>
            <span>Last synced: {lastSyncedText}</span>
            <span aria-hidden="true">&middot;</span>
            <span aria-label={`${sessionsIndexed} sessions indexed`}>
              {sessionsIndexed.toLocaleString()} sessions
            </span>
          </>
        ) : (
          <span>Not yet synced &middot; {projects.length} projects &middot; {totalSessions} sessions</span>
        )}
      </div>

      <button
        type="button"
        onClick={handleRefresh}
        disabled={isSpinning}
        className="p-1 -mr-1 rounded hover:bg-gray-100 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 disabled:opacity-50 cursor-pointer"
        aria-label={isSyncing ? 'Sync in progress' : 'Trigger sync'}
        title={isSyncing ? 'Sync in progress...' : 'Sync data'}
      >
        <RefreshCw
          className={`w-3.5 h-3.5 ${isSpinning ? 'animate-spin' : ''}`}
          aria-hidden="true"
        />
      </button>
    </footer>
  )
}
