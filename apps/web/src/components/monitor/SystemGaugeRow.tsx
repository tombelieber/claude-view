import type { ResourceSnapshot } from '../../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { GaugeCard } from './GaugeCard'

const KiB = 1024
const MiB = 1024 ** 2
const GiB = 1024 ** 3
const TiB = 1024 ** 4

/** Format bytes using binary units (1024-based) — matches how RAM is marketed. */
function formatMemory(bytes: number): string {
  if (bytes >= TiB) return `${(bytes / TiB).toFixed(1)} TB`
  if (bytes >= GiB) return `${(bytes / GiB).toFixed(1)} GB`
  if (bytes >= MiB) return `${(bytes / MiB).toFixed(1)} MB`
  return `${(bytes / KiB).toFixed(1)} KB`
}

/** Format bytes using decimal units (1000-based) — matches how storage is marketed. */
function formatDisk(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  return `${(bytes / 1e3).toFixed(1)} KB`
}

interface SystemGaugeRowProps {
  snapshot: ResourceSnapshot
  systemInfo: SystemInfo | null
  /** Number of active Claude sessions — passed from parent for single source of truth. */
  activeSessionCount: number
}

export function SystemGaugeRow({ snapshot, systemInfo, activeSessionCount }: SystemGaugeRowProps) {
  const memPct =
    snapshot.memoryTotalBytes > 0 ? (snapshot.memoryUsedBytes / snapshot.memoryTotalBytes) * 100 : 0
  const diskPct =
    snapshot.diskTotalBytes > 0 ? (snapshot.diskUsedBytes / snapshot.diskTotalBytes) * 100 : 0

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <GaugeCard
        label="CPU"
        value={snapshot.cpuPercent}
        max={100}
        unit="%"
        detail={systemInfo ? `${systemInfo.cpuCoreCount} cores` : undefined}
      />
      <GaugeCard
        label="Memory"
        value={memPct}
        max={100}
        unit="%"
        detail={`${formatMemory(snapshot.memoryUsedBytes)} / ${formatMemory(snapshot.memoryTotalBytes)}`}
        formatValue={(v) => v.toFixed(1)}
      />
      <GaugeCard
        label="Disk"
        value={diskPct}
        max={100}
        unit="%"
        detail={`${formatDisk(snapshot.diskUsedBytes)} / ${formatDisk(snapshot.diskTotalBytes)}`}
        formatValue={(v) => v.toFixed(1)}
      />
      <GaugeCard
        label="Active Sessions"
        value={activeSessionCount}
        max={Math.max(activeSessionCount, 1)}
        unit=""
        formatValue={(v) => String(Math.round(v))}
        barColor="bg-blue-500"
        valueColor="text-blue-600 dark:text-blue-400"
      />
    </div>
  )
}
