import { useQuery } from '@tanstack/react-query'
import type { AIGenerationStats } from '../types/generated'

/** Time range parameters for AI generation API */
export interface TimeRangeParams {
  /** Start timestamp (Unix seconds) - null for all-time */
  from: number | null
  /** End timestamp (Unix seconds) - null for all-time */
  to: number | null
}

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
async function fetchAIGenerationStats(params?: TimeRangeParams): Promise<AIGenerationStats> {
  let url = '/api/stats/ai-generation'

  // Add time range params if provided (not all-time)
  if (params?.from !== null && params?.to !== null) {
    const searchParams = new URLSearchParams()
    searchParams.set('from', params.from.toString())
    searchParams.set('to', params.to.toString())
    url += `?${searchParams.toString()}`
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
export function useAIGenerationStats(timeRange?: TimeRangeParams | null) {
  return useQuery({
    queryKey: ['ai-generation-stats', timeRange?.from, timeRange?.to],
    queryFn: () => fetchAIGenerationStats(timeRange ?? undefined),
    staleTime: 30_000, // Consider data fresh for 30 seconds
  })
}

/**
 * Format token count to human-readable string (e.g., "1.2M", "450K").
 */
export function formatTokens(tokens: number | null | undefined): string {
  if (tokens === null || tokens === undefined) return '--'
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`
  }
  if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(0)}K`
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
