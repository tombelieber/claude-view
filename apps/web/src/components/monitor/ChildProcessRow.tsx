import { X } from 'lucide-react'
import { useState } from 'react'
import { formatBytes, formatUptime } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { SystemInfo } from '../../types/generated/SystemInfo'

interface ChildProcessRowProps {
  process: ClassifiedProcess
  systemInfo: SystemInfo
  onKill: (pid: number, startTime: number, force: boolean) => void
  isPending: boolean
}

function cpuBarColor(pct: number): string {
  if (pct >= 90) return 'bg-red-500'
  if (pct >= 70) return 'bg-amber-500'
  return 'bg-green-500'
}

function ageColor(uptimeSecs: number, staleness: string): string {
  if (uptimeSecs < 60) return 'text-green-600 dark:text-green-400'
  if (uptimeSecs >= 300 && staleness !== 'Active') return 'text-amber-600 dark:text-amber-400'
  return 'text-gray-400 dark:text-gray-500'
}

/** A single child-process row showing name, CPU bar, CPU%, RAM, age, and kill button. */
export function ChildProcessRow({
  process: proc,
  systemInfo,
  onKill,
  isPending,
}: ChildProcessRowProps) {
  const [confirmKill, setConfirmKill] = useState(false)

  const normalizedCpu = proc.cpuPercent / systemInfo.cpuCoreCount
  const clampedCpu = Math.min(normalizedCpu, 100)

  return (
    <div className="group flex items-center gap-2 px-3 py-1.5 text-sm hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
      <span className="text-gray-700 dark:text-gray-300 truncate min-w-0 flex-shrink">
        {proc.name}
      </span>

      <div className="flex-1" />

      <div className="w-16 shrink-0 flex items-center gap-1.5">
        <div className="flex-1 h-1 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
          <div
            role="progressbar"
            aria-valuenow={Math.round(normalizedCpu)}
            aria-valuemin={0}
            aria-valuemax={100}
            className={`h-full rounded-full transition-all ${cpuBarColor(normalizedCpu)}`}
            style={{ width: `${clampedCpu}%` }}
          />
        </div>
      </div>

      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-12 text-right shrink-0">
        {normalizedCpu.toFixed(1)}%
      </span>

      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-16 text-right shrink-0">
        {formatBytes(proc.memoryBytes)}
      </span>

      <span
        data-testid="process-age"
        className={`text-xs tabular-nums w-10 text-right shrink-0 ${ageColor(proc.uptimeSecs, proc.staleness)}`}
      >
        {formatUptime(proc.uptimeSecs)}
      </span>

      {confirmKill ? (
        <div className="flex items-center gap-1 shrink-0">
          <span className="text-xs text-red-600 dark:text-red-400">Kill {proc.pid}?</span>
          <button
            type="button"
            disabled={isPending}
            onClick={() => {
              onKill(proc.pid, proc.startTime, false)
              setConfirmKill(false)
            }}
            className="text-xs px-1.5 py-0.5 rounded bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 hover:bg-red-200 dark:hover:bg-red-800/40 disabled:opacity-50 disabled:cursor-wait"
          >
            Yes
          </button>
          <button
            type="button"
            onClick={() => setConfirmKill(false)}
            className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
          >
            No
          </button>
        </div>
      ) : (
        <button
          type="button"
          disabled={proc.isSelf}
          title={proc.isSelf ? 'Cannot kill this server process' : 'Terminate process'}
          onClick={() => setConfirmKill(true)}
          className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 dark:text-gray-500 opacity-0 group-hover:opacity-100 transition-opacity disabled:opacity-30 disabled:cursor-not-allowed disabled:group-hover:opacity-30"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  )
}
