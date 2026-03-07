import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

export interface UsageTier {
  id: string
  label: string
  /** 0-100 */
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

async function forceRefreshOAuthUsage(): Promise<OAuthUsage> {
  const response = await fetch('/api/oauth/usage/refresh', { method: 'POST' })
  if (response.status === 429) {
    const retryAfter = response.headers.get('Retry-After')
    const secs = retryAfter ? Number.parseInt(retryAfter, 10) : 60
    throw new Error(`Try again in ${secs}s`)
  }
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to refresh OAuth usage: ${errorText}`)
  }
  return response.json()
}

/**
 * Hook to fetch OAuth usage data with background polling.
 *
 * Returns `refetch` for cache-backed refresh (tooltip hover) and
 * `forceRefresh` mutation for user-initiated bypass of server cache.
 */
export function useOAuthUsage(refetchInterval = 300_000) {
  const queryClient = useQueryClient()

  const query = useQuery({
    queryKey: ['oauth-usage'],
    queryFn: fetchOAuthUsage,
    staleTime: 30_000,
    refetchInterval,
    refetchOnWindowFocus: false,
  })

  const forceRefresh = useMutation({
    mutationFn: forceRefreshOAuthUsage,
    onSuccess: (data) => {
      queryClient.setQueryData(['oauth-usage'], data)
    },
  })

  return { ...query, forceRefresh }
}
