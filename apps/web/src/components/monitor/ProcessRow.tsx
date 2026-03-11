import { Box } from 'lucide-react'
import { useTweenedValue } from '../../hooks/use-tweened-value'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'

interface ProcessRowProps {
  process: ProcessGroup
  maxCpu: number
}

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`
  return `${(bytes / 1e3).toFixed(0)} KB`
}

export function ProcessRow({ process, maxCpu }: ProcessRowProps) {
  const barMax = Math.max(maxCpu, 1)
  const barPct = Math.min((process.cpuPercent / barMax) * 100, 100)
  const tweenedPct = useTweenedValue(barPct)

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
      <div className="flex-1 mx-1">
        <div className="h-1 rounded-full bg-gray-100 dark:bg-gray-800 overflow-hidden">
          <div className="h-full rounded-full bg-purple-500" style={{ width: `${tweenedPct}%` }} />
        </div>
      </div>
      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-16 text-right shrink-0">
        {process.cpuPercent.toFixed(1)}%
      </span>
      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-16 text-right shrink-0">
        {formatBytes(process.memoryBytes)}
      </span>
    </div>
  )
}
