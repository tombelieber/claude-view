import { ChevronDown, ChevronUp } from 'lucide-react'
import { useState } from 'react'
import type { ProcessGroup } from '../../types/generated/ProcessGroup'
import { ProcessRow } from './ProcessRow'

interface TopProcessesPanelProps {
  processes: ProcessGroup[]
}

const DEFAULT_VISIBLE = 5

export function TopProcessesPanel({ processes }: TopProcessesPanelProps) {
  const [expanded, setExpanded] = useState(false)
  const hasMore = processes.length > DEFAULT_VISIBLE
  const visible = expanded ? processes : processes.slice(0, DEFAULT_VISIBLE)
  const maxCpu = processes.length > 0 ? Math.max(...processes.map((p) => p.cpuPercent)) : 1

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 py-3">
      <div className="flex items-center gap-2 px-4 mb-2">
        <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">Top Processes</h2>
        <span className="text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 px-1.5 py-0.5 rounded-full">
          {processes.length}
        </span>
      </div>
      <div className="flex flex-col">
        {visible.map((proc) => (
          <ProcessRow key={proc.name} process={proc} maxCpu={maxCpu} />
        ))}
      </div>
      {hasMore && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 px-4 pt-2 text-xs text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 transition-colors"
        >
          {expanded ? (
            <>
              <ChevronUp className="w-3 h-3" />
              Show less
            </>
          ) : (
            <>
              <ChevronDown className="w-3 h-3" />
              Show all ({processes.length})
            </>
          )}
        </button>
      )}
    </div>
  )
}
