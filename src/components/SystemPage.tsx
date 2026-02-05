import { useState, useCallback } from 'react'
import {
  HardDrive,
  Gauge,
  HeartPulse,
  Brain,
  History,
  Wrench,
  Terminal,
  RefreshCw,
  Trash2,
  Download,
  GitBranch,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  AlertCircle,
  Loader2,
} from 'lucide-react'
import {
  useSystem,
  useReindex,
  useClearCache,
  useGitResync,
  useReset,
  formatBytes,
  formatDuration,
  formatThroughput,
  formatRelativeTimestamp,
  type StorageInfo,
  type PerformanceInfo,
  type HealthInfo,
  type IndexRunInfo,
  type ClassificationInfo,
  type ClaudeCliInfo,
} from '../hooks/use-system'
import { cn } from '../lib/utils'

// ============================================================================
// Section Layout Components
// ============================================================================

function SectionCard({
  icon,
  title,
  children,
  className,
}: {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden',
        className
      )}
    >
      <div className="flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <span className="text-gray-500 dark:text-gray-400">{icon}</span>
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
          {title}
        </h2>
      </div>
      <div className="p-4">{children}</div>
    </div>
  )
}

function MetricRow({
  label,
  value,
  className,
}: {
  label: string
  value: string | null
  className?: string
}) {
  return (
    <div className={cn('flex items-center justify-between py-1', className)}>
      <span className="text-sm text-gray-500 dark:text-gray-400">{label}</span>
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100 tabular-nums">
        {value ?? '--'}
      </span>
    </div>
  )
}

// ============================================================================
// Storage Card
// ============================================================================

function StorageCard({ storage, isLoading }: { storage?: StorageInfo; isLoading: boolean }) {
  if (isLoading || !storage) {
    return (
      <SectionCard icon={<HardDrive className="w-4 h-4" />} title="Storage">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  return (
    <SectionCard icon={<HardDrive className="w-4 h-4" />} title="Storage">
      <div className="space-y-0">
        <MetricRow label="JSONL Files" value={formatBytes(storage.jsonlBytes)} />
        <MetricRow label="Search Index" value={formatBytes(storage.indexBytes)} />
        <MetricRow label="Database" value={formatBytes(storage.dbBytes)} />
        <MetricRow label="Cache" value={formatBytes(storage.cacheBytes)} />
        <div className="border-t border-gray-100 dark:border-gray-800 mt-2 pt-2">
          <MetricRow
            label="Total"
            value={formatBytes(storage.totalBytes)}
            className="font-semibold"
          />
        </div>
      </div>
    </SectionCard>
  )
}

// ============================================================================
// Performance Card
// ============================================================================

function PerformanceCard({
  performance,
  isLoading,
}: {
  performance?: PerformanceInfo
  isLoading: boolean
}) {
  if (isLoading || !performance) {
    return (
      <SectionCard icon={<Gauge className="w-4 h-4" />} title="Performance">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  return (
    <SectionCard icon={<Gauge className="w-4 h-4" />} title="Performance">
      <div className="space-y-0">
        <MetricRow
          label="Last Index"
          value={
            performance.lastIndexDurationMs != null
              ? formatDuration(performance.lastIndexDurationMs)
              : null
          }
        />
        <MetricRow
          label="Throughput"
          value={
            performance.throughputBytesPerSec != null
              ? formatThroughput(performance.throughputBytesPerSec)
              : null
          }
        />
        <MetricRow
          label="Sessions/sec"
          value={
            performance.sessionsPerSec != null
              ? `${performance.sessionsPerSec.toFixed(0)}`
              : null
          }
        />
      </div>
    </SectionCard>
  )
}

// ============================================================================
// Health Card
// ============================================================================

function HealthCard({ health, isLoading }: { health?: HealthInfo; isLoading: boolean }) {
  if (isLoading || !health) {
    return (
      <SectionCard icon={<HeartPulse className="w-4 h-4" />} title="Data Health">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  const statusIcon =
    health.status === 'healthy' ? (
      <CheckCircle2 className="w-4 h-4 text-green-500" />
    ) : health.status === 'warning' ? (
      <AlertCircle className="w-4 h-4 text-amber-500" />
    ) : (
      <XCircle className="w-4 h-4 text-red-500" />
    )

  const statusText =
    health.status === 'healthy'
      ? 'Healthy'
      : health.status === 'warning'
        ? `${health.errorsCount} error${health.errorsCount !== 1 ? 's' : ''}`
        : 'Needs attention'

  const statusColor =
    health.status === 'healthy'
      ? 'text-green-600 dark:text-green-400'
      : health.status === 'warning'
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-red-600 dark:text-red-400'

  return (
    <SectionCard icon={<HeartPulse className="w-4 h-4" />} title="Data Health">
      <div className="space-y-0">
        <MetricRow label="Sessions" value={health.sessionsCount.toLocaleString()} />
        <MetricRow label="Commits" value={health.commitsCount.toLocaleString()} />
        <MetricRow label="Projects" value={health.projectsCount.toLocaleString()} />
        <MetricRow label="Errors" value={health.errorsCount.toLocaleString()} />
        <div className="border-t border-gray-100 dark:border-gray-800 mt-2 pt-2">
          <div className="flex items-center justify-between py-1">
            <span className="text-sm text-gray-500 dark:text-gray-400">Status</span>
            <span className={cn('flex items-center gap-1.5 text-sm font-medium', statusColor)}>
              {statusIcon}
              {statusText}
            </span>
          </div>
        </div>
      </div>
    </SectionCard>
  )
}

// ============================================================================
// Classification Section
// ============================================================================

function ClassificationSection({
  classification,
  isLoading,
}: {
  classification?: ClassificationInfo
  isLoading: boolean
}) {
  if (isLoading || !classification) {
    return (
      <SectionCard icon={<Brain className="w-4 h-4" />} title="AI Classification">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  const total = classification.classifiedCount + classification.unclassifiedCount
  const percentage = total > 0 ? (classification.classifiedCount / total) * 100 : 0
  const isComplete = total > 0 && classification.unclassifiedCount === 0

  return (
    <SectionCard icon={<Brain className="w-4 h-4" />} title="AI Classification">
      {/* Progress bar */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">
            Classified: {classification.classifiedCount.toLocaleString()} /{' '}
            {total.toLocaleString()} ({percentage.toFixed(1)}%)
          </span>
          {isComplete && <CheckCircle2 className="w-4 h-4 text-green-500" />}
        </div>
        <div className="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className={cn(
              'h-full rounded-full transition-all duration-500',
              isComplete
                ? 'bg-green-500'
                : classification.isRunning
                  ? 'bg-blue-500 animate-pulse'
                  : 'bg-blue-500'
            )}
            style={{ width: `${percentage}%` }}
          />
        </div>
      </div>

      {/* Metrics grid */}
      <div className="grid grid-cols-2 gap-x-6 gap-y-1 mb-4">
        <MetricRow label="Provider" value={classification.provider} />
        <MetricRow label="Model" value={classification.model} />
        <MetricRow
          label="Last Run"
          value={
            classification.lastRunAt
              ? formatRelativeTimestamp(classification.lastRunAt)
              : 'Never'
          }
        />
        <MetricRow
          label="Duration"
          value={
            classification.lastRunDurationMs != null
              ? formatDuration(classification.lastRunDurationMs)
              : null
          }
        />
        {classification.lastRunCostCents != null && (
          <MetricRow
            label="Est. Cost"
            value={`~$${(classification.lastRunCostCents / 100).toFixed(2)}`}
          />
        )}
      </div>

      {/* Status message for running jobs */}
      {classification.isRunning && classification.progress != null && (
        <div className="flex items-center gap-2 text-blue-600 dark:text-blue-400 text-sm mb-2">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span>Classification in progress ({classification.progress}%)</span>
        </div>
      )}
    </SectionCard>
  )
}

// ============================================================================
// Index History Table
// ============================================================================

function IndexHistorySection({
  history,
  isLoading,
}: {
  history?: IndexRunInfo[]
  isLoading: boolean
}) {
  const [showAll, setShowAll] = useState(false)

  if (isLoading || !history) {
    return (
      <SectionCard icon={<History className="w-4 h-4" />} title="Index History">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  if (history.length === 0) {
    return (
      <SectionCard icon={<History className="w-4 h-4" />} title="Index History">
        <p className="text-sm text-gray-500 dark:text-gray-400 py-2">No index runs recorded yet.</p>
      </SectionCard>
    )
  }

  const displayed = showAll ? history : history.slice(0, 5)

  return (
    <SectionCard icon={<History className="w-4 h-4" />} title="Index History">
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500 dark:text-gray-400 border-b border-gray-100 dark:border-gray-800">
              <th className="pb-2 font-medium">Time</th>
              <th className="pb-2 font-medium">Type</th>
              <th className="pb-2 font-medium text-right">Sessions</th>
              <th className="pb-2 font-medium text-right">Duration</th>
              <th className="pb-2 font-medium text-right">Status</th>
            </tr>
          </thead>
          <tbody>
            {displayed.map((run, i) => (
              <tr
                key={`${run.timestamp}-${i}`}
                className="border-b border-gray-50 dark:border-gray-800/50 last:border-0"
              >
                <td className="py-1.5 text-gray-700 dark:text-gray-300">
                  {formatRelativeTimestamp(run.timestamp)}
                </td>
                <td className="py-1.5">
                  <span
                    className={cn(
                      'inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium',
                      run.type === 'full'
                        ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
                        : run.type === 'incremental'
                          ? 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400'
                          : 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300'
                    )}
                  >
                    {run.type}
                  </span>
                </td>
                <td className="py-1.5 text-right tabular-nums text-gray-700 dark:text-gray-300">
                  {run.sessionsCount != null ? run.sessionsCount.toLocaleString() : '--'}
                </td>
                <td className="py-1.5 text-right tabular-nums text-gray-700 dark:text-gray-300">
                  {run.durationMs != null ? formatDuration(run.durationMs) : '--'}
                </td>
                <td className="py-1.5 text-right">
                  {run.status === 'completed' ? (
                    <CheckCircle2 className="w-4 h-4 text-green-500 inline" />
                  ) : run.status === 'failed' ? (
                    <span className="inline-flex items-center gap-1">
                      <XCircle className="w-4 h-4 text-red-500" />
                      {run.errorMessage && (
                        <span
                          className="text-xs text-red-500 max-w-[120px] truncate"
                          title={run.errorMessage}
                        >
                          {run.errorMessage}
                        </span>
                      )}
                    </span>
                  ) : (
                    <Loader2 className="w-4 h-4 text-blue-500 animate-spin inline" />
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {history.length > 5 && (
        <button
          type="button"
          onClick={() => setShowAll(!showAll)}
          className="mt-3 text-sm text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
        >
          {showAll ? 'Show Less' : `Show All (${history.length})`}
        </button>
      )}
    </SectionCard>
  )
}

// ============================================================================
// Action Buttons
// ============================================================================

function ActionsSection() {
  const reindex = useReindex()
  const clearCache = useClearCache()
  const gitResync = useGitResync()
  const reset = useReset()

  const [showResetConfirm, setShowResetConfirm] = useState(false)
  const [resetInput, setResetInput] = useState('')
  const [toast, setToast] = useState<{ type: 'success' | 'error'; message: string } | null>(null)

  const showToast = useCallback((type: 'success' | 'error', message: string) => {
    setToast({ type, message })
    setTimeout(() => setToast(null), 3000)
  }, [])

  const handleReindex = async () => {
    try {
      await reindex.mutateAsync()
      showToast('success', 'Re-index started')
    } catch (e) {
      showToast('error', `Reindex failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  const handleClearCache = async () => {
    try {
      const result = await clearCache.mutateAsync()
      showToast('success', `Cache cleared (${formatBytes(result.clearedBytes)})`)
    } catch (e) {
      showToast('error', `Clear cache failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  const handleGitResync = async () => {
    try {
      await gitResync.mutateAsync()
      showToast('success', 'Git re-sync started')
    } catch (e) {
      showToast('error', `Git re-sync failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  const handleReset = async () => {
    if (resetInput !== 'RESET_ALL_DATA') return
    try {
      await reset.mutateAsync('RESET_ALL_DATA')
      showToast('success', 'All data has been reset')
      setShowResetConfirm(false)
      setResetInput('')
    } catch (e) {
      showToast('error', `Reset failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  return (
    <SectionCard icon={<Wrench className="w-4 h-4" />} title="Actions">
      {/* Toast notification */}
      {toast && (
        <div
          className={cn(
            'flex items-center gap-2 px-3 py-2 rounded-md text-sm mb-4',
            toast.type === 'success'
              ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300'
              : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
          )}
        >
          {toast.type === 'success' ? (
            <CheckCircle2 className="w-4 h-4 flex-shrink-0" />
          ) : (
            <XCircle className="w-4 h-4 flex-shrink-0" />
          )}
          <span>{toast.message}</span>
        </div>
      )}

      <div className="flex flex-wrap gap-2 mb-4">
        <ActionButton
          icon={<RefreshCw className="w-4 h-4" />}
          label="Re-index All"
          onClick={handleReindex}
          isLoading={reindex.isPending}
        />
        <ActionButton
          icon={<Trash2 className="w-4 h-4" />}
          label="Clear Cache"
          onClick={handleClearCache}
          isLoading={clearCache.isPending}
        />
        <ActionButton
          icon={<Download className="w-4 h-4" />}
          label="Export Data"
          onClick={() => window.open('/api/export/sessions?format=json', '_blank')}
        />
        <ActionButton
          icon={<GitBranch className="w-4 h-4" />}
          label="Git Re-sync"
          onClick={handleGitResync}
          isLoading={gitResync.isPending}
        />
      </div>

      {/* Reset button */}
      <div className="border-t border-gray-100 dark:border-gray-800 pt-4">
        {!showResetConfirm ? (
          <button
            type="button"
            onClick={() => setShowResetConfirm(true)}
            className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800 rounded-md hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors cursor-pointer"
          >
            <AlertTriangle className="w-4 h-4" />
            Reset All Data...
          </button>
        ) : (
          <div className="bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-lg p-4">
            <div className="flex items-start gap-3 mb-3">
              <AlertTriangle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-red-800 dark:text-red-200 mb-1">
                  This action cannot be undone.
                </p>
                <p className="text-sm text-red-600 dark:text-red-300">
                  This will permanently delete all session metadata, indexes, commit correlations,
                  and classification data. Your original JSONL files will NOT be deleted.
                </p>
              </div>
            </div>
            <div className="mb-3">
              <label
                htmlFor="reset-confirm"
                className="text-sm text-red-700 dark:text-red-300 mb-1 block"
              >
                Type <code className="font-mono bg-red-100 dark:bg-red-900/30 px-1 rounded">RESET_ALL_DATA</code> to confirm:
              </label>
              <input
                id="reset-confirm"
                type="text"
                value={resetInput}
                onChange={(e) => setResetInput(e.target.value)}
                className="w-full text-sm border border-red-200 dark:border-red-700 rounded px-3 py-1.5 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-red-400 focus:outline-none"
                placeholder="RESET_ALL_DATA"
              />
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => {
                  setShowResetConfirm(false)
                  setResetInput('')
                }}
                className="px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleReset}
                disabled={resetInput !== 'RESET_ALL_DATA' || reset.isPending}
                className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors cursor-pointer"
              >
                {reset.isPending ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <AlertTriangle className="w-4 h-4" />
                )}
                Reset All
              </button>
            </div>
          </div>
        )}
      </div>
    </SectionCard>
  )
}

function ActionButton({
  icon,
  label,
  onClick,
  isLoading,
}: {
  icon: React.ReactNode
  label: string
  onClick: () => void
  isLoading?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={isLoading}
      className={cn(
        'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md cursor-pointer',
        'transition-colors duration-150',
        'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
      )}
    >
      {isLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : icon}
      {label}
    </button>
  )
}

// ============================================================================
// Claude CLI Status
// ============================================================================

function CliStatusSection({
  cli,
  isLoading,
}: {
  cli?: ClaudeCliInfo
  isLoading: boolean
}) {
  if (isLoading || !cli) {
    return (
      <SectionCard icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SectionCard>
    )
  }

  if (!cli.path) {
    return (
      <SectionCard icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
        <div className="flex items-start gap-3">
          <XCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-2">
              Not installed
            </p>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-3">
              Install Claude CLI to enable AI classification:
            </p>
            <div className="bg-gray-50 dark:bg-gray-800 rounded-md p-3 text-sm font-mono text-gray-700 dark:text-gray-300 space-y-1">
              <p>npm install -g @anthropic-ai/claude-code</p>
              <p className="text-gray-400"># or</p>
              <p>brew install claude</p>
            </div>
          </div>
        </div>
      </SectionCard>
    )
  }

  return (
    <SectionCard icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <CheckCircle2 className="w-4 h-4 text-green-500" />
          <span className="text-sm text-gray-700 dark:text-gray-300">
            Installed:{' '}
            <code className="font-mono text-xs bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded">
              {cli.path}
            </code>
          </span>
        </div>
        {cli.version && (
          <div className="flex items-center gap-2 ml-6">
            <span className="text-sm text-gray-500 dark:text-gray-400">
              Version: {cli.version}
            </span>
          </div>
        )}
        <div className="flex items-center gap-2">
          {cli.authenticated ? (
            <>
              <CheckCircle2 className="w-4 h-4 text-green-500" />
              <span className="text-sm text-gray-700 dark:text-gray-300">
                Authenticated
                {cli.subscriptionType && cli.subscriptionType !== 'unknown' && (
                  <span className="text-gray-500 dark:text-gray-400">
                    {' '}
                    ({cli.subscriptionType.charAt(0).toUpperCase() + cli.subscriptionType.slice(1)}{' '}
                    subscription)
                  </span>
                )}
              </span>
            </>
          ) : (
            <>
              <AlertCircle className="w-4 h-4 text-amber-500" />
              <span className="text-sm text-gray-700 dark:text-gray-300">Not authenticated</span>
              <span className="text-xs text-gray-400 ml-1">
                Run:{' '}
                <code className="font-mono bg-gray-100 dark:bg-gray-800 px-1 rounded">
                  claude auth login
                </code>
              </span>
            </>
          )}
        </div>
      </div>
    </SectionCard>
  )
}

// ============================================================================
// Main System Page
// ============================================================================

export function SystemPage() {
  const { data, isLoading, error } = useSystem()

  if (error) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="max-w-4xl mx-auto px-6 py-6">
          <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-6">
            System Status
          </h1>
          <div className="flex items-center gap-3 text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 rounded-lg p-4">
            <XCircle className="w-5 h-5 flex-shrink-0" />
            <div>
              <p className="text-sm font-medium">Failed to load system status</p>
              <p className="text-sm mt-1">{error.message}</p>
            </div>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto px-6 py-6">
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-6">
          System Status
        </h1>

        <div className="space-y-4">
          {/* Top metrics row: 3 cards side by side */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <StorageCard storage={data?.storage} isLoading={isLoading} />
            <PerformanceCard performance={data?.performance} isLoading={isLoading} />
            <HealthCard health={data?.health} isLoading={isLoading} />
          </div>

          {/* Classification section */}
          <ClassificationSection
            classification={data?.classification}
            isLoading={isLoading}
          />

          {/* Index history */}
          <IndexHistorySection history={data?.indexHistory} isLoading={isLoading} />

          {/* Actions */}
          <ActionsSection />

          {/* Claude CLI status */}
          <CliStatusSection cli={data?.claudeCli} isLoading={isLoading} />
        </div>
      </div>
    </div>
  )
}
