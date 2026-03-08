import { useQuery } from '@tanstack/react-query'
import type { InboxMessage, TeamDetail, TeamSummary } from '../types/generated'

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

/** Fetch detail for a specific team. */
export function useTeamDetail(name: string | null) {
  return useQuery({
    queryKey: ['team-detail', name],
    queryFn: () => fetchTeamDetail(name!),
    enabled: !!name,
    staleTime: 60_000,
  })
}

/** Fetch inbox messages for a specific team. */
export function useTeamInbox(name: string | null) {
  return useQuery({
    queryKey: ['team-inbox', name],
    queryFn: () => fetchTeamInbox(name!),
    enabled: !!name,
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
