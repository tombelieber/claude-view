import { Activity } from 'lucide-react'
import { useSystemMonitor } from '../hooks/use-system-monitor'

export function SystemMonitorPage() {
  const { status, systemInfo, snapshot } = useSystemMonitor()

  return (
    <div className="flex flex-col gap-4 p-4 max-w-7xl mx-auto">
      {/* Header */}
      <div className="flex items-center gap-3">
        <Activity className="w-6 h-6 text-blue-500" />
        <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">System Monitor</h1>
        <span
          className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium ${
            status === 'connected'
              ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
              : status === 'reconnecting'
                ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                : 'bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400'
          }`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full ${
              status === 'connected'
                ? 'bg-green-500 animate-pulse'
                : status === 'reconnecting'
                  ? 'bg-amber-500 animate-pulse'
                  : 'bg-gray-400'
            }`}
          />
          {status === 'connected'
            ? 'Live'
            : status === 'reconnecting'
              ? 'Reconnecting...'
              : 'Connecting...'}
        </span>
        {systemInfo && (
          <span className="text-sm text-gray-500 dark:text-gray-400 ml-auto">
            {systemInfo.hostname} &middot; {systemInfo.os} &middot; {systemInfo.cpuCoreCount} cores
          </span>
        )}
      </div>

      {/* Content: skeleton or data */}
      {!snapshot ? (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-24 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse" />
          ))}
        </div>
      ) : (
        <div className="space-y-4">
          {/* Gauge row placeholder — will be replaced in Task #6 */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <GaugeSkeleton label="CPU" value={`${snapshot.cpuPercent.toFixed(1)}%`} />
            <GaugeSkeleton
              label="Memory"
              value={`${((snapshot.memoryUsedBytes / snapshot.memoryTotalBytes) * 100).toFixed(1)}%`}
            />
            <GaugeSkeleton
              label="Disk"
              value={`${((snapshot.diskUsedBytes / snapshot.diskTotalBytes) * 100).toFixed(1)}%`}
            />
            <GaugeSkeleton label="Sessions" value={`${snapshot.sessionResources.length}`} />
          </div>
        </div>
      )}
    </div>
  )
}

function GaugeSkeleton({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4">
      <p className="text-xs text-gray-500 dark:text-gray-400 uppercase tracking-wide">{label}</p>
      <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100 mt-1">{value}</p>
    </div>
  )
}
