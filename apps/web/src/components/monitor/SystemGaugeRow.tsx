import type { ResourceSnapshot } from '../../types/generated/ResourceSnapshot'
import type { SystemInfo } from '../../types/generated/SystemInfo'
import { GaugeCard } from './GaugeCard'

function formatBytes(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  return `${(bytes / 1e3).toFixed(1)} KB`
}

interface SystemGaugeRowProps {
  snapshot: ResourceSnapshot
  systemInfo: SystemInfo | null
}

export function SystemGaugeRow({ snapshot, systemInfo }: SystemGaugeRowProps) {
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
        detail={`${formatBytes(snapshot.memoryUsedBytes)} / ${formatBytes(snapshot.memoryTotalBytes)}`}
        formatValue={(v) => v.toFixed(1)}
      />
      <GaugeCard
        label="Disk"
        value={diskPct}
        max={100}
        unit="%"
        detail={`${formatBytes(snapshot.diskUsedBytes)} / ${formatBytes(snapshot.diskTotalBytes)}`}
        formatValue={(v) => v.toFixed(1)}
      />
      <GaugeCard
        label="Active Sessions"
        value={snapshot.sessionResources.length}
        max={Math.max(snapshot.sessionResources.length, 1)}
        unit=""
        formatValue={(v) => String(Math.round(v))}
      />
    </div>
  )
}
