import { useState, useEffect } from 'react'

const CACHE_TTL_SECONDS = 300

interface CacheCountdownRingProps {
  /** Unix timestamp in seconds of last activity */
  lastActivityAt: number
  /** Ring diameter in pixels (default 16) */
  size?: number
}

/**
 * SVG circular progress ring showing Anthropic's prompt cache TTL (300s)
 * depleting in real-time. Green > 60%, amber 20-60%, red < 20%, gray at 0.
 */
export function CacheCountdownRing({ lastActivityAt, size = 16 }: CacheCountdownRingProps) {
  const [remaining, setRemaining] = useState(() => computeRemaining(lastActivityAt))

  useEffect(() => {
    // Immediately sync on prop change
    setRemaining(computeRemaining(lastActivityAt))

    const interval = setInterval(() => {
      setRemaining(computeRemaining(lastActivityAt))
    }, 1000)

    return () => clearInterval(interval)
  }, [lastActivityAt])

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
    ? `Cache: ${minutes}:${seconds.toString().padStart(2, '0')} remaining`
    : 'Cache expired'

  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      className="flex-shrink-0"
      title={timeLabel}
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
  )
}

function computeRemaining(lastActivityAt: number): number {
  if (lastActivityAt <= 0) return 0
  const elapsed = Math.floor(Date.now() / 1000) - lastActivityAt
  return Math.max(0, CACHE_TTL_SECONDS - elapsed)
}

function ringColor(progress: number): string {
  if (progress <= 0) return '#6b7280'   // gray-500
  if (progress < 0.2) return '#ef4444'  // red-500
  if (progress < 0.6) return '#f59e0b'  // amber-500
  return '#22c55e'                       // green-500
}
