import { useState, useRef, useCallback, useEffect } from 'react'
import { RefreshCw, GitCommitHorizontal } from 'lucide-react'
import { toast } from 'sonner'
import type { ProjectSummary } from '../hooks/use-projects'
import { useStatus, formatRelativeTime, useTick } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useGitSyncProgress } from '../hooks/use-git-sync-progress'
import { useQueryClient } from '@tanstack/react-query'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const { data: status, isLoading: isStatusLoading } = useStatus()
  const { triggerSync, isLoading: isSyncing, status: syncStatus, reset: resetSync } = useGitSync()
  const queryClient = useQueryClient()
  useTick(30_000) // Force re-render every 30s to keep relative time fresh
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)

  // SSE progress state
  const [sseEnabled, setSseEnabled] = useState(false)
  const progress = useGitSyncProgress(sseEnabled)

  // Guard against React strict-mode double-firing of the terminal-state effect.
  // Reset to false when sseEnabled transitions to true (new sync started).
  const doneHandledRef = useRef(false)

  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Use lastGitSyncAt for the "last updated" display (more recent than lastIndexedAt)
  const lastSyncTs = status?.lastGitSyncAt ?? status?.lastIndexedAt ?? null
  const lastUpdatedText = lastSyncTs ? formatRelativeTime(lastSyncTs) : null

  // Git sync stats
  const commitsFound = status?.commitsFound ? Number(status.commitsFound) : 0
  const linksCreated = status?.linksCreated ? Number(status.linksCreated) : 0

  // Derive syncing state from SSE phase
  const isSseActive = sseEnabled && progress.phase !== 'idle' && progress.phase !== 'done' && progress.phase !== 'error'
  const isSpinning = isStatusLoading || isSyncing || isSseActive

  // Handle retry from error toast
  const handleRetry = useCallback(async () => {
    resetSync()
    doneHandledRef.current = false
    const started = await triggerSync()
    if (started) setSseEnabled(true)
  }, [triggerSync, resetSync])

  const handleRefresh = async () => {
    if (isSpinning) return
    doneHandledRef.current = false
    const started = await triggerSync()
    if (started) setSseEnabled(true)
  }

  // React to SSE terminal states
  useEffect(() => {
    if (progress.phase === 'done' && !doneHandledRef.current) {
      doneHandledRef.current = true
      toast.success('Sync completed', {
        description: `${progress.reposScanned} repos | ${progress.commitsFound} commits | ${progress.linksCreated} links`,
      })
      queryClient.invalidateQueries({ queryKey: ['status'] })
      queryClient.invalidateQueries({ queryKey: ['dashboard-stats'] })
      queryClient.invalidateQueries({ queryKey: ['projects'] })
      setSseEnabled(false)
      resetSync()
    } else if (progress.phase === 'error' && !doneHandledRef.current) {
      doneHandledRef.current = true
      toast.error('Sync failed', {
        description: progress.errorMessage ?? 'Unknown error',
        duration: 6000,
        action: {
          label: 'Retry',
          onClick: handleRetry,
        },
      })
      setSseEnabled(false)
    }
  }, [progress.phase, progress.reposScanned, progress.commitsFound, progress.linksCreated, progress.errorMessage, queryClient, resetSync, handleRetry])

  // Handle 409 conflict from the HTTP sync trigger
  useEffect(() => {
    if (syncStatus === 'conflict') {
      toast.info('Sync already in progress', {
        description: 'Please wait for the current sync to complete.',
        duration: 3000,
      })
    }
  }, [syncStatus])

  return (
    <footer
      className="h-8 bg-white dark:bg-gray-900 border-t border-gray-200 dark:border-gray-700 px-4 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400"
      role="contentinfo"
      aria-label="Data freshness status"
    >
      <div className="flex items-center gap-1.5">
        {isStatusLoading ? (
          <span className="animate-pulse">Loading status...</span>
        ) : isSseActive ? (
          <span className="animate-pulse text-xs">
            {progress.phase === 'scanning'
              ? progress.totalRepos > 0
                ? `Scanning repo ${progress.reposScanned}/${progress.totalRepos}...`
                : 'Scanning repos...'
              : progress.phase === 'correlating'
                ? progress.totalCorrelatableSessions > 0
                  ? `Linking sessions ${progress.sessionsCorrelated}/${progress.totalCorrelatableSessions}... (${progress.linksCreated} links)`
                  : `Linking commits... (${progress.linksCreated} links)`
                : 'Starting sync...'}
          </span>
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
        className="inline-flex items-center gap-1.5 px-2.5 py-1 -mr-1 rounded text-xs font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
        aria-label={isSyncing || isSseActive ? 'Sync in progress' : 'Sync now'}
        data-testid="sync-button"
      >
        <RefreshCw
          className={`w-3.5 h-3.5 ${isSpinning ? 'animate-spin' : ''}`}
          aria-hidden="true"
        />
        {isSyncing || isSseActive ? 'Syncing...' : 'Sync Now'}
      </button>
    </footer>
  )
}
