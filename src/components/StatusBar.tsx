import { RefreshCw, GitCommitHorizontal } from 'lucide-react'
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

  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Use lastGitSyncAt for the "last updated" display (more recent than lastIndexedAt)
  const lastSyncTs = status?.lastGitSyncAt ?? status?.lastIndexedAt ?? null
  const lastUpdatedText = lastSyncTs ? formatRelativeTime(lastSyncTs) : null

  // Git sync stats
  const commitsFound = status?.commitsFound ? Number(status.commitsFound) : 0
  const linksCreated = status?.linksCreated ? Number(status.linksCreated) : 0

  const isSpinning = isStatusLoading || isSyncing

  const handleRefresh = async () => {
    if (isSpinning) return

    const started = await triggerSync()
    if (started) {
      // Sync accepted (202) — poll for completion then refresh data queries
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
        ) : lastUpdatedText ? (
          <>
            <span>Last update: {lastUpdatedText}</span>
            <span aria-hidden="true">&middot;</span>
            <span>{sessionsIndexed.toLocaleString()} sessions</span>
            {commitsFound > 0 && (
              <>
                <span aria-hidden="true">&middot;</span>
                <span className="inline-flex items-center gap-0.5" title={`${linksCreated} session-commit links`}>
                  <GitCommitHorizontal className="w-3 h-3" aria-hidden="true" />
                  {commitsFound.toLocaleString()}
                </span>
              </>
            )}
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
