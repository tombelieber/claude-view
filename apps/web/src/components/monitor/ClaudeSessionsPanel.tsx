import { AlertTriangle, ChevronDown, ChevronRight } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { formatBytes } from '../../lib/format-utils'
import type { ProcessTreeSnapshot } from '../../types/generated/ProcessTreeSnapshot'
import type { SessionResource } from '../../types/generated/SessionResource'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import type { LiveSession } from '../live/use-live-sessions'
import { ChildProcessRow } from './ChildProcessRow'
import { SessionAccordionRow } from './SessionAccordionRow'
import { SessionRollupBar } from './SessionRollupBar'

interface ClaudeSessionsPanelProps {
  sessionResources: SessionResource[]
  liveSessions: LiveSession[]
  processTree: ProcessTreeSnapshot | null
  systemInfo: SystemInfo | null
}

export function ClaudeSessionsPanel({
  sessionResources,
  liveSessions,
  processTree,
  systemInfo,
}: ClaudeSessionsPanelProps) {
  // --- Two-step merge: sessionId -> LiveSession, PID -> ClassifiedProcess ---
  const sessionMap = new Map(liveSessions.map((s) => [s.id, s]))
  const ecosystemByPid = new Map(processTree?.ecosystem.map((p) => [p.pid, p]) ?? [])

  const merged = sessionResources.map((res) => ({
    resource: res,
    session: sessionMap.get(res.sessionId),
    ecosystem: ecosystemByPid.get(res.pid) ?? null,
  }))

  // --- Sort by rollup CPU desc (safe for NaN) ---
  merged.sort((a, b) => {
    const cpuA = a.resource.cpuPercent + (a.ecosystem?.descendantCpu ?? 0)
    const cpuB = b.resource.cpuPercent + (b.ecosystem?.descendantCpu ?? 0)
    const diff = cpuB - cpuA
    if (Number.isNaN(diff) || diff === 0) {
      return a.resource.sessionId.localeCompare(b.resource.sessionId)
    }
    return diff
  })

  // --- Expand/collapse state ---
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set())
  const [orphansExpanded, setOrphansExpanded] = useState(false)
  const prevSessionIdsRef = useRef<Set<string>>(new Set())

  const allExpanded =
    merged.length > 0 && merged.every((m) => expandedIds.has(m.resource.sessionId))

  // Auto-expand new rows when all *previous* rows were already expanded
  const currentSessionIds = new Set(merged.map((m) => m.resource.sessionId))
  useEffect(() => {
    const prev = prevSessionIdsRef.current
    const allPrevExpanded = prev.size > 0 && [...prev].every((id) => expandedIds.has(id))
    if (allPrevExpanded) {
      const newIds = [...currentSessionIds].filter((id) => !prev.has(id))
      if (newIds.length > 0) {
        setExpandedIds((p) => {
          const next = new Set(p)
          for (const id of newIds) next.add(id)
          return next
        })
      }
    }
    prevSessionIdsRef.current = currentSessionIds
  }) // intentionally no deps — runs every render to track session changes

  function handleToggle(sessionId: string) {
    setExpandedIds((prev) => {
      const next = new Set(prev)
      if (next.has(sessionId)) next.delete(sessionId)
      else next.add(sessionId)
      return next
    })
  }

  function handleExpandCollapseAll() {
    if (allExpanded) {
      setExpandedIds(new Set())
    } else {
      setExpandedIds(new Set(merged.map((m) => m.resource.sessionId)))
    }
  }

  // --- Kill/cleanup state ---
  const [pendingPids, setPendingPids] = useState<Set<number>>(new Set())
  const [cleanupPending, setCleanupPending] = useState(false)

  const handleKill = useCallback(async (pid: number, startTime: number, force: boolean) => {
    setPendingPids((prev) => new Set(prev).add(pid))
    try {
      const resp = await fetch(`/api/processes/${pid}/kill`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ start_time: startTime, force }),
      })
      const data = await resp.json()
      if (data.killed) {
        toast.success(`Process ${pid} terminated`)
      } else {
        toast.error(`Kill failed: ${data.error ?? 'unknown error'}`)
      }
    } catch (e) {
      toast.error('Failed to terminate process')
      console.error('Kill failed:', e)
    } finally {
      setPendingPids((prev) => {
        const next = new Set(prev)
        next.delete(pid)
        return next
      })
    }
  }, [])

  // --- Orphan detection ---
  const allOrphans = [...(processTree?.ecosystem ?? []), ...(processTree?.children ?? [])].filter(
    (p) => p.isUnparented,
  )

  const handleCleanup = useCallback(async () => {
    const stale = allOrphans.filter((p) => p.staleness === 'LikelyStale' && !p.isSelf)
    if (stale.length === 0) return
    setCleanupPending(true)
    try {
      const resp = await fetch('/api/processes/cleanup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          targets: stale.map((p) => ({ pid: p.pid, start_time: p.startTime })),
        }),
      })
      const data = await resp.json()
      const killedCount = data.killed?.length ?? 0
      const failedCount = data.failed?.length ?? 0
      if (killedCount > 0 && failedCount === 0) {
        toast.success(`Cleaned up ${killedCount} process${killedCount > 1 ? 'es' : ''}`)
      } else if (killedCount > 0 && failedCount > 0) {
        toast.warning(`Cleaned ${killedCount}, failed ${failedCount}`)
      } else if (failedCount > 0) {
        toast.error(`Cleanup failed for ${failedCount} process${failedCount > 1 ? 'es' : ''}`)
      }
    } catch (e) {
      toast.error('Failed to clean up processes')
      console.error('Cleanup failed:', e)
    } finally {
      setCleanupPending(false)
    }
  }, [allOrphans])

  // --- Header rollup totals ---
  const totalCpu =
    merged.reduce((sum, m) => sum + m.resource.cpuPercent + (m.ecosystem?.descendantCpu ?? 0), 0) +
    allOrphans.reduce((sum, p) => sum + p.cpuPercent, 0)
  const totalMem =
    merged.reduce(
      (sum, m) => sum + m.resource.memoryBytes + (m.ecosystem?.descendantMemory ?? 0),
      0,
    ) + allOrphans.reduce((sum, p) => sum + p.memoryBytes, 0)

  // --- Empty state ---
  if (sessionResources.length === 0 && allOrphans.length === 0) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-6">
        <div className="flex items-center gap-2 mb-4">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Claude Sessions
          </h2>
          <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 px-1.5 py-0.5 rounded-full">
            0
          </span>
        </div>
        <p className="text-sm text-gray-400 dark:text-gray-500 text-center py-6">
          No active Claude sessions
        </p>
      </div>
    )
  }

  const hasStaleOrphans = allOrphans.some((p) => p.staleness === 'LikelyStale' && !p.isSelf)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Claude Sessions</h2>
        <span className="text-xs bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 px-1.5 py-0.5 rounded-full font-medium">
          {sessionResources.length}
        </span>

        {systemInfo && (
          <div className="flex items-center gap-3 ml-2">
            <div className="w-28">
              <SessionRollupBar label="CPU" value={totalCpu} max={systemInfo.cpuCoreCount * 100} />
            </div>
            <div className="w-28">
              <SessionRollupBar
                label="RAM"
                value={totalMem}
                max={systemInfo.totalMemoryBytes}
                formatValue={(v) => formatBytes(v)}
              />
            </div>
          </div>
        )}

        <div className="flex-1" />

        {merged.length > 0 && (
          <button
            type="button"
            onClick={handleExpandCollapseAll}
            className="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          >
            {allExpanded ? 'Collapse All' : 'Expand All'}
          </button>
        )}
      </div>

      {/* Session rows */}
      <div className="divide-y divide-gray-100 dark:divide-gray-800">
        {merged.map((m) => {
          if (!m.session) {
            // Fallback: resource exists but no live session match
            // Show a minimal row with resource data only
            return (
              <div
                key={m.resource.sessionId}
                className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400"
              >
                Session {m.resource.sessionId.slice(0, 8)} (PID {m.resource.pid}) --
                {m.resource.cpuPercent.toFixed(1)}% CPU, {formatBytes(m.resource.memoryBytes)}
              </div>
            )
          }
          return (
            <SessionAccordionRow
              key={m.resource.sessionId}
              session={m.session}
              resource={m.resource}
              ecosystemProcess={m.ecosystem}
              systemInfo={
                systemInfo ?? {
                  hostname: '',
                  os: '',
                  osVersion: '',
                  arch: '',
                  cpuCoreCount: 1,
                  totalMemoryBytes: 1,
                }
              }
              expanded={expandedIds.has(m.resource.sessionId)}
              onToggle={() => handleToggle(m.resource.sessionId)}
              onKill={handleKill}
              pendingPids={pendingPids}
            />
          )
        })}
      </div>

      {/* Orphaned Processes row */}
      {allOrphans.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-2 px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
            <button
              type="button"
              aria-label="Toggle orphaned processes"
              onClick={() => setOrphansExpanded((prev) => !prev)}
              className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 shrink-0"
            >
              {orphansExpanded ? (
                <ChevronDown className="w-4 h-4" />
              ) : (
                <ChevronRight className="w-4 h-4" />
              )}
            </button>

            <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0" />

            <span className="text-sm font-medium text-amber-700 dark:text-amber-300">
              Orphaned Processes ({allOrphans.length})
            </span>

            {systemInfo && (
              <div className="flex items-center gap-3 ml-2">
                <div className="w-24">
                  <SessionRollupBar
                    label="CPU"
                    value={allOrphans.reduce((s, p) => s + p.cpuPercent, 0)}
                    max={systemInfo.cpuCoreCount * 100}
                  />
                </div>
                <div className="w-24">
                  <SessionRollupBar
                    label="RAM"
                    value={allOrphans.reduce((s, p) => s + p.memoryBytes, 0)}
                    max={systemInfo.totalMemoryBytes}
                    formatValue={(v) => formatBytes(v)}
                  />
                </div>
              </div>
            )}

            <div className="flex-1" />

            <button
              type="button"
              onClick={handleCleanup}
              disabled={!hasStaleOrphans || cleanupPending}
              className="text-xs font-medium px-2 py-1 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 hover:bg-amber-200 dark:hover:bg-amber-800/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Clean up
            </button>
          </div>

          {orphansExpanded && (
            <div className="pl-7">
              {allOrphans.map((orphan) => (
                <ChildProcessRow
                  key={orphan.pid}
                  process={orphan}
                  systemInfo={
                    systemInfo ?? {
                      hostname: '',
                      os: '',
                      osVersion: '',
                      arch: '',
                      cpuCoreCount: 1,
                      totalMemoryBytes: 1,
                    }
                  }
                  onKill={handleKill}
                  pendingPids={pendingPids}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
