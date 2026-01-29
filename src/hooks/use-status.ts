import { useQuery } from '@tanstack/react-query'
import type { IndexMetadata } from '../types/generated'

/**
 * Fetch index metadata from /api/status.
 *
 * Returns:
 * - lastIndexedAt: When indexing last completed (unix timestamp)
 * - lastIndexDurationMs: Duration of last index
 * - sessionsIndexed: Sessions in last index
 * - projectsIndexed: Projects in last index
 * - lastGitSyncAt: When git sync last completed
 * - commitsFound: Commits found in last git sync
 * - linksCreated: Session-commit links created
 * - updatedAt: When metadata was last updated
 */
async function fetchStatus(): Promise<IndexMetadata> {
  const response = await fetch('/api/status')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch status: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch index metadata for data freshness tracking.
 *
 * Useful for:
 * - Showing "Last indexed X minutes ago"
 * - Knowing if git sync has run
 * - Displaying index statistics
 */
export function useStatus() {
  return useQuery({
    queryKey: ['status'],
    queryFn: fetchStatus,
    staleTime: 5_000, // Consider stale after 5 seconds
    refetchInterval: 10_000, // Auto-refetch every 10 seconds (matches periodic git-sync cadence)
  })
}

/**
 * Format a timestamp as relative time (e.g., "5 minutes ago").
 * Returns null if timestamp is null.
 */
export function formatRelativeTime(timestamp: bigint | null): string | null {
  if (timestamp === null) return null

  const now = Date.now()
  const timestampMs = Number(timestamp) * 1000
  const diff = now - timestampMs

  const seconds = Math.floor(diff / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)

  if (days > 0) return `${days} day${days === 1 ? '' : 's'} ago`
  if (hours > 0) return `${hours} hour${hours === 1 ? '' : 's'} ago`
  if (minutes > 0) return `${minutes} minute${minutes === 1 ? '' : 's'} ago`
  return 'just now'
}

// Re-export types for convenience
export type { IndexMetadata } from '../types/generated'
