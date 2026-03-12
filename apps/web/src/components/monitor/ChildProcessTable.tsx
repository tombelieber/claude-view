import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { formatBytes } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import { ClassifiedProcessRow } from './ClassifiedProcessRow'

interface ChildProcessTableProps {
  children: ClassifiedProcess[]
  onKill: (pid: number, startTime: number, force: boolean) => void
}

function ChildProcessGroup({
  proc,
  onKill,
}: {
  proc: ClassifiedProcess
  onKill: (pid: number, startTime: number, force: boolean) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const hasDescendants = proc.descendants.length > 0

  return (
    <div>
      <div className="flex items-center gap-1">
        {hasDescendants ? (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="p-0.5 shrink-0 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          </button>
        ) : (
          <span className="w-4 shrink-0" />
        )}
        <div className="flex-1">
          <ClassifiedProcessRow process={proc} onKill={onKill} />
          {hasDescendants && !expanded && (
            <span className="text-xs text-gray-400 dark:text-gray-500 pl-14">
              +{proc.descendantCount} — {proc.descendantCpu.toFixed(1)}% CPU, {formatBytes(proc.descendantMemory)}
            </span>
          )}
        </div>
      </div>
      {expanded &&
        proc.descendants.map((desc) => (
          <ChildProcessGroup key={desc.pid} proc={desc} onKill={onKill} />
        ))}
    </div>
  )
}

export function ChildProcessTable({ children: procs, onKill }: ChildProcessTableProps) {
  if (procs.length === 0) {
    return (
      <p className="text-sm text-gray-400 dark:text-gray-500 px-3 py-3">
        No child processes detected.
      </p>
    )
  }

  return (
    <div>
      <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-400 dark:text-gray-500 border-b border-gray-100 dark:border-gray-800">
        <span className="w-4 shrink-0" />
        <span className="w-1.5 shrink-0" />
        <span className="w-12 shrink-0">PID</span>
        <span className="truncate flex-1">Command</span>
        <span className="w-12 text-right shrink-0">CPU</span>
        <span className="w-16 text-right shrink-0">Mem</span>
        <span className="w-10 text-right shrink-0">Age</span>
        <span className="w-5 shrink-0" />
      </div>
      {procs.map((proc) => (
        <ChildProcessGroup key={proc.pid} proc={proc} onKill={onKill} />
      ))}
    </div>
  )
}
