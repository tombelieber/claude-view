import type { Device } from '@claude-view/shared'
import { AlertCircle, Loader2, RefreshCcw, Smartphone } from 'lucide-react'
import { DeviceRow } from './DeviceRow'

interface DeviceListProps {
  devices: Device[] | undefined
  isLoading: boolean
  error: Error | null
  onRevoke: (deviceId: string) => void
  revokingId: string | null
  onPairNew: () => void
  onTerminateOthers: () => void
  terminateOthersPending: boolean
}

/**
 * Presentational list of paired devices + action buttons.
 *
 * The list is sorted server-side by `last_seen_at DESC` but we also
 * render revoked devices (dimmed, at the bottom) so the user can see
 * the full history; they just can't revoke again.
 *
 * Trust-over-accuracy: `last_seen_at` is only rendered when the
 * device is active. For revoked devices we show the revoked time.
 * If the server hasn't reported `last_seen_at` yet we show "Never".
 */
export function DeviceList({
  devices,
  isLoading,
  error,
  onRevoke,
  revokingId,
  onPairNew,
  onTerminateOthers,
  terminateOthersPending,
}: DeviceListProps) {
  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Loading devices…</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex items-start gap-2 rounded-md border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 p-3">
        <AlertCircle className="w-4 h-4 text-red-500 flex-shrink-0 mt-0.5" />
        <div>
          <p className="text-sm font-medium text-red-700 dark:text-red-300">
            Couldn't load devices
          </p>
          <p className="text-xs text-red-600 dark:text-red-400 mt-0.5">{error.message}</p>
        </div>
      </div>
    )
  }

  const list = devices ?? []
  const active = list.filter((d) => d.revoked_at === null)
  const revoked = list.filter((d) => d.revoked_at !== null)
  const hasMultipleActive = active.length > 1

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-xs text-gray-500 dark:text-gray-400">
          {active.length === 0
            ? 'No devices paired yet.'
            : `${active.length} active device${active.length === 1 ? '' : 's'}`}
        </p>
        <button
          type="button"
          onClick={onPairNew}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200 transition-colors"
        >
          <Smartphone className="w-4 h-4" />
          Pair a new device
        </button>
      </div>

      {list.length > 0 && (
        <div className="divide-y divide-gray-100 dark:divide-gray-800 border border-gray-200 dark:border-gray-700 rounded-md">
          {active.map((device) => (
            <DeviceRow
              key={device.device_id}
              device={device}
              onRevoke={onRevoke}
              isRevoking={revokingId === device.device_id}
            />
          ))}
          {revoked.map((device) => (
            <DeviceRow
              key={device.device_id}
              device={device}
              onRevoke={onRevoke}
              isRevoking={false}
            />
          ))}
        </div>
      )}

      {hasMultipleActive && (
        <div className="flex items-start justify-between gap-3 rounded-md border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/20 p-3">
          <div>
            <p className="text-sm font-medium text-red-700 dark:text-red-300">
              Sign out all other devices
            </p>
            <p className="text-xs text-red-600 dark:text-red-400 mt-0.5">
              Revoke every paired device except this one. You'll have to pair them again.
            </p>
          </div>
          <button
            type="button"
            onClick={onTerminateOthers}
            disabled={terminateOthersPending}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
          >
            {terminateOthersPending ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <RefreshCcw className="w-4 h-4" />
            )}
            Sign out others
          </button>
        </div>
      )}
    </div>
  )
}
