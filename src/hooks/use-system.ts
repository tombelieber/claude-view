import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import type {
  SystemResponse,
  StorageInfo,
  PerformanceInfo,
  HealthInfo,
  IndexRunInfo,
  ClassificationInfo,
  ClaudeCliStatus,
  ActionResponse,
  ClearCacheResponse,
} from '../types/generated'

// Re-export generated types for consumers
export type {
  SystemResponse,
  StorageInfo,
  PerformanceInfo,
  HealthInfo,
  IndexRunInfo,
  ClassificationInfo,
  ActionResponse,
  ClearCacheResponse,
}

// Alias for backward compat — SystemPage references ClaudeCliInfo
export type ClaudeCliInfo = ClaudeCliStatus
export type { ClaudeCliStatus }

// ============================================================================
// Fetch functions
// ============================================================================

async function fetchSystem(): Promise<SystemResponse> {
  const response = await fetch('/api/system')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch system status: ${errorText}`)
  }
  return response.json()
}

async function triggerReindex(): Promise<ActionResponse> {
  const response = await fetch('/api/system/reindex', { method: 'POST' })
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to trigger reindex: ${errorText}`)
  }
  return response.json()
}

async function triggerClearCache(): Promise<ClearCacheResponse> {
  const response = await fetch('/api/system/clear-cache', { method: 'POST' })
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to clear cache: ${errorText}`)
  }
  return response.json()
}

async function triggerGitResync(): Promise<ActionResponse> {
  const response = await fetch('/api/system/git-resync', { method: 'POST' })
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to trigger git resync: ${errorText}`)
  }
  return response.json()
}

async function triggerReset(confirm: string): Promise<ActionResponse> {
  const response = await fetch('/api/system/reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ confirm }),
  })
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to reset: ${errorText}`)
  }
  return response.json()
}

// ============================================================================
// Hooks
// ============================================================================

/**
 * Hook to fetch comprehensive system status.
 *
 * Polls every 30 seconds to keep metrics fresh.
 */
export function useSystem() {
  return useQuery({
    queryKey: ['system'],
    queryFn: fetchSystem,
    staleTime: 10_000,
    refetchInterval: 30_000,
  })
}

/**
 * Hook to trigger a re-index action.
 */
export function useReindex() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: triggerReindex,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['system'] })
    },
  })
}

/**
 * Hook to clear cache.
 */
export function useClearCache() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: triggerClearCache,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['system'] })
    },
  })
}

/**
 * Hook to trigger git re-sync.
 */
export function useGitResync() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: triggerGitResync,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['system'] })
    },
  })
}

/**
 * Hook to perform a factory reset.
 */
export function useReset() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (confirm: string) => triggerReset(confirm),
    onSuccess: () => {
      queryClient.invalidateQueries()
    },
  })
}

// ============================================================================
// Formatting Helpers
// ============================================================================

/** Format bytes into human-readable size (e.g., 12.3 GB, 847 MB) */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  const value = bytes / Math.pow(1024, i)
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[i]}`
}

/** Format milliseconds into human-readable duration (e.g., 2.8s, 8m 23s) */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = Math.round(seconds % 60)
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`
  const hours = Math.floor(minutes / 60)
  const remainingMinutes = minutes % 60
  return `${hours}h ${remainingMinutes}m`
}

/** Format throughput in bytes/sec to human-readable (e.g., 4.2 GB/s) */
export function formatThroughput(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`
}

/** Format ISO 8601 timestamp as relative time */
export function formatRelativeTimestamp(isoStr: string): string {
  const date = new Date(isoStr)
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
