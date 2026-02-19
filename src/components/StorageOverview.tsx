import { useState, useCallback, useEffect } from 'react'
import { RefreshCw, Loader2, AlertCircle, CheckCircle2, Trash2, MessageSquare, FolderOpen, GitCommit, Calendar, Database, GitBranch } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { PieChart, Pie, Cell, ResponsiveContainer, Sector } from 'recharts'
import {
  useStorageStats,
  formatBytes,
  formatTimestamp,
  formatDurationMs,
} from '../hooks/use-storage-stats'
import { useClearCache } from '../hooks/use-system'
import { useIndexingProgress } from '../hooks/use-indexing-progress'
import { StatCard } from './ui'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'

/** Colors for each storage category — soft, distinct, calming */
const STORAGE_COLORS = [
  { fill: '#D97757', label: 'JSONL Sessions' },   // Claude Code terracotta — their data
  { fill: '#6366f1', label: 'SQLite Database' },   // indigo — this app's DB
  { fill: '#10b981', label: 'Search Index' },       // emerald — this app's index
] as const

/**
 * Active shape renderer for the donut chart hover state.
 * Expands the hovered slice and shows detailed label.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function renderActiveShape(props: any) {
  const {
    cx, cy, innerRadius, outerRadius, startAngle, endAngle,
    fill, payload, percent,
  } = props

  return (
    <g>
      {/* Center label */}
      <text x={cx} y={cy - 8} textAnchor="middle" className="fill-gray-800 dark:fill-gray-200" fontSize={14} fontWeight={600}>
        {payload.label}
      </text>
      <text x={cx} y={cy + 12} textAnchor="middle" className="fill-gray-500 dark:fill-gray-400" fontSize={12}>
        {payload.formattedBytes} ({(percent * 100).toFixed(1)}%)
      </text>
      {/* Expanded slice */}
      <Sector
        cx={cx} cy={cy}
        innerRadius={innerRadius - 2}
        outerRadius={outerRadius + 6}
        startAngle={startAngle}
        endAngle={endAngle}
        fill={fill}
        opacity={1}
      />
      {/* Inner ring highlight */}
      <Sector
        cx={cx} cy={cy}
        innerRadius={innerRadius - 4}
        outerRadius={innerRadius - 2}
        startAngle={startAngle}
        endAngle={endAngle}
        fill={fill}
        opacity={0.4}
      />
    </g>
  )
}

/**
 * Storage Overview section for the Settings page.
 *
 * Displays:
 * - Donut chart: JSONL, SQLite, Search Index — part-to-whole distribution
 * - Total storage in center
 * - Legend with byte sizes
 * - Counts grid: sessions, projects, commits, oldest session, index built, last sync
 * - Action buttons: Rebuild Index
 * - Index performance stats
 */
export function StorageOverview() {
  const { data: stats, isLoading, error } = useStorageStats()
  const queryClient = useQueryClient()

  const clearCache = useClearCache()
  const [isRebuilding, setIsRebuilding] = useState(false)
  const [rebuildStatus, setRebuildStatus] = useState<'idle' | 'success' | 'error'>('idle')
  const [sseEnabled, setSseEnabled] = useState(false)
  const progress = useIndexingProgress(sseEnabled)
  const [activeSlice, setActiveSlice] = useState<number | undefined>(undefined)

  // React to SSE progress phase changes
  useEffect(() => {
    if (progress.phase === 'done') {
      setIsRebuilding(false)
      setRebuildStatus('success')
      setSseEnabled(false)
      // Refresh storage stats to show new index info
      queryClient.invalidateQueries({ queryKey: ['storage-stats'] })
      queryClient.invalidateQueries({ queryKey: ['status'] })
      setTimeout(() => setRebuildStatus('idle'), 4000)
    } else if (progress.phase === 'error') {
      setIsRebuilding(false)
      setRebuildStatus('error')
      setSseEnabled(false)
      toast.error('Index rebuild failed', {
        description: progress.errorMessage ?? 'Unknown error',
      })
      setTimeout(() => setRebuildStatus('idle'), 4000)
    }
  }, [progress.phase, progress.errorMessage, queryClient])

  const onPieEnter = useCallback((_: unknown, index: number) => {
    setActiveSlice(index)
  }, [])

  const onPieLeave = useCallback(() => {
    setActiveSlice(undefined)
  }, [])

  // Calculate total storage
  const totalBytes = stats
    ? Number(stats.jsonlBytes) + Number(stats.sqliteBytes) + Number(stats.indexBytes)
    : 0

  // App-only footprint (SQLite + Search Index — data this app created)
  const appBytes = stats
    ? Number(stats.sqliteBytes) + Number(stats.indexBytes)
    : 0

  // Build donut chart data with source attribution and paths
  const chartData = stats ? [
    { label: 'JSONL Sessions', source: 'Claude Code', bytes: Number(stats.jsonlBytes), formattedBytes: formatBytes(stats.jsonlBytes), path: stats.jsonlPath },
    { label: 'SQLite Database', source: 'This app', bytes: Number(stats.sqliteBytes), formattedBytes: formatBytes(stats.sqliteBytes), path: stats.sqlitePath },
    { label: 'Search Index', source: 'This app', bytes: Number(stats.indexBytes), formattedBytes: formatBytes(stats.indexBytes), path: stats.indexPath },
  ] : []

  // Calculate throughput for index performance
  const indexThroughput =
    stats?.lastIndexDurationMs && Number(stats.lastIndexDurationMs) > 0
      ? (Number(stats.jsonlBytes) / (Number(stats.lastIndexDurationMs) / 1000) / (1024 * 1024)).toFixed(1)
      : null

  const handleClearCache = async () => {
    try {
      const result = await clearCache.mutateAsync()
      toast.success('Cache cleared', {
        description: `Freed ${formatBytes(result.clearedBytes)}`,
      })
      queryClient.invalidateQueries({ queryKey: ['storage-stats'] })
    } catch (e) {
      toast.error('Failed to clear cache', {
        description: e instanceof Error ? e.message : 'Unknown error',
      })
    }
  }

  const handleRebuildIndex = async () => {
    setIsRebuilding(true)
    setRebuildStatus('idle')

    try {
      const response = await fetch('/api/sync/deep', { method: 'POST' })
      if (response.ok || response.status === 202) {
        // Start listening to SSE for real-time progress
        setSseEnabled(true)
      } else if (response.status === 409) {
        // Already running — attach to in-progress SSE stream
        setSseEnabled(true)
      } else {
        setRebuildStatus('error')
        const errorText = await response.text().catch(() => '')
        toast.error('Failed to rebuild index', {
          description: errorText
            ? `Server error (${response.status}): ${errorText}`
            : `Unexpected error (HTTP ${response.status}). Please try again.`,
        })
        setIsRebuilding(false)
        setTimeout(() => setRebuildStatus('idle'), 4000)
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Unknown error'
      console.error('Failed to rebuild index:', e)
      setRebuildStatus('error')
      toast.error('Failed to rebuild index', {
        description: `Network error: ${message}. Please check your connection and try again.`,
      })
      setIsRebuilding(false)
      setTimeout(() => setRebuildStatus('idle'), 4000)
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
      <div className="flex flex-col items-start gap-2 py-4">
        <div className="flex items-center gap-2 text-red-500">
          <AlertCircle className="w-4 h-4" />
          <span className="text-sm">Failed to load storage data</span>
        </div>
        <p className="text-xs text-red-400 ml-6">{error.message}</p>
        <button
          type="button"
          onClick={() => queryClient.invalidateQueries({ queryKey: ['storage-stats'] })}
          className="ml-6 text-xs text-blue-500 hover:text-blue-400 underline cursor-pointer"
        >
          Retry
        </button>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Storage Donut Chart + Legend */}
      <div className="flex flex-col sm:flex-row items-center gap-6">
        {/* Donut Chart */}
        <div className="relative w-48 h-48 shrink-0">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={chartData}
                dataKey="bytes"
                nameKey="label"
                cx="50%"
                cy="50%"
                innerRadius={52}
                outerRadius={76}
                paddingAngle={2}
                activeIndex={activeSlice}
                activeShape={renderActiveShape}
                onMouseEnter={onPieEnter}
                onMouseLeave={onPieLeave}
                animationBegin={0}
                animationDuration={600}
                animationEasing="ease-out"
              >
                {chartData.map((_, i) => (
                  <Cell
                    key={STORAGE_COLORS[i].label}
                    fill={STORAGE_COLORS[i].fill}
                    opacity={activeSlice !== undefined && activeSlice !== i ? 0.4 : 1}
                    stroke="none"
                    className="cursor-pointer transition-opacity duration-150"
                  />
                ))}
              </Pie>
            </PieChart>
          </ResponsiveContainer>
          {/* Center total — only show when no slice is hovered */}
          {activeSlice === undefined && (
            <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
              <span className="text-xs text-gray-500 dark:text-gray-400">Total</span>
              <span className="text-sm font-semibold text-gray-800 dark:text-gray-200">
                {formatBytes(totalBytes)}
              </span>
            </div>
          )}
        </div>

        {/* Legend */}
        <div className="flex flex-col gap-3" role="list" aria-label="Storage breakdown">
          {chartData.map((item, i) => {
            const pct = totalBytes > 0 ? ((item.bytes / totalBytes) * 100).toFixed(1) : '0.0'
            return (
              <div
                key={item.label}
                className="flex items-center gap-3 cursor-default"
                role="listitem"
                onMouseEnter={() => setActiveSlice(i)}
                onMouseLeave={() => setActiveSlice(undefined)}
              >
                <span
                  className="w-3 h-3 rounded-full shrink-0"
                  style={{ backgroundColor: STORAGE_COLORS[i].fill }}
                  aria-hidden="true"
                />
                <div className="flex flex-col">
                  <div className="flex items-center gap-1.5">
                    <span className="text-sm font-medium text-gray-800 dark:text-gray-200">
                      {item.label}
                    </span>
                    <span className={cn(
                      'text-[10px] px-1.5 py-0.5 rounded-full font-medium leading-none',
                      item.source === 'Claude Code'
                        ? 'text-[#D97757] bg-[#D97757]/10 dark:bg-[#D97757]/20'
                        : 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400'
                    )}>
                      {item.source}
                    </span>
                  </div>
                  <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
                    {item.formattedBytes} · {pct}%
                  </span>
                  {item.path && (
                    <code className="text-[11px] text-gray-400 dark:text-gray-500 font-mono">
                      {item.path}
                    </code>
                  )}
                </div>
              </div>
            )
          })}

          {/* App footprint callout */}
          <div className="mt-1 pt-2 border-t border-gray-200 dark:border-gray-700 space-y-1">
            <p className="text-xs text-gray-500 dark:text-gray-400">
              App footprint: <span className="font-medium text-gray-700 dark:text-gray-300">{formatBytes(appBytes)}</span>
              <span className="text-gray-400 dark:text-gray-500"> — JSONL data is read-only from <code className="text-[11px]">~/.claude/</code></span>
            </p>
            {stats?.appDataPath && (
              <p className="text-[11px] text-gray-400 dark:text-gray-500">
                App data: <code className="font-mono">{stats.appDataPath}</code> — safe to delete, rebuilt on next launch
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Counts Grid - Responsive: 2 cols mobile, 3 cols tablet, 6 cols desktop */}
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
        <StatCard label="Sessions" value={formatNumber(stats?.sessionCount ?? 0)} icon={MessageSquare} />
        <StatCard label="Projects" value={formatNumber(stats?.projectCount ?? 0)} icon={FolderOpen} />
        <StatCard label="Commits" value={formatNumber(stats?.commitCount ?? 0)} icon={GitCommit} />
        <StatCard
          label="Oldest Session"
          value={formatTimestamp(stats?.oldestSessionDate ?? null)}
          icon={Calendar}
        />
        <StatCard label="Index Built" value={formatTimestamp(stats?.lastIndexAt ?? null)} icon={Database} />
        <StatCard label="Last Git Sync" value={formatTimestamp(stats?.lastGitSyncAt ?? null)} icon={GitBranch} />
      </div>

      {/* Actions */}
      <div className="pt-2">
        <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3">
          Actions
        </h4>
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2 sm:gap-3">
            <button
              type="button"
              onClick={handleRebuildIndex}
              disabled={isRebuilding}
              className={cn(
                'inline-flex items-center gap-2 px-3 py-2 min-h-[44px] min-w-[44px]',
                'text-sm font-medium rounded-md cursor-pointer',
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
            <button
              type="button"
              onClick={handleClearCache}
              disabled={clearCache.isPending}
              className={cn(
                'inline-flex items-center gap-2 px-3 py-2 min-h-[44px] min-w-[44px]',
                'text-sm font-medium rounded-md cursor-pointer',
                'transition-colors duration-150',
                'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
              )}
            >
              {clearCache.isPending ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Trash2 className="w-4 h-4" />
              )}
              Clear Cache
            </button>
          </div>

          {/* Inline progress bar — visible while rebuilding */}
          {isRebuilding && (
            <RebuildProgressBar
              phase={progress.phase}
              indexed={progress.indexed}
              total={progress.total}
            />
          )}

          {/* Success summary — brief flash after completion */}
          {rebuildStatus === 'success' && !isRebuilding && (
            <div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400 animate-in fade-in duration-200">
              <CheckCircle2 className="w-4 h-4" />
              <span>
                Rebuilt {formatNumber(progress.indexed)} session{progress.indexed !== 1 ? 's' : ''} successfully
              </span>
            </div>
          )}
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

/**
 * Inline progress bar shown during index rebuild.
 * Displays phase label, session counter, and animated bar.
 */
function RebuildProgressBar({
  phase,
  indexed,
  total,
}: {
  phase: string
  indexed: number
  total: number
}) {
  const percentage = total > 0 ? Math.min((indexed / total) * 100, 100) : 0
  const isIndeterminate = phase === 'idle' || phase === 'reading-indexes' || total === 0

  const phaseLabel =
    phase === 'reading-indexes'
      ? 'Scanning sessions...'
      : phase === 'deep-indexing'
        ? `Indexing ${formatNumber(indexed)} / ${formatNumber(total)} sessions`
        : 'Starting rebuild...'

  return (
    <div className="space-y-1.5 animate-in fade-in slide-in-from-top-1 duration-200">
      <div className="flex items-center justify-between">
        <span className="text-xs text-gray-600 dark:text-gray-400">{phaseLabel}</span>
        {!isIndeterminate && (
          <span className="text-xs font-medium tabular-nums text-gray-700 dark:text-gray-300">
            {Math.round(percentage)}%
          </span>
        )}
      </div>
      <div
        className="h-2 w-full bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden"
        role="progressbar"
        aria-valuenow={isIndeterminate ? undefined : indexed}
        aria-valuemin={0}
        aria-valuemax={isIndeterminate ? undefined : total}
        aria-label={`Index rebuild: ${phaseLabel}`}
      >
        {isIndeterminate ? (
          <div className="h-full w-1/3 rounded-full bg-gradient-to-r from-blue-500 to-indigo-500 animate-indeterminate" />
        ) : (
          <div
            className="h-full rounded-full bg-gradient-to-r from-blue-500 to-indigo-500 transition-all duration-300 ease-out"
            style={{ width: `${percentage}%` }}
          />
        )}
      </div>
    </div>
  )
}
