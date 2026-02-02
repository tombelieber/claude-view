import { useState, useCallback } from 'react'
import {
  Database,
  GitBranch,
  Download,
  Info,
  RefreshCw,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Command,
} from 'lucide-react'
import { useStatus, formatRelativeTime } from '../hooks/use-status'
import { useGitSync } from '../hooks/use-git-sync'
import { useExport, type ExportFormat } from '../hooks/use-export'
import { useQueryClient } from '@tanstack/react-query'
import { formatNumber } from '../lib/format-utils'
import { cn } from '../lib/utils'

// Hardcoded version - should match package.json
const APP_VERSION = '0.1.0'

interface SettingsSectionProps {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
}

function SettingsSection({ icon, title, children }: SettingsSectionProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
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

/**
 * Settings page with data status, git sync, export, and about sections.
 *
 * Route: /settings
 *
 * Sections:
 * 1. DATA STATUS: Last indexed, duration, sessions, projects
 * 2. GIT SYNC: Last sync, commits found, links created, [Sync Git History] button
 * 3. EXPORT DATA: Format radio (JSON/CSV), Scope radio (All/Current project), [Download Export] button
 * 4. ABOUT: Version, keyboard shortcuts
 */
export function SettingsPage() {
  const { data: status, isLoading: isStatusLoading } = useStatus()
  const { triggerSync, status: syncStatus, isLoading: isSyncing, error: syncError, reset: resetSync } = useGitSync()
  const { exportSessions, isExporting, error: exportError, clearError: clearExportError } = useExport()

  const queryClient = useQueryClient()
  const [exportFormat, setExportFormat] = useState<ExportFormat>('json')
  const [exportScope, setExportScope] = useState<'all' | 'project'>('all')
  const [isSavingInterval, setIsSavingInterval] = useState(false)
  const [intervalSaveStatus, setIntervalSaveStatus] = useState<'idle' | 'success' | 'error'>('idle')

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
    // Note: project scope not implemented yet - would need project context
    await exportSessions(exportFormat)
  }

  // Format duration in milliseconds to human-readable
  const formatDurationMs = (ms: bigint | null): string => {
    if (ms === null) return '--'
    const seconds = Number(ms) / 1000
    if (seconds < 1) return `${Number(ms)}ms`
    return `${seconds.toFixed(1)}s`
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-6">
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-6">Settings</h1>

        <div className="space-y-4">
          {/* DATA STATUS */}
          <SettingsSection icon={<Database className="w-4 h-4" />} title="Data Status">
            {isStatusLoading ? (
              <div className="flex items-center gap-2 text-gray-400 py-4">
                <Loader2 className="w-4 h-4 animate-spin" />
                <span className="text-sm">Loading status...</span>
              </div>
            ) : status ? (
              <div className="space-y-0">
                <InfoRow
                  label="Last indexed"
                  value={status.lastIndexedAt ? formatRelativeTime(status.lastIndexedAt) : 'Never'}
                />
                <InfoRow
                  label="Index duration"
                  value={formatDurationMs(status.lastIndexDurationMs)}
                />
                <InfoRow
                  label="Sessions"
                  value={formatNumber(status.sessionsIndexed)}
                />
                <InfoRow
                  label="Projects"
                  value={formatNumber(status.projectsIndexed)}
                />
              </div>
            ) : (
              <p className="text-sm text-gray-500 dark:text-gray-400">No status data available</p>
            )}
          </SettingsSection>

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
        </div>
      </div>
    </div>
  )
}
