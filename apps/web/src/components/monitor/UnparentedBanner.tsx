import { AlertTriangle, Trash2 } from 'lucide-react'
import { formatBytes } from '../../lib/format-utils'
import type { ClassifiedProcess } from '../../types/generated/ClassifiedProcess'
import type { ProcessTreeTotals } from '../../types/generated/ProcessTreeTotals'

interface UnparentedBannerProps {
  totals: ProcessTreeTotals
  allProcesses: ClassifiedProcess[]
  onCleanup: (targets: Array<{ pid: number; startTime: number }>) => void
}

export function UnparentedBanner({ totals, allProcesses, onCleanup }: UnparentedBannerProps) {
  if (totals.unparentedCount === 0) return null

  function handleCleanup() {
    const stale = allProcesses.filter((p) => p.isUnparented && p.staleness === 'LikelyStale' && !p.isSelf)
    onCleanup(stale.map((p) => ({ pid: p.pid, startTime: p.startTime })))
  }

  return (
    <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-sm">
      <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0" />
      <span className="text-amber-700 dark:text-amber-300 flex-1">
        {totals.unparentedCount} unparented process{totals.unparentedCount !== 1 ? 'es' : ''} using {formatBytes(totals.unparentedMemory)}
      </span>
      <button
        type="button"
        onClick={handleCleanup}
        className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 hover:bg-amber-200 dark:hover:bg-amber-800/50 transition-colors"
      >
        <Trash2 className="w-3 h-3" />
        Clean up stale
      </button>
    </div>
  )
}
