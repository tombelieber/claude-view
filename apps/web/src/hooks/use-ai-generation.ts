import { useQuery } from '@tanstack/react-query'
import type { AIGenerationStats } from '../types/generated'
import type { TimeRangeParams } from '../types/time-range'

export type { TimeRangeParams } from '../types/time-range'

/**
 * Fetch AI generation stats from /api/stats/ai-generation.
 *
 * Returns:
 * - linesAdded, linesRemoved: Currently 0 (future migration needed)
 * - filesCreated: Files edited/created by AI
 * - totalInputTokens, totalOutputTokens: Aggregate token usage
 * - tokensByModel: Token breakdown by AI model
 * - tokensByProject: Top 5 projects by token usage + "Others"
 */
async function fetchAIGenerationStats(params?: TimeRangeParams, project?: string, branch?: string): Promise<AIGenerationStats> {
  let url = '/api/stats/ai-generation'
  const searchParams = new URLSearchParams()

  // Add time range params if provided (not all-time)
  if (params?.from != null && params?.to != null) {
    searchParams.set('from', params.from.toString())
    searchParams.set('to', params.to.toString())
  }
  if (project) {
    searchParams.set('project', project)
  }
  if (branch) {
    searchParams.set('branch', branch)
  }
  const qs = searchParams.toString()
  if (qs) {
    url += `?${qs}`
  }

  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch AI generation stats: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch AI generation statistics with optional time range filter.
 *
 * @param timeRange - Optional time range filter. If null/undefined or both from/to are null,
 *                    returns all-time stats.
 *
 * Returns AIGenerationStats with:
 * - linesAdded, linesRemoved (currently 0, future feature)
 * - filesCreated (files edited by AI)
 * - totalInputTokens, totalOutputTokens
 * - tokensByModel (breakdown by model)
 * - tokensByProject (top 5 + "Others")
 */
export function useAIGenerationStats(timeRange?: TimeRangeParams | null, project?: string, branch?: string) {
  return useQuery({
    queryKey: ['ai-generation-stats', timeRange?.from, timeRange?.to, project, branch],
    queryFn: () => fetchAIGenerationStats(timeRange ?? undefined, project, branch),
    staleTime: 30_000, // Consider data fresh for 30 seconds
  })
}

/**
 * Format token count to human-readable string (e.g., "1.2M", "450K").
 */
export function formatTokens(tokens: number | null | undefined): string {
  if (tokens === null || tokens === undefined) return '--'
  if (tokens >= 1_000_000_000) {
    const b = tokens / 1_000_000_000
    return `${b >= 10 ? b.toFixed(1) : b.toFixed(2)}B`
  }
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000
    return `${m >= 100 ? m.toFixed(1) : m.toFixed(1)}M`
  }
  if (tokens >= 1_000) {
    const k = tokens / 1_000
    return `${k >= 100 ? k.toFixed(0) : k >= 10 ? k.toFixed(0) : k.toFixed(1)}k`
  }
  return tokens.toString()
}

/**
 * Format line count with sign (e.g., "+12,847", "-3,201").
 */
export function formatLineCount(lines: number, showPlus = true): string {
  const formatted = lines.toLocaleString()
  if (showPlus && lines > 0) {
    return `+${formatted}`
  }
  return formatted
}

// Re-export types for convenience
export type { AIGenerationStats, TokensByModel, TokensByProject } from '../types/generated'
