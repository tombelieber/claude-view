import { useQuery } from '@tanstack/react-query'

// ============================================================================
// Types — local interfaces mirroring crates/server/src/routes/providers.rs
// (ProviderUsage / ProvidersUsageResponse, camelCase JSON). Generated TS
// types for this endpoint may not exist yet.
// ============================================================================

export interface ProviderUsage {
  /** Provider kebab id (e.g. "codex", "opencode"). */
  id: string
  displayName: string
  /** Total sessions for this provider in the window. */
  sessions: number
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  /** Sessions that reported any token usage. */
  usageSessions: number
  /** Sessions whose every token resolved to a priced model. */
  pricedSessions: number
  /** Sum over priced sessions only; absent when no session priced. */
  costUsd?: number
}

export interface ProvidersUsageResponse {
  days: number
  /** Foreign agents only (Codex, OpenCode, …), sorted by token volume desc. */
  providers: ProviderUsage[]
}

// ============================================================================
// Fetch
// ============================================================================

async function fetchProvidersUsage(days: number): Promise<ProvidersUsageResponse> {
  const response = await fetch(`/api/providers/usage?days=${days}`)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch provider usage: ${errorText}`)
  }
  return response.json()
}

// ============================================================================
// Hook
// ============================================================================

/** Per-provider (foreign agent) usage rollup for the last `days` days. */
export function useProvidersUsage(days: number) {
  return useQuery({
    queryKey: ['providers-usage', days],
    queryFn: () => fetchProvidersUsage(days),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  })
}
