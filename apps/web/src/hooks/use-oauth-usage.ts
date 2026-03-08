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

/** Parse `max-age=N` from Cache-Control header (seconds), defaulting to 300. */
function parseMaxAgeSecs(response: Response): number {
  const cc = response.headers.get('Cache-Control')
  if (cc) {
    const match = cc.match(/max-age=(\d+)/)
    if (match) return Number.parseInt(match[1], 10)
  }
  return 300
}

/**
 * Last server-provided max-age in seconds, used as the baseline for
 * staleTime and refetchInterval. Updated on every successful fetch.
 */
let serverMaxAgeSecs = 300

async function fetchOAuthUsage(): Promise<OAuthUsage> {
  const response = await fetch('/api/oauth/usage')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch OAuth usage: ${errorText}`)
  }
  serverMaxAgeSecs = parseMaxAgeSecs(response)
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
  serverMaxAgeSecs = parseMaxAgeSecs(response)
  return response.json()
}

/**
 * Hook to fetch OAuth usage data.
 *
 * Timing is server-driven: the backend sets `Cache-Control: max-age=<ttl>`,
 * and the module-level `serverMaxAgeSecs` tracks it. TanStack Query re-reads
 * staleTime/refetchInterval on every render, so it picks up changes naturally.
 *
 * `forceRefresh` mutation bypasses the server cache (user-initiated refresh button).
 */
export function useOAuthUsage() {
  const queryClient = useQueryClient()
  const intervalMs = serverMaxAgeSecs * 1000

  const query = useQuery({
    queryKey: ['oauth-usage'],
    queryFn: fetchOAuthUsage,
    staleTime: intervalMs,
    refetchInterval: intervalMs,
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
