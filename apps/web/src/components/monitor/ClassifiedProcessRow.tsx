import { MoreHorizontal, Skull } from 'lucide-react'
import { useState } from 'react'
import { formatBytes, formatUptime } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { EcosystemTag } from '../../types/generated/EcosystemTag'
import type { Staleness } from '../../types/generated/Staleness'

interface ClassifiedProcessRowProps {
  process: ClassifiedProcess
  onKill: (pid: number, startTime: number, force: boolean) => void
  depth?: number
}

function tagBadge(tag: EcosystemTag | null | undefined): string {
  switch (tag) {
    case 'cli': return 'CLI'
    case 'ide': return 'IDE'
    case 'desktop': return 'App'
    case 'self': return 'Us'
    default: return ''
  }
}

function stalenessDot(staleness: Staleness): { color: string; title: string } {
  switch (staleness) {
    case 'Active': return { color: 'bg-green-500', title: 'Active' }
    case 'Idle': return { color: 'bg-gray-400', title: 'Idle' }
    case 'LikelyStale': return { color: 'bg-amber-500', title: 'Likely stale' }
  }
}

export function ClassifiedProcessRow({ process: proc, onKill, depth = 0 }: ClassifiedProcessRowProps) {
  const [confirmKill, setConfirmKill] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const dot = stalenessDot(proc.staleness)

  return (
    <div
      className="flex items-center gap-2 px-3 py-1.5 text-sm hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
      style={{ paddingLeft: depth > 0 ? `${12 + depth * 16}px` : undefined }}
    >
      <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${dot.color}`} title={dot.title} />
      <span className="tabular-nums text-xs text-gray-400 dark:text-gray-500 w-12 shrink-0">
        {proc.pid}
      </span>
      <span className="text-gray-700 dark:text-gray-300 truncate min-w-0 flex-shrink">
        {proc.name}
      </span>
      {proc.ecosystemTag && (
        <span className="text-xs px-1 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 shrink-0">
          {tagBadge(proc.ecosystemTag)}
        </span>
      )}
      {proc.isUnparented && (
        <span className="text-xs px-1 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 shrink-0">
          orphan
        </span>
      )}
      <div className="flex-1" />
      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-12 text-right shrink-0">
        {proc.cpuPercent.toFixed(1)}%
      </span>
      <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums w-16 text-right shrink-0">
        {formatBytes(proc.memoryBytes)}
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums w-10 text-right shrink-0">
        {formatUptime(proc.uptimeSecs)}
      </span>
      {confirmKill ? (
        <div className="flex items-center gap-1 shrink-0">
          <span className="text-xs text-red-600 dark:text-red-400">Kill {proc.pid}?</span>
          <button
            type="button"
            onClick={() => { onKill(proc.pid, proc.startTime, false); setConfirmKill(false) }}
            className="text-xs px-1.5 py-0.5 rounded bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 hover:bg-red-200 dark:hover:bg-red-800/40"
          >
            Yes
          </button>
          <button
            type="button"
            onClick={() => setConfirmKill(false)}
            className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
          >
            No
          </button>
        </div>
      ) : (
        <div className="relative shrink-0">
          <button
            type="button"
            disabled={proc.isSelf}
            title={proc.isSelf ? 'Cannot kill this server process' : 'Process actions'}
            onClick={() => setMenuOpen(!menuOpen)}
            className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 dark:text-gray-500 disabled:opacity-30 disabled:cursor-not-allowed"
          >
            <MoreHorizontal className="w-3.5 h-3.5" />
          </button>
          {menuOpen && (
            <>
              <div className="fixed inset-0 z-10" onClick={() => setMenuOpen(false)} />
              <div className="absolute right-0 top-full mt-1 z-20 rounded-md shadow-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 py-1 min-w-[120px]">
                <button
                  type="button"
                  onClick={() => { setMenuOpen(false); setConfirmKill(true) }}
                  className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
                >
                  <Skull className="w-3 h-3" />
                  Terminate
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  )
}
