import { useRef, useCallback, useEffect } from 'react'
import { RefreshCw, GitCommitHorizontal } from 'lucide-react'
import { toast } from 'sonner'
import type { ProjectSummary } from '../hooks/use-projects'
import { useStatus, formatRelativeTime, useTick } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useQueryClient } from '@tanstack/react-query'

interface StatusBarProps {
  projects: ProjectSummary[]
}

export function StatusBar({ projects }: StatusBarProps) {
  const { data: status, isLoading: isStatusLoading } = useStatus()
  const { triggerSync, isLoading: isSyncing, status: syncStatus, error: syncError, reset: resetSync } = useGitSync()
  const queryClient = useQueryClient()
  useTick(30_000) // Force re-render every 30s to keep relative time fresh
  const totalSessions = projects.reduce((sum, p) => sum + p.sessionCount, 0)

  // Track previous status values to calculate deltas
  const prevStatusRef = useRef<typeof status | null>(null)
  // Track last shown toast to prevent duplicates
  const lastToastRef = useRef<{ type: string; error?: string } | null>(null)
  // Track component mount state to prevent state updates after unmount
  const mountedRef = useRef(true)
  useEffect(() => () => { mountedRef.current = false }, [])

  const sessionsIndexed = status?.sessionsIndexed ? Number(status.sessionsIndexed) : totalSessions

  // Use lastGitSyncAt for the "last updated" display (more recent than lastIndexedAt)
  const lastSyncTs = status?.lastGitSyncAt ?? status?.lastIndexedAt ?? null
  const lastUpdatedText = lastSyncTs ? formatRelativeTime(lastSyncTs) : null

  // Git sync stats
  const commitsFound = status?.commitsFound ? Number(status.commitsFound) : 0
  const linksCreated = status?.linksCreated ? Number(status.linksCreated) : 0

  const isSpinning = isStatusLoading || isSyncing

  // Poll for sync completion and show toast with results
  const pollForCompletion = useCallback(async (prevSessions: number, prevCommits: number, prevLinks: number) => {
    const maxAttempts = 30 // 30 seconds max
    let attempts = 0

    const poll = async () => {
      if (!mountedRef.current) return

      attempts++
      if (attempts > maxAttempts) {
        toast.warning('Sync is taking longer than expected', {
          description: 'The sync is still running in the background.',
          duration: 4000,
        })
        return
      }

      // Fetch fresh status
      try {
        const response = await fetch('/api/status')
        if (!mountedRef.current) return

        if (!response.ok) {
          console.warn(`Sync status poll failed: HTTP ${response.status}`)
          setTimeout(poll, 1000)
          return
        }

        const newStatus = await response.json()
        if (!mountedRef.current) return

        const newSyncTs = newStatus.lastGitSyncAt ?? newStatus.lastIndexedAt

        // Check if sync has completed (timestamp changed)
        const oldSyncTs = prevStatusRef.current?.lastGitSyncAt ?? prevStatusRef.current?.lastIndexedAt
        if (newSyncTs && (!oldSyncTs || newSyncTs > oldSyncTs)) {
          // Sync completed - calculate deltas
          const newSessions = Number(newStatus.sessionsIndexed ?? 0)
          const newCommits = Number(newStatus.commitsFound ?? 0)
          const newLinks = Number(newStatus.linksCreated ?? 0)

          const sessionsDelta = newSessions - prevSessions
          const linksDelta = newLinks - prevLinks

          // Show success toast with stats
          const parts: string[] = []
          parts.push(`${newSessions.toLocaleString()} sessions`)
          if (sessionsDelta > 0) {
            parts.push(`+${sessionsDelta} new`)
          }
          if (newCommits > 0) {
            parts.push(`${newCommits.toLocaleString()} commits`)
          }
          if (linksDelta > 0) {
            parts.push(`+${linksDelta} links`)
          }

          toast.success('Sync completed', {
            description: parts.join(' | '),
            duration: 4000,
          })

          // Refresh data queries
          queryClient.invalidateQueries({ queryKey: ['status'] })
          queryClient.invalidateQueries({ queryKey: ['dashboard-stats'] })
          queryClient.invalidateQueries({ queryKey: ['projects'] })

          prevStatusRef.current = newStatus
          return
        }

        // Not completed yet, poll again
        setTimeout(poll, 1000)
      } catch (e) {
        console.warn('Sync status poll failed:', e)
        // Error polling, try again
        setTimeout(poll, 1000)
      }
    }

    poll()
  }, [queryClient])

  // Handle retry from error toast
  const handleRetry = useCallback(async () => {
    // Reset sync state first
    resetSync()
    lastToastRef.current = null

    // Store current values before retry
    const prevSessions = sessionsIndexed
    const prevCommits = commitsFound
    const prevLinks = linksCreated

    const started = await triggerSync()
    if (started) {
      pollForCompletion(prevSessions, prevCommits, prevLinks)
    }
  }, [triggerSync, pollForCompletion, sessionsIndexed, commitsFound, linksCreated, resetSync])

  const handleRefresh = async () => {
    if (isSpinning) return

    // Store current values before sync
    const prevSessions = sessionsIndexed
    const prevCommits = commitsFound
    const prevLinks = linksCreated

    const started = await triggerSync()
    if (started) {
      // Start polling for completion
      pollForCompletion(prevSessions, prevCommits, prevLinks)
    }
  }

  // Show error toast when sync fails (using useEffect to prevent repeated calls)
  useEffect(() => {
    if (syncStatus === 'error' && syncError) {
      // Check if we already showed this error
      if (lastToastRef.current?.type === 'error' && lastToastRef.current?.error === syncError) {
        return
      }
      lastToastRef.current = { type: 'error', error: syncError }

      toast.error('Sync failed', {
        description: syncError,
        duration: 6000,
        action: {
          label: 'Retry',
          onClick: handleRetry,
        },
      })
    } else if (syncStatus === 'conflict') {
      // Check if we already showed conflict toast
      if (lastToastRef.current?.type === 'conflict') {
        return
      }
      lastToastRef.current = { type: 'conflict' }

      toast.info('Sync already in progress', {
        description: 'Please wait for the current sync to complete.',
        duration: 3000,
      })
    } else if (syncStatus === 'idle' || syncStatus === 'success') {
      // Reset toast tracking when sync completes or is idle
      lastToastRef.current = null
    }
  }, [syncStatus, syncError, handleRetry])

  return (
    <footer
      className="h-8 bg-white dark:bg-gray-900 border-t border-gray-200 dark:border-gray-700 px-4 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400"
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
        className="inline-flex items-center gap-1.5 px-2.5 py-1 -mr-1 rounded text-xs font-medium text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
        aria-label={isSyncing ? 'Sync in progress' : 'Sync now'}
        data-testid="sync-button"
      >
        <RefreshCw
          className={`w-3.5 h-3.5 ${isSpinning ? 'animate-spin' : ''}`}
          aria-hidden="true"
        />
        {isSyncing ? 'Syncing...' : 'Sync Now'}
      </button>
    </footer>
  )
}
