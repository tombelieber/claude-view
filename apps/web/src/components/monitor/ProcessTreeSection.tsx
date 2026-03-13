import { useCallback, useState } from 'react'
import { toast } from 'sonner'
import type { ProcessTreeSnapshot } from '../../types/generated/ProcessTreeSnapshot'
import { ChildProcessTable } from './ChildProcessTable'
import { EcosystemTable } from './EcosystemTable'
import { UnparentedBanner } from './UnparentedBanner'

interface ProcessTreeSectionProps {
  tree: ProcessTreeSnapshot
  freshAt: number | null
}

const STALE_THRESHOLD_MS = 15_000

export function ProcessTreeSection({ tree, freshAt }: ProcessTreeSectionProps) {
  const [killPending, setKillPending] = useState(false)
  const [pendingPids, setPendingPids] = useState<Set<number>>(new Set())

  const isStale = freshAt == null || Date.now() - freshAt > STALE_THRESHOLD_MS

  const handleKill = useCallback(async (pid: number, startTime: number, force: boolean) => {
    setPendingPids((prev) => new Set(prev).add(pid))
    try {
      await fetch(`/api/processes/${pid}/kill`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ start_time: startTime, force }),
      })
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

  const handleCleanup = useCallback(async (targets: Array<{ pid: number; startTime: number }>) => {
    if (targets.length === 0) return
    setKillPending(true)
    try {
      await fetch('/api/processes/cleanup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          targets: targets.map((t) => ({ pid: t.pid, start_time: t.startTime })),
        }),
      })
    } catch (e) {
      toast.error('Failed to clean up processes')
      console.error('Cleanup failed:', e)
    } finally {
      setKillPending(false)
    }
  }, [])

  const allProcesses = [...tree.ecosystem, ...tree.children]

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
          Claude Process Tree
        </h2>
        <span
          className={`inline-flex items-center gap-1.5 text-xs ${
            isStale ? 'text-amber-600 dark:text-amber-400' : 'text-green-600 dark:text-green-400'
          }`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full ${
              isStale ? 'bg-amber-500' : 'bg-green-500 animate-pulse'
            }`}
          />
          {isStale ? 'Stale' : 'Live'}
        </span>
      </div>

      <UnparentedBanner
        totals={tree.totals}
        allProcesses={allProcesses}
        onCleanup={handleCleanup}
        isPending={killPending}
      />

      {killPending && <p className="text-xs text-gray-400 dark:text-gray-500">Sending signals…</p>}

      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
        <div className="flex items-center gap-2 px-4 py-2 border-b border-gray-100 dark:border-gray-800">
          <h3 className="text-xs font-semibold text-gray-700 dark:text-gray-300">
            Claude Ecosystem
          </h3>
          <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 px-1.5 py-0.5 rounded-full">
            {tree.totals.ecosystemCount}
          </span>
          <span className="ml-auto text-xs text-gray-400 dark:text-gray-500">
            {tree.totals.ecosystemCpu.toFixed(1)}% CPU
          </span>
        </div>
        <EcosystemTable processes={tree.ecosystem} onKill={handleKill} pendingPids={pendingPids} />
      </div>

      {tree.children.length > 0 && (
        <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
          <div className="flex items-center gap-2 px-4 py-2 border-b border-gray-100 dark:border-gray-800">
            <h3 className="text-xs font-semibold text-gray-700 dark:text-gray-300">
              Child Processes
            </h3>
            <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 px-1.5 py-0.5 rounded-full">
              {tree.totals.childCount}
            </span>
            <span className="ml-auto text-xs text-gray-400 dark:text-gray-500">
              {tree.totals.childCpu.toFixed(1)}% CPU
            </span>
          </div>
          <ChildProcessTable
            processes={tree.children}
            onKill={handleKill}
            pendingPids={pendingPids}
          />
        </div>
      )}
    </div>
  )
}
