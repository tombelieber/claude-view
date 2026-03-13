import { ChevronDown, ChevronRight } from 'lucide-react'
import { formatBytes } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { ChildProcessRow } from './ChildProcessRow'
import { SessionRollupBar } from './SessionRollupBar'

interface SelfAppRowProps {
  process: ClassifiedProcess
  systemInfo: SystemInfo
  expanded: boolean
  onToggle: () => void
  onKill: (pid: number, startTime: number, force: boolean) => void
  pendingPids: Set<number>
  expandAll?: boolean
}

export function SelfAppRow({
  process: proc,
  systemInfo,
  expanded,
  onToggle,
  onKill,
  pendingPids,
  expandAll = false,
}: SelfAppRowProps) {
  const rollupCpu = proc.cpuPercent + proc.descendantCpu
  const rollupMem = proc.memoryBytes + proc.descendantMemory
  const descendantCount = proc.descendantCount

  return (
    <div className="border-b border-gray-100 dark:border-gray-800">
      <div className="flex items-center gap-2 px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
        {descendantCount > 0 ? (
          <button
            type="button"
            aria-label="Toggle app details"
            onClick={onToggle}
            className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 shrink-0"
          >
            {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
          </button>
        ) : (
          <div className="w-5 shrink-0" />
        )}

        <div className="w-2 h-2 rounded-full shrink-0 bg-green-500" />

        <div className="min-w-0 flex-1 flex items-center gap-2 overflow-hidden">
          <span className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
            claude-view
          </span>

          <span className="text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 shrink-0">
            This App
          </span>
        </div>

        {descendantCount > 0 && (
          <span className="text-[10px] tabular-nums text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded-full shrink-0">
            {descendantCount} proc{descendantCount !== 1 ? 's' : ''}
          </span>
        )}

        <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 shrink-0">
          PID:{proc.pid}
        </span>

        {/* CPU | RAM — right-aligned */}
        <div className="flex items-center gap-4 shrink-0 ml-auto">
          <div className="w-56">
            <SessionRollupBar label="CPU" value={rollupCpu} max={systemInfo.cpuCoreCount * 100} />
          </div>
          <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
          <div className="w-56">
            <SessionRollupBar
              label="RAM"
              value={rollupMem}
              max={systemInfo.totalMemoryBytes}
              formatValue={(v) => formatBytes(v)}
            />
          </div>
        </div>
      </div>

      {!expanded && descendantCount > 0 && (
        <div className="pl-11 pb-1 text-xs text-gray-400 dark:text-gray-500">
          {'\u2570\u2500'} {descendantCount} child proc{descendantCount !== 1 ? 's' : ''}
        </div>
      )}

      {expanded &&
        proc.descendants.map((child) => (
          <ChildProcessRow
            key={child.pid}
            process={child}
            systemInfo={systemInfo}
            onKill={onKill}
            pendingPids={pendingPids}
            expandAll={expandAll}
          />
        ))}
    </div>
  )
}
