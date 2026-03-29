import { formatBytes } from '../../lib/format-utils'
import type { ComponentStatus } from '../../types/generated/ComponentStatus'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { SessionRollupBar } from './SessionRollupBar'

interface ComponentRowProps {
  component: ComponentStatus
  systemInfo: SystemInfo
  totalVramBytes?: number | null
}

export function ComponentRow({ component: c, systemInfo, totalVramBytes }: ComponentRowProps) {
  const kindLabel: Record<string, string> = {
    server: 'server',
    sidecar: 'child process',
    omlx: 'external',
  }
  const label = kindLabel[c.details.type] ?? 'child process'

  const statusDot = c.running
    ? 'bg-green-500 dark:bg-green-400'
    : c.enabled
      ? 'bg-amber-500 dark:bg-amber-400 animate-pulse'
      : 'bg-gray-400 dark:bg-gray-600'

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 pl-11 text-sm">
      <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${statusDot}`} />

      <span className="font-medium text-gray-700 dark:text-gray-300 truncate min-w-0">
        {c.name}
      </span>

      <span className="text-xs text-gray-400 dark:text-gray-500 shrink-0">{label}</span>

      {!c.enabled && (
        <span className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 shrink-0">
          OFF
        </span>
      )}

      {c.pid != null && (
        <span className="text-xs tabular-nums text-gray-400 dark:text-gray-500 shrink-0">
          PID:{c.pid}
        </span>
      )}

      {/* Session count badge — sidecar only */}
      {c.details.type === 'sidecar' && c.details.sessionCount != null && (
        <span className="text-xs tabular-nums px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 shrink-0">
          {c.details.sessionCount} session{c.details.sessionCount !== 1 ? 's' : ''}
        </span>
      )}

      <div className="flex items-center gap-4 shrink-0 ml-auto">
        <div className="w-40">
          <SessionRollupBar label="CPU" value={c.cpuPercent} max={systemInfo.cpuCoreCount * 100} />
        </div>
        <div className="w-px h-3 bg-gray-200 dark:bg-gray-700" />
        <div className="w-40">
          <SessionRollupBar
            label="RAM"
            value={c.memoryBytes}
            max={systemInfo.totalMemoryBytes}
            formatValue={(v) => formatBytes(v)}
          />
        </div>
        {c.vramBytes != null && totalVramBytes != null && totalVramBytes > 0 && (
          <>
            <div className="w-px h-3 bg-gray-200 dark:bg-gray-700" />
            <div className="w-40">
              <SessionRollupBar
                label="VRAM"
                value={c.vramBytes}
                max={totalVramBytes}
                formatValue={(v) => formatBytes(v)}
                color="purple"
              />
            </div>
          </>
        )}
      </div>
    </div>
  )
}
