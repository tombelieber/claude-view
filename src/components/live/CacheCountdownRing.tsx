import { useState, useEffect } from 'react'
import * as Tooltip from '@radix-ui/react-tooltip'

const CACHE_TTL_SECONDS = 300

interface CacheCountdownRingProps {
  /** Unix timestamp in seconds of last cache hit/creation, or null if no cache activity */
  lastCacheHitAt: number | null
  /** Ring diameter in pixels (default 16) */
  size?: number
}

/**
 * SVG circular progress ring showing Anthropic's prompt cache TTL (300s)
 * depleting in real-time. Green > 60%, amber 20-60%, red < 20%, gray at 0.
 * Returns null if no cache activity has occurred.
 */
export function CacheCountdownRing({ lastCacheHitAt, size = 16 }: CacheCountdownRingProps) {
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

  if (!hasCacheHit) {
    return null
  }

  const progress = remaining / CACHE_TTL_SECONDS // 1.0 = full, 0.0 = expired
  const color = ringColor(progress)

  // SVG geometry
  const strokeWidth = 2
  const radius = (size - strokeWidth) / 2
  const circumference = 2 * Math.PI * radius
  const offset = circumference * (1 - progress)

  const minutes = Math.floor(remaining / 60)
  const seconds = remaining % 60
  const timeLabel = remaining > 0
    ? `Prompt cache warm — ${minutes}:${seconds.toString().padStart(2, '0')} until expiry. Next turn uses cached tokens (faster, cheaper).`
    : 'Prompt cache expired — next turn will re-cache the system prompt (slower, more expensive).'

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <svg
            width={size}
            height={size}
            viewBox={`0 0 ${size} ${size}`}
            className="flex-shrink-0 cursor-default"
            aria-label={timeLabel}
          >
            {/* Background track */}
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              fill="none"
              stroke="currentColor"
              strokeWidth={strokeWidth}
              className="text-gray-700"
            />
            {/* Progress arc */}
            <circle
              cx={size / 2}
              cy={size / 2}
              r={radius}
              fill="none"
              stroke={color}
              strokeWidth={strokeWidth}
              strokeDasharray={circumference}
              strokeDashoffset={offset}
              strokeLinecap="round"
              transform={`rotate(-90 ${size / 2} ${size / 2})`}
            />
          </svg>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            sideOffset={4}
            className="z-50 max-w-xs rounded-md px-3 py-2 text-xs leading-relaxed bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            {timeLabel}
            <a
              href="https://platform.claude.com/docs/en/build-with-claude/prompt-caching"
              target="_blank"
              rel="noopener noreferrer"
              className="block mt-1 text-sky-400 dark:text-sky-600 underline"
              onClick={(e) => e.stopPropagation()}
            >
              Learn about prompt caching
            </a>
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}

function computeRemaining(lastCacheHitAt: number | null): number {
  if (lastCacheHitAt == null || lastCacheHitAt <= 0) return 0
  const elapsed = Math.floor(Date.now() / 1000) - lastCacheHitAt
  return Math.max(0, CACHE_TTL_SECONDS - elapsed)
}

function ringColor(progress: number): string {
  if (progress <= 0) return '#6b7280'   // gray-500
  if (progress < 0.2) return '#ef4444'  // red-500
  if (progress < 0.6) return '#f59e0b'  // amber-500
  return '#22c55e'                       // green-500
}
