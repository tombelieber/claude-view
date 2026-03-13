import { Box } from 'lucide-react'
import { formatBytes } from '../../lib/format-utils'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { SessionRollupBar } from './SessionRollupBar'

interface ProcessRowProps {
  process: ProcessGroup
  systemInfo: SystemInfo | null
}

export function ProcessRow({ process, systemInfo }: ProcessRowProps) {
  const cpuMax = (systemInfo?.cpuCoreCount ?? 1) * 100
  const memMax = systemInfo?.totalMemoryBytes ?? 1

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-sm">
      <Box className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0" />
      <span className="text-gray-700 dark:text-gray-300 truncate min-w-0 flex-shrink">
        {process.name}
      </span>
      {process.processCount > 1 && (
        <span className="text-xs text-gray-400 dark:text-gray-500 shrink-0">
          ({process.processCount})
        </span>
      )}

      {/* CPU | RAM — right-aligned */}
      <div className="flex items-center gap-4 shrink-0 ml-auto">
        <div className="w-56">
          <SessionRollupBar label="CPU" value={process.cpuPercent} max={cpuMax} />
        </div>
        <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
        <div className="w-56">
          <SessionRollupBar
            label="RAM"
            value={process.memoryBytes}
            max={memMax}
            formatValue={(v) => formatBytes(v)}
          />
        </div>
      </div>
    </div>
  )
}
