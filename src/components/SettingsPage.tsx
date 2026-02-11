import { useState, useCallback } from 'react'
import {
  GitBranch,
  Download,
  Info,
  RefreshCw,
  Loader2,
  CheckCircle2,
  AlertCircle,
  XCircle,
  Command,
  HardDrive,
  History,
  Terminal,
  AlertTriangle,
  ChevronDown,
} from 'lucide-react'
import { useStatus, formatRelativeTime } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useExport, type ExportFormat } from '../hooks/use-export'
import {
  useSystem,
  useReset,
  formatDuration,
  formatRelativeTimestamp,
} from '../hooks/use-system'
import { useQueryClient } from '@tanstack/react-query'
import { formatNumber } from '../lib/format-utils'
import { cn } from '../lib/utils'
import { StorageOverview } from './StorageOverview'
import { ClassificationStatus } from './ClassificationStatus'
import { ProviderSettings } from './ProviderSettings'
import type { IndexRunInfo, ClaudeCliStatus } from '../types/generated'

declare const __APP_VERSION__: string
const APP_VERSION = __APP_VERSION__

interface SettingsSectionProps {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
  className?: string
}

function SettingsSection({ icon, title, children, className }: SettingsSectionProps) {
  return (
    <div className={cn(
      'bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden',
      className
    )}>
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

interface InfoRowProps {
  label: string
  value: string | null
  className?: string
}

function InfoRow({ label, value, className }: InfoRowProps) {
  return (
    <div className={cn('flex items-center justify-between py-1.5', className)}>
      <span className="text-sm text-gray-500 dark:text-gray-400">{label}</span>
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100 tabular-nums">
        {value ?? '--'}
      </span>
    </div>
  )
}

// ============================================================================
// Index History Section
// ============================================================================

function IndexHistorySection({
  history,
  isLoading,
}: {
  history?: IndexRunInfo[]
  isLoading: boolean
}) {
  const [isExpanded, setIsExpanded] = useState(false)

  if (isLoading || !history) {
    return (
      <SettingsSection icon={<History className="w-4 h-4" />} title="Index History">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SettingsSection>
    )
  }

  if (history.length === 0) {
    return (
      <SettingsSection icon={<History className="w-4 h-4" />} title="Index History">
        <p className="text-sm text-gray-500 dark:text-gray-400 py-2">No index runs recorded yet.</p>
      </SettingsSection>
    )
  }

  const displayed = isExpanded ? history : history.slice(0, 5)

  return (
    <SettingsSection icon={<History className="w-4 h-4" />} title="Index History">
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex items-center gap-1.5 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 mb-3 cursor-pointer transition-colors"
      >
        <ChevronDown className={cn('w-4 h-4 transition-transform duration-200', isExpanded && 'rotate-180')} />
        {isExpanded ? 'Collapse' : `Show ${history.length} runs`}
      </button>

      {(isExpanded || history.length <= 5) && (
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
      )}
    </SettingsSection>
  )
}

// ============================================================================
// Claude CLI Status Section
// ============================================================================

function CliStatusSection({
  cli,
  isLoading,
}: {
  cli?: ClaudeCliStatus
  isLoading: boolean
}) {
  if (isLoading || !cli) {
    return (
      <SettingsSection icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
        <div className="flex items-center gap-2 text-gray-400 py-4">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span className="text-sm">Loading...</span>
        </div>
      </SettingsSection>
    )
  }

  if (!cli.path) {
    return (
      <SettingsSection icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
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
      </SettingsSection>
    )
  }

  return (
    <SettingsSection icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
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
    </SettingsSection>
  )
}

// ============================================================================
// Danger Zone Section
// ============================================================================

function DangerZoneSection() {
  const reset = useReset()
  const [showResetConfirm, setShowResetConfirm] = useState(false)
  const [resetInput, setResetInput] = useState('')

  const handleReset = async () => {
    if (resetInput !== 'RESET_ALL_DATA') return
    try {
      await reset.mutateAsync('RESET_ALL_DATA')
      setShowResetConfirm(false)
      setResetInput('')
    } catch {
      // Error state handled by mutation
    }
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-red-200 dark:border-red-900/50 overflow-hidden">
      <div className="flex items-center gap-2 px-4 py-3 bg-red-50 dark:bg-red-950/30 border-b border-red-200 dark:border-red-900/50">
        <AlertTriangle className="w-4 h-4 text-red-500" />
        <h2 className="text-sm font-semibold text-red-700 dark:text-red-400 uppercase tracking-wide">
          Danger Zone
        </h2>
      </div>
      <div className="p-4">
        {!showResetConfirm ? (
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium text-gray-900 dark:text-gray-100">Reset All Data</p>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                Permanently delete all session metadata, indexes, and classification data.
              </p>
            </div>
            <button
              type="button"
              onClick={() => setShowResetConfirm(true)}
              className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800 rounded-md hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors cursor-pointer"
            >
              Reset...
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-start gap-3">
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
            <div>
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
                Reset All Data
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ============================================================================
// Main Settings Page
// ============================================================================

/**
 * Settings page — unified system settings and status.
 *
 * Route: /settings
 *
 * Sections:
 * 1. DATA & STORAGE: Donut chart, counts grid, rebuild index, clear cache, performance stats
 * 2. CLASSIFICATION: Classification status card + provider settings
 * 3. CLAUDE CLI: CLI install/auth status (powers classification)
 * 4. GIT SYNC: Last sync, commits found, links created, [Sync Git History] button
 * 5. INDEX HISTORY: Collapsible table of past index runs
 * 6. EXPORT DATA: Format radio (JSON/CSV), Scope radio (All/Current project), [Download Export] button
 * 7. ABOUT: Version, keyboard shortcuts
 * 8. DANGER ZONE: Reset All Data (type-to-confirm)
 */
export function SettingsPage() {
  const { data: status } = useStatus()
  const { data: systemData, isLoading: systemLoading } = useSystem()
  const { triggerSync, status: syncStatus, isLoading: isSyncing, error: syncError, reset: resetSync } = useGitSync()
  const { exportSessions, isExporting, error: exportError, clearError: clearExportError } = useExport()

  const queryClient = useQueryClient()
  const [exportFormat, setExportFormat] = useState<ExportFormat>('json')
  const [exportScope, setExportScope] = useState<'all' | 'project'>('all')
  const [isSavingInterval, setIsSavingInterval] = useState(false)
  const [intervalSaveStatus, setIntervalSaveStatus] = useState<'idle' | 'success' | 'error'>('idle')
  const [showProviderSettings, setShowProviderSettings] = useState(false)

  const handleIntervalChange = useCallback(async (value: string) => {
    const secs = parseInt(value, 10)
    if (isNaN(secs)) return

    setIsSavingInterval(true)
    setIntervalSaveStatus('idle')
    try {
      const res = await fetch('/api/settings/git-sync-interval', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ intervalSecs: secs }),
      })
      if (!res.ok) throw new Error('Failed to save')
      setIntervalSaveStatus('success')
      queryClient.invalidateQueries({ queryKey: ['status'] })
      setTimeout(() => setIntervalSaveStatus('idle'), 2000)
    } catch {
      setIntervalSaveStatus('error')
      setTimeout(() => setIntervalSaveStatus('idle'), 3000)
    } finally {
      setIsSavingInterval(false)
    }
  }, [queryClient])

  const handleSync = async () => {
    resetSync()
    await triggerSync()
  }

  const handleExport = async () => {
    clearExportError()
    await exportSessions(exportFormat)
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-6">
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-6">Settings</h1>

        <div className="space-y-4">
          {/* STORAGE OVERVIEW */}
          <SettingsSection icon={<HardDrive className="w-4 h-4" />} title="Data & Storage">
            <StorageOverview />
          </SettingsSection>

          {/* CLASSIFICATION */}
          <ClassificationStatus
            onConfigure={() => setShowProviderSettings((v) => !v)}
          />

          {/* CLASSIFICATION PROVIDER SETTINGS */}
          {showProviderSettings && <ProviderSettings />}

          {/* CLAUDE CLI STATUS */}
          <CliStatusSection cli={systemData?.claudeCli} isLoading={systemLoading} />

          {/* GIT SYNC */}
          <SettingsSection icon={<GitBranch className="w-4 h-4" />} title="Git Sync">
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
              Scans git history and correlates commits with sessions.
            </p>

            {status && (
              <div className="space-y-0 mb-4">
                <InfoRow
                  label="Last sync"
                  value={status.lastGitSyncAt ? formatRelativeTime(status.lastGitSyncAt) : 'Never'}
                />
                <InfoRow
                  label="Commits found"
                  value={formatNumber(status.commitsFound)}
                />
                <InfoRow
                  label="Links created"
                  value={formatNumber(status.linksCreated)}
                />
              </div>
            )}

            {/* Sync interval setting */}
            <div className="mb-4">
              <div className="flex items-center justify-between">
                <label htmlFor="sync-interval" className="text-sm text-gray-500 dark:text-gray-400">
                  Auto-sync interval
                </label>
                <div className="flex items-center gap-2">
                  <select
                    id="sync-interval"
                    value={status?.gitSyncIntervalSecs != null ? Number(status.gitSyncIntervalSecs) : 60}
                    onChange={(e) => handleIntervalChange(e.target.value)}
                    disabled={isSavingInterval}
                    className="text-sm border border-gray-200 dark:border-gray-700 rounded px-2 py-1 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-400 focus:outline-none disabled:opacity-50"
                  >
                    <option value="10">10 seconds</option>
                    <option value="30">30 seconds</option>
                    <option value="60">1 minute</option>
                    <option value="120">2 minutes</option>
                    <option value="300">5 minutes</option>
                    <option value="600">10 minutes</option>
                    <option value="1800">30 minutes</option>
                    <option value="3600">1 hour</option>
                  </select>
                  {isSavingInterval && <Loader2 className="w-3.5 h-3.5 animate-spin text-gray-400" />}
                  {intervalSaveStatus === 'success' && <CheckCircle2 className="w-3.5 h-3.5 text-green-500" />}
                  {intervalSaveStatus === 'error' && <AlertCircle className="w-3.5 h-3.5 text-red-500" />}
                </div>
              </div>
            </div>

            {/* Sync status message */}
            {syncStatus === 'success' && (
              <div className="flex items-center gap-2 text-green-600 dark:text-green-400 mb-3 text-sm">
                <CheckCircle2 className="w-4 h-4" />
                <span>Sync started successfully</span>
              </div>
            )}
            {syncStatus === 'conflict' && (
              <div className="flex items-center gap-2 text-amber-600 dark:text-amber-400 mb-3 text-sm">
                <AlertCircle className="w-4 h-4" />
                <span>Sync already in progress</span>
              </div>
            )}
            {syncError && (
              <div className="flex items-center gap-2 text-red-600 dark:text-red-400 mb-3 text-sm">
                <AlertCircle className="w-4 h-4" />
                <span>{syncError}</span>
              </div>
            )}

            <button
              type="button"
              onClick={handleSync}
              disabled={isSyncing}
              aria-busy={isSyncing}
              className={cn(
                'inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer',
                'transition-colors duration-150',
                'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
              )}
            >
              {isSyncing ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Syncing...
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4" />
                  Sync Git History
                </>
              )}
            </button>
          </SettingsSection>

          {/* INDEX HISTORY */}
          <IndexHistorySection history={systemData?.indexHistory} isLoading={systemLoading} />

          {/* EXPORT DATA */}
          <SettingsSection icon={<Download className="w-4 h-4" />} title="Export Data">
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
              Export all session data with metrics and commits.
            </p>

            {/* Format selection */}
            <div className="mb-4">
              <label className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2 block">Format</label>
              <div className="flex items-center gap-4">
                <label className="inline-flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="format"
                    value="json"
                    checked={exportFormat === 'json'}
                    onChange={() => setExportFormat('json')}
                    className="w-4 h-4 text-blue-600 focus:ring-blue-500"
                  />
                  <span className="text-sm text-gray-700 dark:text-gray-300">JSON</span>
                </label>
                <label className="inline-flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="format"
                    value="csv"
                    checked={exportFormat === 'csv'}
                    onChange={() => setExportFormat('csv')}
                    className="w-4 h-4 text-blue-600 focus:ring-blue-500"
                  />
                  <span className="text-sm text-gray-700 dark:text-gray-300">CSV</span>
                </label>
              </div>
            </div>

            {/* Scope selection */}
            <div className="mb-4">
              <label className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2 block">Scope</label>
              <div className="flex items-center gap-4">
                <label className="inline-flex items-center gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="scope"
                    value="all"
                    checked={exportScope === 'all'}
                    onChange={() => setExportScope('all')}
                    className="w-4 h-4 text-blue-600 focus:ring-blue-500"
                  />
                  <span className="text-sm text-gray-700 dark:text-gray-300">All sessions</span>
                </label>
                <label className="inline-flex items-center gap-2 cursor-pointer opacity-50">
                  <input
                    type="radio"
                    name="scope"
                    value="project"
                    checked={exportScope === 'project'}
                    onChange={() => setExportScope('project')}
                    disabled
                    className="w-4 h-4 text-blue-600 focus:ring-blue-500"
                  />
                  <span className="text-sm text-gray-700 dark:text-gray-300">Current project only</span>
                  <span className="text-xs text-gray-400">(coming soon)</span>
                </label>
              </div>
            </div>

            {/* Export error */}
            {exportError && (
              <div className="flex items-center gap-2 text-red-600 dark:text-red-400 mb-3 text-sm">
                <AlertCircle className="w-4 h-4" />
                <span>{exportError}</span>
              </div>
            )}

            <button
              type="button"
              onClick={handleExport}
              disabled={isExporting}
              aria-busy={isExporting}
              className={cn(
                'inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer',
                'transition-colors duration-150',
                'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
              )}
            >
              {isExporting ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Exporting...
                </>
              ) : (
                <>
                  <Download className="w-4 h-4" />
                  Download Export
                </>
              )}
            </button>
          </SettingsSection>

          {/* ABOUT */}
          <SettingsSection icon={<Info className="w-4 h-4" />} title="About">
            <div className="mb-4">
              <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
                Claude View v{APP_VERSION}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                Browse and export Claude Code sessions
              </p>
            </div>

            <div className="border-t border-gray-100 dark:border-gray-800 pt-4">
              <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Keyboard Shortcuts</h3>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div className="flex items-center gap-2">
                  <kbd className="inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-gray-700 dark:text-gray-300">
                    <Command className="w-3 h-3" />K
                  </kbd>
                  <span className="text-gray-600 dark:text-gray-400">Command palette</span>
                </div>
                <div className="flex items-center gap-2">
                  <kbd className="inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-gray-700 dark:text-gray-300">
                    <Command className="w-3 h-3" />/
                  </kbd>
                  <span className="text-gray-600 dark:text-gray-400">Focus search</span>
                </div>
                <div className="flex items-center gap-2">
                  <kbd className="inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-gray-700 dark:text-gray-300">
                    <Command className="w-3 h-3" /><span className="text-[10px]">Shift</span>E
                  </kbd>
                  <span className="text-gray-600 dark:text-gray-400">Export HTML</span>
                </div>
                <div className="flex items-center gap-2">
                  <kbd className="inline-flex items-center gap-1 px-2 py-1 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-gray-700 dark:text-gray-300">
                    <Command className="w-3 h-3" /><span className="text-[10px]">Shift</span>P
                  </kbd>
                  <span className="text-gray-600 dark:text-gray-400">Export PDF</span>
                </div>
              </div>
            </div>
          </SettingsSection>

          {/* DANGER ZONE — always last */}
          <DangerZoneSection />
        </div>
      </div>
    </div>
  )
}
