import { ChevronDown, ChevronRight } from 'lucide-react'
import { formatBytes } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { SessionResource } from '../../types/generated/SessionResource'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { type LiveSession, sessionTotalCost } from '../live/use-live-sessions'
import { SourceBadge } from '../shared/SourceBadge'
import { ChildProcessRow } from './ChildProcessRow'
import { SessionRollupBar } from './SessionRollupBar'

interface SessionAccordionRowProps {
  session: LiveSession
  resource: SessionResource
  ecosystemProcess: ClassifiedProcess | null
  systemInfo: SystemInfo
  expanded: boolean
  onToggle: () => void
  onKill: (pid: number, startTime: number, force: boolean) => void
  pendingPids: Set<number>
  /** When true, force all nested child processes open. */
  expandAll?: boolean
  /** When true, process tree hasn't loaded yet — show skeleton placeholders. */
  processTreePending?: boolean
}

function statusDotClass(status: LiveSession['status']): string {
  switch (status) {
    case 'working':
      return 'bg-green-500 animate-pulse'
    case 'paused':
      return 'bg-amber-500'
    default:
      return 'bg-gray-400'
  }
}

export function SessionAccordionRow({
  session,
  resource,
  ecosystemProcess,
  systemInfo,
  expanded,
  onToggle,
  onKill,
  pendingPids,
  expandAll = false,
  processTreePending = false,
}: SessionAccordionRowProps) {
  const rollupCpu = ecosystemProcess
    ? resource.cpuPercent + ecosystemProcess.descendantCpu
    : resource.cpuPercent
  const rollupMem = ecosystemProcess
    ? resource.memoryBytes + ecosystemProcess.descendantMemory
    : resource.memoryBytes

  const cost = sessionTotalCost(session)
  const descendantCount = ecosystemProcess?.descendantCount ?? 0

  return (
    <div className="border-b border-gray-100 dark:border-gray-800">
      <div className="flex items-center gap-2 px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
        {descendantCount > 0 ? (
          <button
            type="button"
            aria-label="Toggle session details"
            onClick={onToggle}
            className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 shrink-0"
          >
            {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
          </button>
        ) : processTreePending ? (
          <div className="w-5 h-5 rounded bg-gray-100 dark:bg-gray-800 animate-pulse shrink-0" />
        ) : (
          <div className="w-5 shrink-0" />
        )}

        <div
          data-testid="status-dot"
          className={`w-2 h-2 rounded-full shrink-0 ${statusDotClass(session.status)}`}
        />

        {/* Text section — truncates when space is tight */}
        <div className="min-w-0 flex-1 flex items-center gap-2 overflow-hidden">
          <span className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
            {session.projectDisplayName}
          </span>

          {session.effectiveBranch && (
            <span className="text-xs text-gray-500 dark:text-gray-400 truncate">
              {session.effectiveBranch}
            </span>
          )}

          {session.source ? (
            <SourceBadge source={session.source} />
          ) : processTreePending ? (
            <span className="inline-block h-4 w-8 rounded bg-gray-100 dark:bg-gray-800 animate-pulse shrink-0" />
          ) : null}
        </div>

        <span className="text-xs font-medium tabular-nums text-gray-700 dark:text-gray-300 shrink-0">
          ${cost.toFixed(2)}
        </span>

        {descendantCount > 0 ? (
          <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded-full shrink-0">
            {descendantCount} proc{descendantCount !== 1 ? 's' : ''}
          </span>
        ) : processTreePending ? (
          <span className="inline-block h-4 w-14 rounded-full bg-gray-100 dark:bg-gray-800 animate-pulse shrink-0" />
        ) : null}

        <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 shrink-0">
          T:{session.turnCount}
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

      {!expanded && descendantCount === 0 && processTreePending && (
        <div className="pl-11 pb-1">
          <span className="inline-block h-3 w-24 rounded bg-gray-100 dark:bg-gray-800 animate-pulse" />
        </div>
      )}

      {expanded && ecosystemProcess === null && session.pid === null && (
        <div className="pl-11 py-2 text-xs text-gray-400 dark:text-gray-500">
          PID unknown — process tree unavailable
        </div>
      )}

      {expanded && ecosystemProcess === null && session.pid !== null && (
        <div className="pl-11 py-2 animate-pulse text-xs text-gray-400 dark:text-gray-500">
          Loading child processes...
        </div>
      )}

      {expanded &&
        ecosystemProcess?.descendants.map((child) => (
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
