import { useState } from 'react'
import { RefreshCw, Trash2, Loader2, AlertCircle, CheckCircle2 } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import {
  useStorageStats,
  formatBytes,
  formatTimestamp,
  formatDurationMs,
} from '../hooks/use-storage-stats'
import { ProgressBar } from './ui/ProgressBar'
import { StatCard } from './ui/StatCard'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'

/**
 * Storage Overview section for the Settings page.
 *
 * Displays:
 * - Storage bars: JSONL, SQLite, Search Index with progress visualization
 * - Total storage summary
 * - Counts grid: sessions, projects, commits, oldest session, index built, last sync
 * - Action buttons: Rebuild Index, Clear Cache
 * - Index performance stats
 */
export function StorageOverview() {
  const { data: stats, isLoading, error } = useStorageStats()
  const queryClient = useQueryClient()

  const [isRebuilding, setIsRebuilding] = useState(false)
  const [rebuildStatus, setRebuildStatus] = useState<'idle' | 'success' | 'error'>('idle')

  // Calculate total storage
  const totalBytes = stats
    ? Number(stats.jsonlBytes) + Number(stats.sqliteBytes) + Number(stats.indexBytes)
    : 0

  // Calculate percentages for progress bars
  const jsonlPercent = totalBytes > 0 ? (Number(stats?.jsonlBytes ?? 0) / totalBytes) * 100 : 0
  const sqlitePercent = totalBytes > 0 ? (Number(stats?.sqliteBytes ?? 0) / totalBytes) * 100 : 0
  const indexPercent = totalBytes > 0 ? (Number(stats?.indexBytes ?? 0) / totalBytes) * 100 : 0

  // Calculate throughput for index performance
  const indexThroughput =
    stats?.lastIndexDurationMs && Number(stats.lastIndexDurationMs) > 0
      ? (Number(stats.jsonlBytes) / (Number(stats.lastIndexDurationMs) / 1000) / (1024 * 1024)).toFixed(1)
      : null

  const handleRebuildIndex = async () => {
    setIsRebuilding(true)
    setRebuildStatus('idle')

    try {
      // Trigger full Tantivy index rebuild via /api/sync/deep
      const response = await fetch('/api/sync/deep', { method: 'POST' })
      if (response.ok || response.status === 202) {
        setRebuildStatus('success')
        toast.success('Index rebuild started', {
          description: 'Full Tantivy index rebuild initiated. This may take a moment.',
        })
        // Invalidate queries to refresh data
        queryClient.invalidateQueries({ queryKey: ['storage-stats'] })
        queryClient.invalidateQueries({ queryKey: ['status'] })
      } else if (response.status === 409) {
        // Sync already in progress
        setRebuildStatus('idle')
        toast.info('Rebuild in progress', {
          description: 'An index rebuild is already running. Please wait for it to complete.',
        })
      } else {
        setRebuildStatus('error')
        toast.error('Failed to rebuild index', {
          description: 'An unexpected error occurred. Please try again.',
        })
      }
    } catch {
      setRebuildStatus('error')
      toast.error('Failed to rebuild index', {
        description: 'Network error. Please check your connection and try again.',
      })
    } finally {
      setIsRebuilding(false)
      setTimeout(() => setRebuildStatus('idle'), 3000)
    }
  }

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-gray-400 py-8">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Loading storage data...</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex items-center gap-2 text-red-500 py-4">
        <AlertCircle className="w-4 h-4" />
        <span className="text-sm">Failed to load storage data</span>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Storage Bars */}
      <div className="space-y-1">
        <ProgressBar
          label="JSONL Sessions"
          value={jsonlPercent}
          max={100}
          suffix={formatBytes(stats?.jsonlBytes ?? 0)}
        />
        <ProgressBar
          label="SQLite Database"
          value={sqlitePercent}
          max={100}
          suffix={formatBytes(stats?.sqliteBytes ?? 0)}
        />
        <ProgressBar
          label="Search Index"
          value={indexPercent}
          max={100}
          suffix={formatBytes(stats?.indexBytes ?? 0)}
        />
      </div>

      {/* Total Storage */}
      <div className="text-sm text-gray-600 dark:text-gray-400">
        <span className="font-medium">Total:</span>{' '}
        <span className="font-semibold text-gray-800 dark:text-gray-200">
          {formatBytes(totalBytes)}
        </span>
      </div>

      {/* Counts Grid */}
      <div className="grid grid-cols-3 gap-2">
        <StatCard label="Sessions" value={formatNumber(stats?.sessionCount ?? 0)} />
        <StatCard label="Projects" value={formatNumber(stats?.projectCount ?? 0)} />
        <StatCard label="Commits" value={formatNumber(stats?.commitCount ?? 0)} />
        <StatCard
          label="Oldest Session"
          value={formatTimestamp(stats?.oldestSessionDate ?? null)}
        />
        <StatCard label="Index Built" value={formatTimestamp(stats?.lastIndexAt ?? null)} />
        <StatCard label="Last Git Sync" value={formatTimestamp(stats?.lastGitSyncAt ?? null)} />
      </div>

      {/* Actions */}
      <div className="pt-2">
        <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
          Actions
        </h4>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={handleRebuildIndex}
            disabled={isRebuilding}
            className={cn(
              'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md cursor-pointer',
              'transition-colors duration-150',
              'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
            )}
          >
            {isRebuilding ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : rebuildStatus === 'success' ? (
              <CheckCircle2 className="w-4 h-4 text-green-500" />
            ) : rebuildStatus === 'error' ? (
              <AlertCircle className="w-4 h-4 text-red-500" />
            ) : (
              <RefreshCw className="w-4 h-4" />
            )}
            Rebuild Index
          </button>

          {/* TODO: Wire up to DELETE /api/cache endpoint when backend implements it.
              The endpoint should clear the Tantivy index only (SQLite stays).
              See design doc: docs/plans/2026-02-05-dashboard-analytics-design.md */}
          <button
            type="button"
            disabled
            className={cn(
              'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md cursor-not-allowed',
              'bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500',
              'opacity-50'
            )}
            title="Clear Cache (endpoint not yet implemented)"
          >
            <Trash2 className="w-4 h-4" />
            Clear Cache
          </button>
        </div>
      </div>

      {/* Index Performance */}
      {stats?.lastIndexDurationMs !== null && (
        <div className="pt-2">
          <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2">
            Index Performance
          </h4>
          <div className="text-sm text-gray-600 dark:text-gray-400 space-y-1">
            <p>
              <span className="text-gray-500 dark:text-gray-500">Last deep index:</span>{' '}
              <span className="font-medium text-gray-800 dark:text-gray-200">
                {formatDurationMs(stats?.lastIndexDurationMs ?? null)}
              </span>
              {indexThroughput && (
                <span className="text-gray-500 dark:text-gray-500">
                  {' '}
                  ({formatNumber(stats?.lastIndexSessionCount ?? 0)} sessions{' '}
                  {indexThroughput} MB/s)
                </span>
              )}
            </p>
            {stats?.lastGitSyncAt !== null && (
              <p>
                <span className="text-gray-500 dark:text-gray-500">Last git sync:</span>{' '}
                <span className="font-medium text-gray-800 dark:text-gray-200">
                  {formatTimestamp(stats?.lastGitSyncAt ?? null)}
                </span>
                {Number(stats?.commitCount ?? 0) > 0 && (
                  <span className="text-gray-500 dark:text-gray-500">
                    {' '}
                    ({formatNumber(stats?.commitCount ?? 0)} commits linked)
                  </span>
                )}
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
