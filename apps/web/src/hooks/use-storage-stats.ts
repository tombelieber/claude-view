import { useQuery } from '@tanstack/react-query'
import type { StorageStats } from '../types/generated'

/**
 * Fetch storage statistics from /api/stats/storage.
 *
 * Returns:
 * - Storage sizes: JSONL, SQLite, search index
 * - Counts: sessions, projects, commits
 * - Timing: oldest session, last index, last git sync
 */
async function fetchStorageStats(): Promise<StorageStats> {
  const response = await fetch('/api/stats/storage')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch storage stats: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch storage statistics for the settings page.
 *
 * Useful for:
 * - Showing storage breakdown (JSONL, SQLite, index)
 * - Displaying counts (sessions, projects, commits)
 * - Showing timing info (oldest session, last index, last sync)
 */
export function useStorageStats() {
  return useQuery({
    queryKey: ['storage-stats'],
    queryFn: fetchStorageStats,
    staleTime: 30_000, // Consider stale after 30 seconds
    refetchInterval: 60_000, // Auto-refetch every 60 seconds
  })
}

/**
 * Format bytes to human-readable string (e.g., "11.8 GB", "245 MB").
 */
export function formatBytes(bytes: bigint | number | null): string {
  if (bytes === null) return '--'
  const num = typeof bytes === 'bigint' ? Number(bytes) : bytes

  if (num >= 1024 * 1024 * 1024) {
    return `${(num / (1024 * 1024 * 1024)).toFixed(1)} GB`
  }
  if (num >= 1024 * 1024) {
    return `${(num / (1024 * 1024)).toFixed(1)} MB`
  }
  if (num >= 1024) {
    return `${(num / 1024).toFixed(1)} KB`
  }
  return `${num} B`
}

/**
 * Format a Unix timestamp as a relative time string or date.
 */
export function formatTimestamp(timestamp: bigint | null): string {
  if (timestamp === null) return 'Never'

  const ts = Number(timestamp)
  if (ts <= 0) return 'Never'
  const date = new Date(ts * 1000)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffSecs = Math.floor(diffMs / 1000)
  const diffMins = Math.floor(diffSecs / 60)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffSecs < 60) return `${diffSecs}s ago`
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 7) return `${diffDays}d ago`

  // For older dates, show the actual date
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
  })
}

/**
 * Format duration in milliseconds to human-readable string.
 */
export function formatDurationMs(ms: bigint | null): string {
  if (ms === null) return '--'
  const num = Number(ms)

  if (num >= 1000) {
    return `${(num / 1000).toFixed(1)}s`
  }
  return `${num}ms`
}
