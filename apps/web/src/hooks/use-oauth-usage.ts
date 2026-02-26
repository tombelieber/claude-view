import { useQuery } from '@tanstack/react-query'

export interface UsageTier {
  id: string
  label: string
  /** 0–100 */
  percentage: number
  /** ISO-8601 reset timestamp */
  resetAt: string
  /** Dollar description for budget tiers, e.g. "$51.25 / $50.00 spent" */
  spent?: string | null
}

export interface OAuthUsage {
  hasAuth: boolean
  error: string | null
  plan?: string | null
  tiers: UsageTier[]
}

async function fetchOAuthUsage(): Promise<OAuthUsage> {
  const response = await fetch('/api/oauth/usage')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch OAuth usage: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch OAuth usage data with background polling.
 *
 * Returns `refetch` so consumers can trigger a fresh fetch on demand
 * (e.g. when a hover popover opens).
 */
export function useOAuthUsage(refetchInterval: number = 300_000) {
  return useQuery({
    queryKey: ['oauth-usage'],
    queryFn: fetchOAuthUsage,
    staleTime: 30_000,
    refetchInterval,
    refetchOnWindowFocus: false,
  })
}
