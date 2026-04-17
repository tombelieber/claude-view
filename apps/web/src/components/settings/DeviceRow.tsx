import type { Device, DevicePlatform } from '@claude-view/shared'
import { Loader2, Monitor, Smartphone, Tablet, XCircle } from 'lucide-react'

/**
 * Format an ISO-8601 timestamp as relative time (e.g., "5 minutes ago").
 * Returns "Never" for null, empty, or un-parseable values — trust over accuracy.
 */
export function formatRelativeIso(iso: string | null | undefined): string {
  if (!iso) return 'Never'
  const ms = Date.parse(iso)
  if (Number.isNaN(ms) || ms <= 0) return 'Never'
  const diff = Date.now() - ms
  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  if (days > 0) return `${days} day${days === 1 ? '' : 's'} ago`
  if (hours > 0) return `${hours} hour${hours === 1 ? '' : 's'} ago`
  if (minutes > 0) return `${minutes} minute${minutes === 1 ? '' : 's'} ago`
  if (seconds > 5) return `${seconds} seconds ago`
  return 'just now'
}

function PlatformIcon({ platform }: { platform: DevicePlatform }) {
  const className = 'w-4 h-4 text-gray-400 dark:text-gray-500 flex-shrink-0'
  if (platform === 'ios' || platform === 'android') return <Smartphone className={className} />
  if (platform === 'mac') return <Monitor className={className} />
  if (platform === 'web') return <Tablet className={className} />
  return <Tablet className={className} />
}

interface DeviceRowProps {
  device: Device
  onRevoke: (deviceId: string) => void
  isRevoking: boolean
}

export function DeviceRow({ device, onRevoke, isRevoking }: DeviceRowProps) {
  const isRevoked = device.revoked_at !== null
  const platform = (device.platform as DevicePlatform) ?? 'web'
  const lastSeen = formatRelativeIso(device.last_seen_at)
  const paired = device.created_at ? new Date(device.created_at).toLocaleDateString() : '—'

  return (
    <div
      className={
        isRevoked
          ? 'flex items-center justify-between gap-3 px-4 py-3 opacity-60'
          : 'flex items-center justify-between gap-3 px-4 py-3'
      }
    >
      <div className="flex items-center gap-3 min-w-0">
        <PlatformIcon platform={platform} />
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
              {device.display_name}
            </p>
            {isRevoked && (
              <span className="inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded uppercase tracking-wider bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
                Revoked
              </span>
            )}
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
            {isRevoked
              ? `Revoked ${formatRelativeIso(device.revoked_at)}`
              : `Last seen ${lastSeen} · Paired ${paired}`}
          </p>
        </div>
      </div>

      {!isRevoked && (
        <button
          type="button"
          onClick={() => onRevoke(device.device_id)}
          disabled={isRevoking}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
          aria-label={`Revoke ${device.display_name}`}
        >
          {isRevoking ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <XCircle className="w-3.5 h-3.5" />
          )}
          Revoke
        </button>
      )}
    </div>
  )
}
