import { useQuery } from '@tanstack/react-query'
import type {
  BranchSessionsResponse,
  ContributionsResponse,
  SessionContributionResponse,
} from '../types/generated'
import type { TimeRangePreset } from './use-time-range'

/**
 * Map frontend presets to the contributions API's expected range strings.
 * NOTE: Contributions API expects YYYY-MM-DD for from/to (NOT Unix timestamps).
 * This differs from the dashboard stats API which uses Unix seconds.
 */
function presetToApiRange(preset: TimeRangePreset): string {
  switch (preset) {
    case 'today': return 'today'
    case '7d': return 'week'
    case '30d': return 'month'
    case '90d': return '90days'
    case 'all': return 'all'
    case 'custom': return 'custom'
  }
}

export interface ContributionsTimeRange {
  preset: TimeRangePreset
  from?: number | null  // unix seconds (converted to YYYY-MM-DD before sending)
  to?: number | null    // unix seconds (converted to YYYY-MM-DD before sending)
}

/**
 * Fetch contributions data from the API.
 */
async function fetchContributions(
  time: ContributionsTimeRange,
  projectId?: string,
  branch?: string
): Promise<ContributionsResponse> {
  const apiRange = presetToApiRange(time.preset)
  let url = `/api/contributions?range=${encodeURIComponent(apiRange)}`
  if (time.preset === 'custom' && time.from != null && time.to != null) {
    const fromDate = new Date(time.from * 1000).toISOString().split('T')[0]
    const toDate = new Date(time.to * 1000).toISOString().split('T')[0]
    url += `&from=${fromDate}&to=${toDate}`
  }
  if (projectId) {
    url += `&projectId=${encodeURIComponent(projectId)}`
  }
  if (branch) {
    url += `&branch=${encodeURIComponent(branch)}`
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
export function useContributions(time: ContributionsTimeRange, projectId?: string, branch?: string) {
  return useQuery({
    queryKey: ['contributions', time.preset, time.from, time.to, projectId, branch],
    queryFn: () => fetchContributions(time, projectId, branch),
    staleTime: getStaleTime(time.preset),
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
  time: ContributionsTimeRange,
  projectId?: string
): Promise<BranchSessionsResponse> {
  const apiRange = presetToApiRange(time.preset)
  let url = `/api/contributions/branches/${encodeURIComponent(branch)}/sessions?range=${encodeURIComponent(apiRange)}`
  if (time.preset === 'custom' && time.from != null && time.to != null) {
    const fromDate = new Date(time.from * 1000).toISOString().split('T')[0]
    const toDate = new Date(time.to * 1000).toISOString().split('T')[0]
    url += `&from=${fromDate}&to=${toDate}`
  }
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
  time: ContributionsTimeRange = { preset: '7d' },
  enabled: boolean = true,
  projectId?: string
) {
  return useQuery({
    queryKey: ['branch-sessions', branch, time.preset, time.from, time.to, projectId],
    queryFn: () => fetchBranchSessions(branch!, time, projectId),
    enabled: enabled && !!branch,
    staleTime: 5 * 60 * 1000, // 5 min
  })
}

/**
 * Get stale time based on time range (match API cache duration).
 */
function getStaleTime(preset: TimeRangePreset): number {
  switch (preset) {
    case 'today':
      return 60 * 1000 // 1 min
    case '7d':
      return 5 * 60 * 1000 // 5 min
    case '30d':
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
