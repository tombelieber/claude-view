import { useState, useEffect } from 'react'
import { cn } from '../../lib/utils'

const CACHE_TTL_SECONDS = 300

interface CacheCountdownBarProps {
  lastCacheHitAt: number | null
  cacheStatus: 'warm' | 'cold' | 'unknown'
}

/**
 * Inline horizontal progress bar showing Anthropic's prompt cache TTL (300s)
 * depleting in real-time. Designed for the side panel overview tab.
 *
 * Shows: progress bar, time remaining, percentage, and next-turn hint.
 * Returns null if no cache activity has occurred.
 */
export function CacheCountdownBar({ lastCacheHitAt, cacheStatus }: CacheCountdownBarProps) {
  const hasCacheHit = lastCacheHitAt != null && lastCacheHitAt > 0
  const [remaining, setRemaining] = useState(() => hasCacheHit ? computeRemaining(lastCacheHitAt) : 0)

  useEffect(() => {
    if (!hasCacheHit) {
      setRemaining(0)
      return
    }

    setRemaining(computeRemaining(lastCacheHitAt))

    const interval = setInterval(() => {
      setRemaining(computeRemaining(lastCacheHitAt))
    }, 1000)

    return () => clearInterval(interval)
  }, [lastCacheHitAt, hasCacheHit])

  if (!hasCacheHit && cacheStatus === 'unknown') {
    return null
  }

  const progress = remaining / CACHE_TTL_SECONDS
  const isExpired = remaining <= 0
  const minutes = Math.floor(remaining / 60)
  const seconds = remaining % 60
  const pctLabel = isExpired ? '0%' : `${Math.round(progress * 100)}%`
  const timeLabel = isExpired ? 'expired' : `${minutes}:${seconds.toString().padStart(2, '0')}`

  const barColor = isExpired
    ? 'bg-gray-400 dark:bg-gray-600'
    : progress < 0.2
      ? 'bg-red-500'
      : progress < 0.6
        ? 'bg-amber-500'
        : 'bg-green-500'

  const statusText = isExpired
    ? 'Cache expired — next turn will re-cache (slower, costs more)'
    : 'Cache warm — next turn uses cached tokens (faster, cheaper)'

  return (
    <div className="space-y-2">
      {/* Time + percentage */}
      <div className="flex items-center justify-between">
        <span className={cn(
          'text-xs font-mono tabular-nums font-medium',
          isExpired ? 'text-gray-400 dark:text-gray-500' : 'text-gray-900 dark:text-gray-100',
        )}>
          {timeLabel}
        </span>
        <span className="text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400">
          {pctLabel} remaining
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-2 rounded-full bg-gray-200 dark:bg-gray-800 overflow-hidden">
        {!isExpired && (
          <div
            className={cn(barColor, 'h-full rounded-full transition-all duration-1000')}
            style={{ width: `${progress * 100}%` }}
          />
        )}
      </div>

      {/* Hint */}
      <p className={cn(
        'text-[11px] leading-relaxed',
        isExpired ? 'text-gray-400 dark:text-gray-500' : 'text-gray-500 dark:text-gray-400',
      )}>
        {statusText}
      </p>
    </div>
  )
}

function computeRemaining(lastCacheHitAt: number | null): number {
  if (lastCacheHitAt == null || lastCacheHitAt <= 0) return 0
  const elapsed = Math.floor(Date.now() / 1000) - lastCacheHitAt
  return Math.max(0, CACHE_TTL_SECONDS - elapsed)
}
