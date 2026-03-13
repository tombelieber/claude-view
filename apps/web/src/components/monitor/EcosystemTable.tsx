import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import { ClassifiedProcessRow } from './ClassifiedProcessRow'

interface EcosystemTableProps {
  processes: ClassifiedProcess[]
  onKill: (pid: number, startTime: number, force: boolean) => void
}

export function EcosystemTable({ processes, onKill }: EcosystemTableProps) {
  if (processes.length === 0) {
    return (
      <p className="text-sm text-gray-400 dark:text-gray-500 px-3 py-3">
        No Claude processes detected.
      </p>
    )
  }

  return (
    <div>
      <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-400 dark:text-gray-500 border-b border-gray-100 dark:border-gray-800">
        <span className="w-1.5 shrink-0" />
        <span className="w-12 shrink-0">PID</span>
        <span className="truncate flex-1">Name</span>
        <span className="w-12 text-right shrink-0">CPU</span>
        <span className="w-16 text-right shrink-0">Mem</span>
        <span className="w-10 text-right shrink-0">Age</span>
        <span className="w-5 shrink-0" />
      </div>
      {processes.map((proc) => (
        <ClassifiedProcessRow key={proc.pid} process={proc} onKill={onKill} />
      ))}
    </div>
  )
}
