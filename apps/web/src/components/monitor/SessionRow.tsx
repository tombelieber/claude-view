import { useTweenedValue } from '../../hooks/use-tweened-value'
import type { SessionResource } from '../../types/generated/SessionResource'

interface SessionRowProps {
  resource: SessionResource
  /** Session status from live session data */
  status?: 'working' | 'paused' | 'done'
  /** Project display name */
  projectName?: string
  /** Git branch */
  branch?: string
  /** Cost in USD */
  costUsd?: number
  /** Token count */
  totalTokens?: number
  /** Turn count */
  turnCount?: number
}

function statusColor(status?: string): { dot: string; text: string } {
  switch (status) {
    case 'working':
      return { dot: 'bg-green-500 animate-pulse', text: 'text-green-600 dark:text-green-400' }
    case 'paused':
      return { dot: 'bg-amber-500', text: 'text-amber-600 dark:text-amber-400' }
    default:
      return { dot: 'bg-gray-400', text: 'text-gray-500 dark:text-gray-400' }
  }
}

function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`
  return `${(bytes / 1e3).toFixed(0)} KB`
}

export function SessionRow({
  resource,
  status,
  projectName,
  branch,
  costUsd,
  totalTokens,
  turnCount,
}: SessionRowProps) {
  const cpuTweened = useTweenedValue(resource.cpuPercent)
  const colors = statusColor(status)

  return (
    <div className="flex flex-col gap-1 px-3 py-2 rounded-md border border-gray-100 dark:border-gray-800 bg-white dark:bg-gray-900 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors">
      {/* Line 1: status dot + project + CPU bar + RAM */}
      <div className="flex items-center gap-2">
        <span className={`w-2 h-2 rounded-full shrink-0 ${colors.dot}`} />
        <span className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
          {projectName || resource.sessionId.slice(0, 8)}
        </span>
        <div className="flex-1 mx-2">
          <div className="h-1.5 rounded-full bg-gray-100 dark:bg-gray-800 overflow-hidden">
            <div
              className="h-full rounded-full bg-blue-500 transition-colors"
              style={{ width: `${Math.min(cpuTweened, 100)}%` }}
            />
          </div>
        </div>
        <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-14 text-right">
          {resource.cpuPercent.toFixed(1)}% CPU
        </span>
        <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-16 text-right">
          {formatBytes(resource.memoryBytes)}
        </span>
      </div>
      {/* Line 2: metadata */}
      <div className="flex items-center gap-3 pl-4 text-xs text-gray-400 dark:text-gray-500">
        {branch && <span className="truncate max-w-[120px]">{branch}</span>}
        {totalTokens != null && <span>{totalTokens.toLocaleString()} tokens</span>}
        {costUsd != null && <span>${costUsd.toFixed(2)}</span>}
        {turnCount != null && <span>{turnCount} turns</span>}
        <span className="ml-auto tabular-nums">PID {resource.pid}</span>
      </div>
    </div>
  )
}
