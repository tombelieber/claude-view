// apps/web/src/components/chat/ConnectionBanner.tsx
import { RefreshCw, WifiOff } from 'lucide-react'
import type { ConnectionHealth } from '../../hooks/use-session-control'
import { cn } from '../../lib/utils'

interface ConnectionBannerProps {
  health: ConnectionHealth
  onRetry?: () => void
}

export function ConnectionBanner({ health, onRetry }: ConnectionBannerProps) {
  if (health === 'ok') return null

  const isDegraded = health === 'degraded'

  return (
    <div
      role="status"
      className={cn(
        'flex items-center gap-2 px-4 py-2 text-xs font-medium',
        isDegraded
          ? 'bg-amber-50 text-amber-800 dark:bg-amber-950/50 dark:text-amber-300'
          : 'bg-red-50 text-red-800 dark:bg-red-950/50 dark:text-red-300',
      )}
    >
      {isDegraded ? (
        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
      ) : (
        <WifiOff className="w-3.5 h-3.5" />
      )}
      <span>{isDegraded ? 'Reconnecting...' : 'Connection lost'}</span>
      {!isDegraded && onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="ml-auto text-xs underline hover:no-underline cursor-pointer"
        >
          Retry
        </button>
      )}
    </div>
  )
}
