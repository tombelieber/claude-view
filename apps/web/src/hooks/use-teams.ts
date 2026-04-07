import { useQuery } from '@tanstack/react-query'
import type { InboxMessage, TeamDetail, TeamSummary } from '../types/generated'
import type { TeamCostBreakdown } from '../types/generated/TeamCostBreakdown'
import type { TeamMemberSidechain } from '@claude-view/shared/types/generated/TeamMemberSidechain'

// ============================================================================
// Fetch functions
// ============================================================================

async function fetchTeams(): Promise<TeamSummary[]> {
  const res = await fetch('/api/teams')
  if (!res.ok) throw new Error('Failed to fetch teams')
  return res.json()
}

async function fetchTeamDetail(name: string): Promise<TeamDetail> {
  const res = await fetch(`/api/teams/${encodeURIComponent(name)}`)
  if (!res.ok) throw new Error(`Failed to fetch team: ${name}`)
  return res.json()
}

async function fetchTeamInbox(name: string): Promise<InboxMessage[]> {
  const res = await fetch(`/api/teams/${encodeURIComponent(name)}/inbox`)
  if (!res.ok) throw new Error(`Failed to fetch inbox for team: ${name}`)
  return res.json()
}

async function fetchTeamCost(name: string): Promise<TeamCostBreakdown> {
  const res = await fetch(`/api/teams/${encodeURIComponent(name)}/cost`)
  if (!res.ok) throw new Error(`Failed to fetch cost for team: ${name}`)
  return res.json()
}

async function fetchTeamSidechains(
  name: string,
  sessionId: string,
): Promise<TeamMemberSidechain[]> {
  const res = await fetch(
    `/api/teams/${encodeURIComponent(name)}/sidechains?session_id=${encodeURIComponent(sessionId)}`,
  )
  if (!res.ok) throw new Error(`Failed to fetch sidechains for team: ${name}`)
  return res.json()
}

// ============================================================================
// Hooks
// ============================================================================

/** Fetch all teams for the /teams index page. */
export function useTeams() {
  return useQuery({
    queryKey: ['teams'],
    queryFn: fetchTeams,
    staleTime: 60_000, // Teams are static (completed bursts), cache 1 min
  })
}

/** Fetch detail for a specific team (header info: name, description, createdAt). */
export function useTeamDetail(name: string | null) {
  return useQuery({
    queryKey: ['team-detail', name],
    queryFn: () => fetchTeamDetail(name ?? ''),
    enabled: !!name,
    staleTime: 60_000,
  })
}

/**
 * Fetch inbox messages for a specific team.
 * Event-driven: `version` comes from SSE `teamInboxCount` — when it changes,
 * query key changes → React Query auto-refetches. No polling needed.
 */
export function useTeamInbox(name: string | null, version?: number) {
  return useQuery({
    queryKey: ['team-inbox', name, version ?? 0],
    queryFn: () => fetchTeamInbox(name ?? ''),
    enabled: !!name,
    staleTime: 60_000,
  })
}

/** Fetch cost breakdown for a specific team. */
export function useTeamCost(name: string | null) {
  return useQuery({
    queryKey: ['team-cost', name],
    queryFn: () => fetchTeamCost(name ?? ''),
    enabled: !!name,
    staleTime: 120_000, // Cost is expensive to compute, cache 2 min
  })
}

/** Fetch sidechain (subagent JSONL) info for a team member in a given session. */
export function useTeamSidechains(teamName: string | null, sessionId: string | null) {
  return useQuery({
    queryKey: ['team-sidechains', teamName, sessionId],
    queryFn: () => fetchTeamSidechains(teamName ?? '', sessionId ?? ''),
    enabled: !!teamName && !!sessionId,
    staleTime: 60_000,
  })
}

/**
 * Match a session ID to its team.
 * Returns the matching TeamSummary if this session is a team lead, null otherwise.
 * Uses the teams list (already cached) to avoid extra API calls.
 *
 * Note: This hook calls useTeams() internally. When used in list contexts
 * (e.g., SessionCard rendered N times), React Query deduplicates -- same
 * queryKey ['teams'] returns the same cache entry with zero network overhead.
 */
export function useTeamForSession(sessionId: string | undefined): TeamSummary | null {
  const { data: teams } = useTeams()
  if (!sessionId || !teams) return null
  return teams.find((t) => t.leadSessionId === sessionId) ?? null
}
