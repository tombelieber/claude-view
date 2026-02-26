import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'

// ============================================================================
// Types matching the Rust backend response structs
// ============================================================================

export interface CoachingRule {
  id: string
  patternId: string
  title: string
  body: string
  scope: string
  appliedAt: string
  filePath: string
}

export interface ListRulesResponse {
  rules: CoachingRule[]
  count: number
  maxRules: number
}

export interface ApplyRuleRequest {
  patternId: string
  recommendation: string
  title: string
  impactScore: number
  sampleSize: number
  scope: string
}

// ============================================================================
// Hook
// ============================================================================

/**
 * Fetch, apply, and remove coaching rules via the API.
 *
 * Coaching rules are CLAUDE.md directives generated from pattern insights.
 * The backend enforces a budget cap (default 8 rules) to keep the file
 * focused and avoid overwhelming Claude with too many instructions.
 *
 * Uses React Query for caching with 30s stale time.
 */
export function useCoachingRules() {
  const queryClient = useQueryClient()

  const query = useQuery({
    queryKey: ['coaching-rules'],
    queryFn: async (): Promise<ListRulesResponse> => {
      const response = await fetch('/api/coaching/rules')
      if (!response.ok) {
        const errorText = await response.text()
        throw new Error(`Failed to fetch coaching rules: ${errorText}`)
      }
      return response.json()
    },
    staleTime: 30_000, // 30 seconds
    refetchOnWindowFocus: false,
  })

  const applyMutation = useMutation({
    mutationFn: async (req: ApplyRuleRequest): Promise<CoachingRule> => {
      const response = await fetch('/api/coaching/rules', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
      })
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Unknown error' }))
        throw new Error(error.details || error.error || 'Failed to apply rule')
      }
      return response.json()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['coaching-rules'] })
    },
  })

  const removeMutation = useMutation({
    mutationFn: async (id: string): Promise<void> => {
      const response = await fetch(`/api/coaching/rules/${id}`, {
        method: 'DELETE',
      })
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Unknown error' }))
        throw new Error(error.details || error.error || 'Failed to remove rule')
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['coaching-rules'] })
    },
  })

  const applyBulk = async (requests: ApplyRuleRequest[]): Promise<void> => {
    const appliedIds = new Set(query.data?.rules.map((r) => r.patternId) ?? [])
    const toApply = requests.filter((r) => !appliedIds.has(r.patternId))
    const budget = (query.data?.maxRules ?? 8) - (query.data?.count ?? 0)
    const batch = toApply.slice(0, budget)
    for (const req of batch) {
      await applyMutation.mutateAsync(req)
    }
  }

  const removeAll = async (): Promise<void> => {
    const rules = query.data?.rules ?? []
    for (const rule of rules) {
      await removeMutation.mutateAsync(rule.id)
    }
  }

  return {
    rules: query.data?.rules ?? [],
    count: query.data?.count ?? 0,
    maxRules: query.data?.maxRules ?? 8,
    isLoading: query.isLoading,
    error: query.error,
    applyRule: applyMutation.mutateAsync,
    removeRule: removeMutation.mutateAsync,
    applyBulk,
    removeAll,
    isApplying: applyMutation.isPending,
    isRemoving: removeMutation.isPending,
  }
}
