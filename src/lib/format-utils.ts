/**
 * Shared formatting utilities for metrics display.
 * Used across dashboard and session detail components.
 */

/** Format large numbers with K/M suffixes */
export function formatNumber(value: bigint | number | null): string {
  if (value === null) return '--'
  const num = typeof value === 'bigint' ? Number(value) : value
  if (num >= 1_000_000) {
    return `${(num / 1_000_000).toFixed(1)}M`
  }
  if (num >= 1_000) {
    return `${(num / 1_000).toFixed(1)}K`
  }
  return num.toLocaleString()
}

/** Format token counts with live-monitor style suffixes (k/M). */
export function formatTokenCount(value: number | null | undefined): string {
  if (value === null || value === undefined) return '--'
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(0)}k`
  return String(value)
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
