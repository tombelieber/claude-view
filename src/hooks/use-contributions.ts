import { useQuery } from '@tanstack/react-query'
import type {
  BranchSessionsResponse,
  ContributionsResponse,
  SessionContributionResponse,
} from '../types/generated'

/**
 * Time range options for contributions API.
 */
export type TimeRange = 'today' | 'week' | 'month' | '90days' | 'all'

/**
 * Fetch contributions data from the API.
 */
async function fetchContributions(range: TimeRange, projectId?: string): Promise<ContributionsResponse> {
  let url = `/api/contributions?range=${encodeURIComponent(range)}`
  if (projectId) {
    url += `&projectId=${encodeURIComponent(projectId)}`
  }
  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch contributions: ${errorText}`)
  }
  return response.json()
}

/**
 * Fetch session contribution details.
 */
async function fetchSessionContribution(sessionId: string): Promise<SessionContributionResponse> {
  const response = await fetch(`/api/contributions/sessions/${encodeURIComponent(sessionId)}`)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch session contribution: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch contributions data.
 *
 * Returns ContributionsResponse with:
 * - overview: FluencyMetrics, OutputMetrics, EffectivenessMetrics
 * - trend: Array<DailyTrendPoint>
 * - efficiency: EfficiencyMetrics
 * - byModel: Array<ModelStats>
 * - learningCurve: LearningCurve
 * - byBranch: Array<BranchBreakdown>
 * - bySkill: Array<SkillStats>
 * - uncommitted: Array<UncommittedWork>
 * - warnings: Array<ContributionWarning>
 */
export function useContributions(range: TimeRange = 'week', projectId?: string) {
  return useQuery({
    queryKey: ['contributions', range, projectId],
    queryFn: () => fetchContributions(range, projectId),
    staleTime: getStaleTime(range),
    gcTime: 30 * 60 * 1000, // 30 min garbage collection
  })
}

/**
 * Hook to fetch session contribution details.
 */
export function useSessionContribution(sessionId: string | null) {
  return useQuery({
    queryKey: ['session-contribution', sessionId],
    queryFn: () => fetchSessionContribution(sessionId!),
    enabled: !!sessionId,
    staleTime: 5 * 60 * 1000, // 5 min
  })
}

/**
 * Fetch sessions for a specific branch.
 */
async function fetchBranchSessions(
  branch: string,
  range: TimeRange,
  projectId?: string
): Promise<BranchSessionsResponse> {
  let url = `/api/contributions/branches/${encodeURIComponent(branch)}/sessions?range=${encodeURIComponent(range)}`
  if (projectId) {
    url += `&projectId=${encodeURIComponent(projectId)}`
  }
  const response = await fetch(url)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch branch sessions: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch sessions for a specific branch.
 *
 * Only fetches when enabled (branch is expanded).
 */
export function useBranchSessions(
  branch: string | null,
  range: TimeRange = 'week',
  enabled: boolean = true,
  projectId?: string
) {
  return useQuery({
    queryKey: ['branch-sessions', branch, range, projectId],
    queryFn: () => fetchBranchSessions(branch!, range, projectId),
    enabled: enabled && !!branch,
    staleTime: 5 * 60 * 1000, // 5 min
  })
}

/**
 * Get stale time based on time range (match API cache duration).
 */
function getStaleTime(range: TimeRange): number {
  switch (range) {
    case 'today':
      return 60 * 1000 // 1 min
    case 'week':
      return 5 * 60 * 1000 // 5 min
    case 'month':
      return 15 * 60 * 1000 // 15 min
    default:
      return 30 * 60 * 1000 // 30 min
  }
}

// Re-export types for convenience
export type {
  BranchSession,
  BranchSessionsResponse,
  ContributionsResponse,
  SessionContributionResponse,
  OverviewMetrics,
  FluencyMetrics,
  OutputMetrics,
  EffectivenessMetrics,
  DailyTrendPoint,
  EfficiencyMetrics,
  ModelStats,
  LearningCurve,
  BranchBreakdown,
  SkillStats,
  UncommittedWork,
  ContributionWarning,
  Insight,
  InsightKind,
} from '../types/generated'
