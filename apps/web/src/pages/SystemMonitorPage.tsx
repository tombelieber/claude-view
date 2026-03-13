import { Activity } from 'lucide-react'
import { useRef } from 'react'
import { useLiveSessions } from '../components/live/use-live-sessions'
import { ClaudeSessionsPanel } from '../components/monitor/ClaudeSessionsPanel'
import { ProcessTreeSection } from '../components/monitor/ProcessTreeSection'
import { SystemGaugeRow } from '../components/monitor/SystemGaugeRow'
import { TopProcessesPanel } from '../components/monitor/TopProcessesPanel'
import { useSystemMonitor } from '../hooks/use-system-monitor'

export function SystemMonitorPage() {
  const { status, systemInfo, snapshot, processTree, processTreeFreshAt } = useSystemMonitor()
  const { sessions } = useLiveSessions()
  const hasRevealedRef = useRef(false)

  // Track first data arrival for stagger animation
  if (snapshot && !hasRevealedRef.current) {
    hasRevealedRef.current = true
  }
  const shouldAnimate = hasRevealedRef.current

  return (
    <div className="flex flex-col gap-4 p-4 max-w-7xl mx-auto h-full">
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

      {/* Content */}
      {!snapshot ? (
        <div className="space-y-4">
          {/* Gauge skeleton */}
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div
                key={`gauge-skeleton-${i}`}
                className="h-24 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse"
              />
            ))}
          </div>
          {/* Session panel skeleton */}
          <div className="h-48 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse" />
          {/* Process panel skeleton */}
          <div className="h-40 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse" />
        </div>
      ) : (
        <div className="flex flex-col gap-4 flex-1 min-h-0">
          {/* Gauges — sticky at top */}
          <div
            className="sticky top-0 z-10 bg-gray-50 dark:bg-gray-950 -mx-4 px-4 py-2"
            style={shouldAnimate ? { animation: 'monitor-reveal 300ms ease-out both' } : undefined}
          >
            <SystemGaugeRow snapshot={snapshot} systemInfo={systemInfo} />
          </div>

          {/* Claude Sessions */}
          <div
            style={
              shouldAnimate ? { animation: 'monitor-reveal 300ms ease-out 100ms both' } : undefined
            }
          >
            <ClaudeSessionsPanel
              sessionResources={snapshot.sessionResources}
              liveSessions={sessions}
            />
          </div>

          {/* Top Processes */}
          <div
            style={
              shouldAnimate ? { animation: 'monitor-reveal 300ms ease-out 200ms both' } : undefined
            }
          >
            <TopProcessesPanel processes={snapshot.topProcesses} />
          </div>

          {/* Claude Process Tree */}
          {processTree && <ProcessTreeSection tree={processTree} freshAt={processTreeFreshAt} />}
        </div>
      )}

      {/* Stagger animation keyframes */}
      <style>{`
        @keyframes monitor-reveal {
          from {
            opacity: 0;
            transform: translateY(8px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      `}</style>
    </div>
  )
}
