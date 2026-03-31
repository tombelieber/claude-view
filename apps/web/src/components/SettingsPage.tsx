import { useQueryClient } from '@tanstack/react-query'
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  Command,
  Download,
  ExternalLink,
  GitBranch,
  HardDrive,
  History,
  Info,
  Link2,
  Loader2,
  RefreshCw,
  Shield,
  Smartphone,
  Star,
  XCircle,
} from 'lucide-react'
import { useCallback, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { useConfig } from '../hooks/use-config'
import { type ExportFormat, useExport } from '../hooks/use-export'
import { useGitSync } from '../hooks/use-git-sync'
import { useRevokeShare, useShares } from '../hooks/use-share'
import { formatRelativeTime, useStatus } from '../hooks/use-status'
import { formatDuration, formatRelativeTimestamp, useReset, useSystem } from '../hooks/use-system'
import { useTelemetry } from '../hooks/use-telemetry'
import { useTrackEvent } from '../hooks/use-track-event'
import { formatNumber } from '../lib/format-utils'
import { TOAST_DURATION } from '../lib/notify'
import { cn } from '../lib/utils'
import type { IndexRunInfo } from '../types/generated'
import { AccountSection } from './AccountSection'
import { OnDeviceAiCard } from './OnDeviceAiCard'
import { ProviderSettings } from './ProviderSettings'
import { StorageOverview } from './StorageOverview'
import { TelemetrySection } from './TelemetrySection'
import { SegmentedControl } from './ui/SegmentedControl'

declare const __APP_VERSION__: string
declare const __APP_BUILD_DATE__: string
const APP_VERSION = __APP_VERSION__
const APP_BUILD_DATE = __APP_BUILD_DATE__

const GITHUB_REPO = 'tombelieber/claude-view'
const GITHUB_URL = `https://github.com/${GITHUB_REPO}`

interface SettingsSectionProps {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
  className?: string
}

function SettingsSection({ icon, title, children, className }: SettingsSectionProps) {
  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700',
        className,
      )}
    >
      <div className="px-5 pt-4 pb-1.5">
        <div className="flex items-center gap-2">
          <span className="text-gray-400 dark:text-gray-500">{icon}</span>
          <h2 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
            {title}
          </h2>
        </div>
      </div>
      <div className="px-5 pb-5 pt-1">{children}</div>
    </div>
  )
}

function SectionGroup({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <h2 className="text-[11px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-[0.12em] mb-3 px-1">
        {label}
      </h2>
      <div className="space-y-3">{children}</div>
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
        <ChevronDown
          className={cn('w-4 h-4 transition-transform duration-200', isExpanded && 'rotate-180')}
        />
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
                            : 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300',
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
                Type{' '}
                <code className="font-mono bg-red-100 dark:bg-red-900/30 px-1 rounded">
                  RESET_ALL_DATA
                </code>{' '}
                to confirm:
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
// Shared Links Section
// ============================================================================

function SharedLinksSection() {
  const { data: shares, isLoading } = useShares()
  const revokeShare = useRevokeShare()

  if (isLoading) return <div className="text-gray-500 dark:text-gray-400 text-sm">Loading...</div>
  if (!shares?.length) {
    return (
      <div className="text-gray-500 dark:text-gray-400 text-sm">No shared conversations yet.</div>
    )
  }

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-gray-500 dark:text-gray-400 text-left border-b border-gray-200 dark:border-gray-700">
          <th className="pb-2">Title</th>
          <th className="pb-2">Created</th>
          <th className="pb-2">Views</th>
          <th className="pb-2">Link</th>
          <th className="pb-2" />
        </tr>
      </thead>
      <tbody>
        {shares.map((share) => (
          <tr key={share.token} className="border-b border-gray-100 dark:border-gray-800">
            <td className="py-2 text-gray-700 dark:text-gray-300">{share.title ?? 'Untitled'}</td>
            <td className="py-2 text-gray-500 dark:text-gray-400">
              {share.created_at > 0
                ? new Date(share.created_at * 1000).toLocaleDateString()
                : '\u2014'}
            </td>
            <td className="py-2 text-gray-500 dark:text-gray-400">{share.view_count}</td>
            <td className="py-2">
              {share.url ? (
                <button
                  type="button"
                  onClick={() => {
                    if (share.url) navigator.clipboard.writeText(share.url)
                    toast.success('Copied to clipboard', { duration: TOAST_DURATION.micro })
                  }}
                  className="text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300 truncate max-w-48 text-left"
                  title={share.url}
                >
                  Copy link
                </button>
              ) : (
                <span className="text-gray-400 dark:text-gray-500 text-sm">Link unavailable</span>
              )}
            </td>
            <td className="py-2">
              <button
                type="button"
                disabled={revokeShare.isPending}
                onClick={() => {
                  if (confirm('Revoke this share? The link will stop working.')) {
                    revokeShare.mutate(share.session_id)
                  }
                }}
                className="text-red-600 dark:text-red-500 hover:text-red-500 dark:hover:text-red-400 text-xs disabled:opacity-50 disabled:cursor-wait"
              >
                {revokeShare.isPending ? 'Revoking\u2026' : 'Revoke'}
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

// ============================================================================
// Keyboard Shortcut Helpers (for About section)
// ============================================================================

interface KeyCombo {
  mod?: boolean // Cmd on Mac
  ctrl?: boolean // Ctrl
  shift?: boolean
  key: string
}

interface ShortcutDef {
  keys: KeyCombo[]
  label: string
  separator?: string // e.g. "/" to show "j / k"
}

function ShortcutKbd({ combo }: { combo: KeyCombo }) {
  return (
    <kbd className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-xs font-mono bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-gray-700 dark:text-gray-300">
      {combo.mod && <Command className="w-3 h-3" />}
      {combo.ctrl && <span className="text-[10px]">Ctrl</span>}
      {combo.shift && <span className="text-[10px]">Shift</span>}
      {combo.key}
    </kbd>
  )
}

function ShortcutGroup({ title, shortcuts }: { title: string; shortcuts: ShortcutDef[] }) {
  return (
    <div>
      <h4 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2">
        {title}
      </h4>
      <div className="space-y-1.5">
        {shortcuts.map((s) => (
          <div key={s.label} className="flex items-center justify-between">
            <div className="flex items-center gap-1">
              {s.keys.map((combo, i) => (
                <span key={i} className="inline-flex items-center gap-1">
                  {i > 0 && s.separator && (
                    <span className="text-xs text-gray-400 dark:text-gray-500 mx-0.5">
                      {s.separator}
                    </span>
                  )}
                  <ShortcutKbd combo={combo} />
                </span>
              ))}
            </div>
            <span className="text-xs text-gray-500 dark:text-gray-400">{s.label}</span>
          </div>
        ))}
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
 * 2. CLASSIFICATION: Classification status card + provider settings (CLI status inline)
 * 3. GIT SYNC: Last sync, commits found, links created, [Sync Git History] button
 * 4. INDEX HISTORY: Collapsible table of past index runs
 * 5. EXPORT DATA: Format radio (JSON/CSV), Scope radio (All/Current project), [Download Export] button
 * 6. ABOUT: Version, keyboard shortcuts
 * 7. DANGER ZONE: Reset All Data (type-to-confirm)
 */
export function SettingsPage() {
  const { data: status } = useStatus()
  const { data: systemData, isLoading: systemLoading } = useSystem()
  const {
    triggerSync,
    status: syncStatus,
    isLoading: isSyncing,
    error: syncError,
    reset: resetSync,
  } = useGitSync()
  const {
    exportSessions,
    isExporting,
    error: exportError,
    clearError: clearExportError,
  } = useExport()

  const queryClient = useQueryClient()
  const [exportFormat, setExportFormat] = useState<ExportFormat>('json')
  const [isSavingInterval, setIsSavingInterval] = useState(false)
  const [intervalSaveStatus, setIntervalSaveStatus] = useState<'idle' | 'success' | 'error'>('idle')
  const [_searchParams, _setSearchParams] = useSearchParams()
  const trackEvent = useTrackEvent()

  const handleIntervalChange = useCallback(
    async (value: string) => {
      const secs = Number.parseInt(value, 10)
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
        trackEvent('settings_changed', { changed_fields: ['git_sync_interval'] })
        queryClient.invalidateQueries({ queryKey: ['status'] })
        setTimeout(() => setIntervalSaveStatus('idle'), 2000)
      } catch {
        setIntervalSaveStatus('error')
        setTimeout(() => setIntervalSaveStatus('idle'), 3000)
      } finally {
        setIsSavingInterval(false)
      }
    },
    [queryClient, trackEvent],
  )

  const handleSync = async () => {
    resetSync()
    await triggerSync()
  }

  const handleExport = async () => {
    clearExportError()
    await exportSessions(exportFormat)
  }

  const config = useConfig()
  const { enableTelemetry, disableTelemetry } = useTelemetry()

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-8">
        <div className="flex items-baseline gap-3 mb-8">
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Settings</h1>
          <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
            v{APP_VERSION}
          </span>
        </div>

        <div className="space-y-8">
          {/* ── Account & AI ─────────────────────────────────── */}
          <SectionGroup label="Account & AI">
            <AccountSection />
            <OnDeviceAiCard />
            <ProviderSettings cliStatus={systemData?.claudeCli} />
          </SectionGroup>

          {/* ── Data ─────────────────────────────────────────── */}
          <SectionGroup label="Data">
            <SettingsSection icon={<HardDrive className="w-4 h-4" />} title="Storage">
              <StorageOverview />
            </SettingsSection>

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
                <InfoRow label="Commits found" value={formatNumber(status.commitsFound)} />
                <InfoRow label="Links created" value={formatNumber(status.linksCreated)} />
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
                    value={
                      status?.gitSyncIntervalSecs != null ? Number(status.gitSyncIntervalSecs) : 60
                    }
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
                  {isSavingInterval && (
                    <Loader2 className="w-3.5 h-3.5 animate-spin text-gray-400" />
                  )}
                  {intervalSaveStatus === 'success' && (
                    <CheckCircle2 className="w-3.5 h-3.5 text-green-500" />
                  )}
                  {intervalSaveStatus === 'error' && (
                    <AlertCircle className="w-3.5 h-3.5 text-red-500" />
                  )}
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
                'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
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

            <IndexHistorySection history={systemData?.indexHistory} isLoading={systemLoading} />

            <SettingsSection icon={<Download className="w-4 h-4" />} title="Export">
              <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
                Export all session data with metrics and commits.
              </p>

              <div className="flex flex-wrap items-center gap-4 mb-4">
                <div>
                  <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider block mb-1.5">
                    Format
                  </span>
                  <SegmentedControl
                    value={exportFormat}
                    onChange={setExportFormat}
                    options={[
                      { value: 'json' as ExportFormat, label: 'JSON' },
                      { value: 'csv' as ExportFormat, label: 'CSV' },
                    ]}
                    ariaLabel="Export format"
                  />
                </div>
              </div>

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
                  'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
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
          </SectionGroup>

          {/* ── Connections ───────────────────────────────────── */}
          <SectionGroup label="Connections">
            <SettingsSection icon={<Smartphone className="w-4 h-4" />} title="Mobile Pairing">
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm text-gray-700 dark:text-gray-300">
                    Pair your phone with Claude View for on-the-go access.
                  </p>
                  <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                    Scan a QR code to securely connect the mobile app.
                  </p>
                </div>
                <span className="inline-flex items-center px-2.5 py-1 text-xs font-medium rounded-full bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300">
                  Coming Soon
                </span>
              </div>
            </SettingsSection>

            <SettingsSection icon={<Link2 className="w-4 h-4" />} title="Shared Links">
              <SharedLinksSection />
            </SettingsSection>
          </SectionGroup>

          {/* ── Privacy & About ───────────────────────────────── */}
          <SectionGroup label="Privacy & About">
            <SettingsSection icon={<Shield className="w-4 h-4" />} title="Privacy">
              <TelemetrySection
                telemetryStatus={config.telemetry}
                hasPosHogKey={config.posthogKey !== null}
                onEnable={enableTelemetry}
                onDisable={disableTelemetry}
              />
            </SettingsSection>

            {/* ABOUT */}
          <SettingsSection icon={<Info className="w-4 h-4" />} title="About">
            <div className="flex items-center justify-between mb-4">
              <div>
                <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
                  Claude View v{APP_VERSION}
                </p>
                <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5 tabular-nums">
                  Built {APP_BUILD_DATE}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <a
                  href={GITHUB_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={cn(
                    'inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md',
                    'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900',
                    'hover:bg-gray-800 dark:hover:bg-gray-200 transition-colors',
                  )}
                >
                  <Star className="w-3.5 h-3.5" />
                  Star on GitHub
                </a>
                <a
                  href={`${GITHUB_URL}/releases`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={cn(
                    'inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md',
                    'border border-gray-200 dark:border-gray-700 text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors',
                  )}
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  Releases
                </a>
              </div>
            </div>

            <div className="space-y-5">

              {/* Global */}
              <ShortcutGroup
                title="Global"
                shortcuts={[
                  { keys: [{ mod: true, key: 'K' }], label: 'Command palette' },
                  { keys: [{ mod: true, key: 'B' }], label: 'Toggle sidebar' },
                ]}
              />

              {/* Conversation */}
              <ShortcutGroup
                title="Conversation"
                shortcuts={[
                  { keys: [{ mod: true, key: 'F' }], label: 'Find in conversation' },
                  { keys: [{ mod: true, shift: true, key: 'E' }], label: 'Export HTML' },
                  { keys: [{ mod: true, shift: true, key: 'P' }], label: 'Export PDF' },
                  { keys: [{ mod: true, shift: true, key: 'R' }], label: 'Copy resume command' },
                ]}
              />

              {/* Chat Tabs */}
              <ShortcutGroup
                title="Chat Tabs"
                shortcuts={[
                  { keys: [{ ctrl: true, key: 'T' }], label: 'New tab' },
                  { keys: [{ ctrl: true, key: 'W' }], label: 'Close tab' },
                  { keys: [{ ctrl: true, key: 'Tab' }], label: 'Next tab' },
                  { keys: [{ ctrl: true, shift: true, key: 'Tab' }], label: 'Previous tab' },
                  { keys: [{ ctrl: true, key: '\\' }], label: 'Split right' },
                  { keys: [{ ctrl: true, shift: true, key: '\\' }], label: 'Split down' },
                ]}
              />

              {/* Live Monitor */}
              <ShortcutGroup
                title="Live Monitor"
                shortcuts={[
                  {
                    keys: [{ key: 'j' }, { key: 'k' }],
                    label: 'Next / previous session',
                    separator: '/',
                  },
                  {
                    keys: [{ key: '1' }, { key: '2' }, { key: '3' }, { key: '4' }],
                    label: 'Switch view',
                    separator: '/',
                  },
                  { keys: [{ key: '/' }], label: 'Search' },
                  { keys: [{ key: '?' }], label: 'Show all shortcuts' },
                ]}
              />
            </div>
          </SettingsSection>
          </SectionGroup>

          {/* ── Danger Zone — always last, outside all groups ── */}
          <DangerZoneSection />
        </div>
      </div>
    </div>
  )
}
