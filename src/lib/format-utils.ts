/**
 * Shared formatting utilities for metrics display.
 * Used across dashboard and session detail components.
 */

/**
 * Smart number formatter — always picks the unit that gives 1-3 digits before
 * the decimal. Used for line counts, generic counts, etc.
 *   999 → "999"  |  1,200 → "1.2K"  |  45,000 → "45K"  |  2,300,000 → "2.3M"  |  7.2B → "7.20B"
 */
export function formatNumber(value: bigint | number | null): string {
  if (value === null) return '--'
  const num = typeof value === 'bigint' ? Number(value) : value
  if (num >= 1_000_000_000) {
    const b = num / 1_000_000_000
    return `${b >= 10 ? b.toFixed(1) : b.toFixed(2)}B`
  }
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`
  if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`
  return num.toLocaleString()
}

/**
 * Smart token formatter — B/M/k suffixes.
 *   999 → "999"  |  5,400 → "5.4k"  |  4.9M → "4.9M"  |  7.18B → "7.18B"
 */
export function formatTokenCount(value: number | null | undefined): string {
  if (value === null || value === undefined) return '--'
  if (value >= 1_000_000_000) {
    const b = value / 1_000_000_000
    return `${b >= 10 ? b.toFixed(1) : b.toFixed(2)}B`
  }
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) {
    const k = value / 1_000
    return `${k >= 100 ? k.toFixed(0) : k >= 10 ? k.toFixed(0) : k.toFixed(1)}k`
  }
  return String(value)
}

/**
 * Smart USD formatter — picks $, $K, or $M to keep numbers readable.
 *   0 → "$0.00"  |  0.003 → "$0.0030"  |  50.34 → "$50.34"
 *   1,500 → "$1.50K"  |  11,714 → "$11.7K"  |  250,000 → "$250K"
 */
export function formatCostUsd(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  if (usd < 1_000) return `$${usd.toFixed(2)}`
  if (usd < 1_000_000) {
    const k = usd / 1_000
    return `$${k >= 100 ? k.toFixed(0) : k >= 10 ? k.toFixed(1) : k.toFixed(2)}K`
  }
  const m = usd / 1_000_000
  return `$${m >= 10 ? m.toFixed(1) : m.toFixed(2)}M`
}

/**
 * Format percentage values.
 * @param value - If 0-1 range, pass asRatio=true. If already percentage (0-100), pass asRatio=false.
 * @param asRatio - If true, value is treated as 0-1 ratio and multiplied by 100
 */
export function formatPercent(value: number | null, asRatio: boolean = false): string {
  if (value === null) return '--'
  const percent = asRatio ? value * 100 : value
  return `${percent.toFixed(1)}%`
}

/** Truncate commit message to specified length, taking first line only */
export function truncateMessage(message: string, maxLength: number = 60): string {
  const firstLine = message.split('\n')[0]
  if (firstLine.length <= maxLength) return firstLine
  return firstLine.slice(0, maxLength - 3) + '...'
}

/** Format timestamp as relative time (e.g., "5m ago", "2h ago", "3d ago") */
export function formatRelativeTime(timestamp: bigint | number): string {
  const ts = typeof timestamp === 'bigint' ? Number(timestamp) : timestamp
  if (ts <= 0) return '--'
  const date = new Date(ts * 1000)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 7) return `${diffDays}d ago`
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}
