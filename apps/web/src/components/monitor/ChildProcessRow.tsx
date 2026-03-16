import { ChevronDown, ChevronRight, X } from 'lucide-react'
import { useState } from 'react'
import { formatBytes, formatUptime } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { SystemInfo } from '../../types/generated/SystemInfo'

const MAX_DEPTH = 3

const GENERIC_RUNTIMES = new Set([
  'node',
  'python',
  'python3',
  'ruby',
  'java',
  'perl',
  'deno',
  'bun',
  'bash',
  'sh',
  'zsh',
  'tail',
  'cat',
  'grep',
])

/** Extract a short label from command line for generic process names. */
function processLabel(name: string, command: string): string {
  if (!command || !GENERIC_RUNTIMES.has(name)) return name

  const parts = command.split(/\s+/)
  // Skip executable path, find first non-flag argument
  for (let i = 1; i < parts.length; i++) {
    const part = parts[i]
    if (!part.startsWith('-')) {
      const basename = part.split('/').pop() ?? part
      if (basename && basename !== name) {
        return `${name} · ${basename}`
      }
      break
    }
  }
  return name
}

interface ChildProcessRowProps {
  process: ClassifiedProcess
  systemInfo: SystemInfo
  onKill: (pid: number, startTime: number, force: boolean) => void
  pendingPids: Set<number>
  depth?: number
  /** When true, force all nested children open (driven by "Expand All"). */
  expandAll?: boolean
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
  pendingPids,
  depth = 0,
  expandAll = false,
}: ChildProcessRowProps) {
  const [confirmKill, setConfirmKill] = useState(false)
  const [localExpanded, setLocalExpanded] = useState(false)

  const isPending = pendingPids.has(proc.pid)
  const normalizedCpu = proc.cpuPercent / systemInfo.cpuCoreCount
  const clampedCpu = Math.min(normalizedCpu, 100)
  const hasChildren = proc.descendants.length > 0 && depth < MAX_DEPTH
  const expanded = localExpanded || expandAll

  return (
    <>
      <div
        className="group flex items-center gap-2 py-1.5 text-sm hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
        style={{ paddingLeft: `${12 + depth * 20}px`, paddingRight: 12 }}
      >
        {hasChildren ? (
          <button
            type="button"
            aria-label="Toggle child processes"
            onClick={() => setLocalExpanded((prev) => !prev)}
            className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 shrink-0"
          >
            {expanded ? (
              <ChevronDown className="w-3.5 h-3.5" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5" />
            )}
          </button>
        ) : (
          <div className="w-5 shrink-0" />
        )}

        <span
          className="text-gray-700 dark:text-gray-300 truncate min-w-0 flex-shrink"
          title={proc.command}
        >
          {processLabel(proc.name, proc.command)}
        </span>
        {proc.ecosystemTag === 'sidecar' && (
          <span className="text-[10px] font-semibold uppercase px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
            Agent SDK
          </span>
        )}
        <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums shrink-0">
          {proc.pid}
        </span>

        {hasChildren && !expanded && (
          <span className="text-[10px] text-gray-400 dark:text-gray-500 shrink-0">
            +{proc.descendants.length}
          </span>
        )}

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

        {isPending ? (
          <span className="text-xs text-amber-600 dark:text-amber-400 animate-pulse shrink-0">
            Killing {proc.pid}…
          </span>
        ) : confirmKill ? (
          <div className="flex items-center gap-1 shrink-0">
            <span className="text-xs text-red-600 dark:text-red-400">Kill {proc.pid}?</span>
            <button
              type="button"
              onClick={() => {
                onKill(proc.pid, proc.startTime, false)
                setConfirmKill(false)
              }}
              className="text-xs px-1.5 py-0.5 rounded bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 hover:bg-red-200 dark:hover:bg-red-800/40"
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

      {expanded &&
        proc.descendants.map((child) => (
          <ChildProcessRow
            key={child.pid}
            process={child}
            systemInfo={systemInfo}
            onKill={onKill}
            pendingPids={pendingPids}
            depth={depth + 1}
            expandAll={expandAll}
          />
        ))}
    </>
  )
}
