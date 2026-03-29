import { ChevronDown, ChevronRight } from 'lucide-react'
import { formatBytes } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { ComponentSnapshot } from '../../types/generated/ComponentSnapshot'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { ChildProcessRow } from './ChildProcessRow'
import { ComponentRow } from './ComponentRow'
import { SessionRollupBar } from './SessionRollupBar'

interface ComponentDashboardProps {
  process: ClassifiedProcess
  systemInfo: SystemInfo
  components?: ComponentSnapshot | null
  expanded: boolean
  onToggle: () => void
  onKill: (pid: number, startTime: number, force: boolean) => void
  pendingPids: Set<number>
  expandAll?: boolean
}

export function ComponentDashboard({
  process: proc,
  systemInfo,
  components,
  expanded,
  onToggle,
  onKill,
  pendingPids,
  expandAll = false,
}: ComponentDashboardProps) {
  // Header = self process + all component rows (sidecar, omlx).
  // Component rows = breakdown. Header is the rollup.
  const componentCpu = components?.components.reduce((s, c) => s + c.cpuPercent, 0) ?? 0
  const componentMem = components?.components.reduce((s, c) => s + c.memoryBytes, 0) ?? 0
  const rollupCpu = proc.cpuPercent + componentCpu
  const rollupMem = proc.memoryBytes + componentMem
  const componentCount = components?.components.length ?? proc.descendantCount
  const rollupVram = components
    ? components.components.reduce((sum, c) => sum + (c.vramBytes ?? 0), 0)
    : 0
  const totalVramBytes = components?.totalVramBytes ?? 0
  const showVramColumn = rollupVram > 0 && totalVramBytes > 0

  return (
    <div className="border-b border-gray-100 dark:border-gray-800">
      {/* Header row */}
      <div className="flex items-center gap-2 px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
        <button
          type="button"
          aria-label="Toggle component details"
          onClick={onToggle}
          className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 shrink-0"
        >
          {expanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
        </button>

        <div className="w-2 h-2 rounded-full shrink-0 bg-green-500 dark:bg-green-400" />

        <div className="min-w-0 flex-1 flex items-center gap-2 overflow-hidden">
          <span className="font-medium text-sm text-gray-900 dark:text-gray-100 truncate">
            claude-view
          </span>

          <span className="text-xs font-semibold uppercase px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 shrink-0">
            This App
          </span>

          {components?.buildMode === 'debug' && (
            <span className="text-xs px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 shrink-0">
              debug build
            </span>
          )}
        </div>

        {componentCount > 0 && (
          <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded-full shrink-0">
            {componentCount} component{componentCount !== 1 ? 's' : ''}
          </span>
        )}

        <span className="text-xs tabular-nums text-gray-500 dark:text-gray-400 shrink-0">
          PID:{proc.pid}
        </span>

        <div className="flex items-center gap-4 shrink-0 ml-auto">
          {showVramColumn && (
            <>
              <div className="w-56">
                <SessionRollupBar
                  label="VRAM"
                  value={rollupVram}
                  max={totalVramBytes}
                  formatValue={(v) => formatBytes(v)}
                  color="purple"
                />
              </div>
              <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
            </>
          )}
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

      {/* Collapsed hint */}
      {!expanded && componentCount > 0 && (
        <div className="pl-11 pb-1 text-xs text-gray-400 dark:text-gray-500">
          {'\u2570\u2500'} {componentCount} component
          {componentCount !== 1 ? 's' : ''}
        </div>
      )}

      {/* Expanded: component rows + child process rows */}
      {expanded && (
        <>
          {components?.components.map((comp) => (
            <ComponentRow
              key={comp.name}
              component={comp}
              systemInfo={systemInfo}
              totalVramBytes={components.totalVramBytes}
              showVramColumn={showVramColumn}
            />
          ))}
          {proc.descendants.map((child) => (
            <ChildProcessRow
              key={child.pid}
              process={child}
              systemInfo={systemInfo}
              onKill={onKill}
              pendingPids={pendingPids}
              expandAll={expandAll}
            />
          ))}
        </>
      )}
    </div>
  )
}
