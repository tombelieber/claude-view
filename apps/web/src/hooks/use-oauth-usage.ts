import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

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

interface OAuthUsageResult {
  data: OAuthUsage
  maxAgeSecs: number
}

async function fetchOAuthUsage(): Promise<OAuthUsageResult> {
  const response = await fetch('/api/oauth/usage')
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch OAuth usage: ${errorText}`)
  }
  return { data: await response.json(), maxAgeSecs: parseMaxAgeSecs(response) }
}

async function forceRefreshOAuthUsage(): Promise<OAuthUsageResult> {
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
  return { data: await response.json(), maxAgeSecs: parseMaxAgeSecs(response) }
}

/**
 * Hook to fetch OAuth usage data.
 *
 * Timing is server-driven: the backend sets `Cache-Control: max-age=<remaining_ttl>`.
 * We store the server's max-age in React state so the interval actually updates after
 * each fetch — a plain module-level `let` wouldn't trigger a re-render, causing the
 * refetchInterval to stay frozen at whatever value was captured on first render.
 *
 * `forceRefresh` mutation bypasses the server cache (user-initiated refresh button).
 */
export function useOAuthUsage() {
  const queryClient = useQueryClient()
  const [intervalMs, setIntervalMs] = useState(300_000)

  const query = useQuery({
    queryKey: ['oauth-usage'],
    queryFn: async () => {
      const result = await fetchOAuthUsage()
      setIntervalMs(result.maxAgeSecs * 1000)
      return result
    },
    staleTime: intervalMs,
    refetchInterval: intervalMs,
    refetchOnWindowFocus: false,
    select: (result) => result.data,
  })

  const forceRefresh = useMutation({
    mutationFn: forceRefreshOAuthUsage,
    onSuccess: (result) => {
      setIntervalMs(result.maxAgeSecs * 1000)
      queryClient.setQueryData(['oauth-usage'], result)
    },
  })

  return { ...query, forceRefresh }
}
