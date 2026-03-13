import { useState } from 'react'
import { formatBytes } from '../../lib/format-utils'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { ProcessRow } from './ProcessRow'
import { SessionRollupBar } from './SessionRollupBar'

interface TopProcessesPanelProps {
  processes: ProcessGroup[]
  systemInfo: SystemInfo | null
}

const DEFAULT_VISIBLE = 5

export function TopProcessesPanel({ processes, systemInfo }: TopProcessesPanelProps) {
  const [expanded, setExpanded] = useState(false)
  const hasMore = processes.length > DEFAULT_VISIBLE
  const visible = expanded ? processes : processes.slice(0, DEFAULT_VISIBLE)

  const totalCpu = processes.reduce((sum, p) => sum + p.cpuPercent, 0)
  const totalMem = processes.reduce((sum, p) => sum + p.memoryBytes, 0)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
      {/* Header — matches Claude Sessions header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Top Processes</h2>
        <span className="text-xs bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-400 px-1.5 py-0.5 rounded-full font-medium">
          {processes.length}
        </span>

        {systemInfo && (
          <div className="flex items-center gap-4 ml-2">
            <div className="w-28">
              <SessionRollupBar label="CPU" value={totalCpu} max={systemInfo.cpuCoreCount * 100} />
            </div>
            <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
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

        {hasMore && (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          >
            {expanded ? 'Show Less' : `Show All (${processes.length})`}
          </button>
        )}
      </div>

      <div className="flex flex-col">
        {visible.map((proc) => (
          <ProcessRow key={proc.name} process={proc} systemInfo={systemInfo} />
        ))}
      </div>
    </div>
  )
}
