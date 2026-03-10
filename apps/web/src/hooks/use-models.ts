import { useQuery } from '@tanstack/react-query'
import { formatModelName } from '../lib/format-model'
import type { ModelWithStats } from '../types/generated'

async function fetchModels(): Promise<ModelWithStats[]> {
  const res = await fetch('/api/models')
  if (!res.ok) throw new Error(`GET /api/models failed: ${res.status}`)
  return res.json()
}

/**
 * Fetch all known models from the backend.
 *
 * Merges two sources server-side:
 * 1. User's session history — models actually used (with usage stats)
 * 2. LiteLLM pricing map — all known Claude models (refreshed every 24h)
 *
 * Results are sorted: used models first (by total_turns desc), then unused alphabetically.
 */
export function useModels() {
  return useQuery({
    queryKey: ['models'],
    queryFn: fetchModels,
    staleTime: 5 * 60 * 1000,
  })
}

export interface ModelOption {
  id: string
  label: string
}

/**
 * Derive a model options list suitable for dropdowns (ModelSelector, ProviderSettings).
 *
 * Filters to Claude models only, deduplicates by family (picks the latest version),
 * and returns { id, label } pairs with display-friendly labels.
 */
export function useModelOptions(): { options: ModelOption[]; isLoading: boolean } {
  const { data, isLoading } = useModels()

  if (!data || data.length === 0) {
    return { options: [], isLoading }
  }

  // Filter to Claude models, pick one per family (highest total_turns wins, already sorted)
  const seen = new Set<string>()
  const options: ModelOption[] = []

  for (const m of data) {
    if (!m.id.startsWith('claude-')) continue
    const family = m.family ?? 'unknown'
    if (seen.has(family)) continue
    seen.add(family)
    options.push({ id: m.id, label: formatModelName(m.id) })
  }

  return { options, isLoading }
}
