import { useQuery } from '@tanstack/react-query'

export interface AuthIdentity {
  hasAuth: boolean
  email: string | null
  orgName: string | null
  subscriptionType: string | null
  authMethod: string | null
}

async function fetchAuthIdentity(): Promise<AuthIdentity> {
  const response = await fetch('/api/oauth/identity')
  if (!response.ok) {
    throw new Error(`Failed to fetch auth identity: ${await response.text()}`)
  }
  return response.json()
}

/**
 * Hook to fetch auth identity (email, org, plan).
 * Fetched once and cached forever (staleTime: Infinity).
 * Enable with `enabled` flag to defer until tooltip opens.
 */
export function useAuthIdentity(enabled = true) {
  return useQuery({
    queryKey: ['auth-identity'],
    queryFn: fetchAuthIdentity,
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
    enabled,
  })
}
