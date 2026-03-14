import { useQuery } from '@tanstack/react-query'
import { formatContextWindow, formatModelName } from '../lib/format-model'
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
  description?: string
  contextWindow?: string
}

/**
 * Derive a model options list suitable for dropdowns (ModelSelector, ProviderSettings).
 *
 * When `sdkOnly` is true (default for chat), filters to models the Agent SDK
 * reports as usable — the SDK is the source of truth for what can be selected.
 * When false (analytics/settings), returns all Claude models.
 *
 * Deduplicates by family (picks the latest version / highest usage).
 */
export function useModelOptions(opts?: {
  sdkOnly?: boolean
}): { options: ModelOption[]; isLoading: boolean } {
  const sdkOnly = opts?.sdkOnly ?? true
  const { data, isLoading } = useModels()

  if (!data || data.length === 0) {
    return { options: [], isLoading }
  }

  // Check if ANY model has sdk_supported — if not, SDK hasn't reported yet (cold start).
  // Fall back to showing all Claude models so the selector isn't empty.
  const hasAnySdkModel = sdkOnly && data.some((m) => m.sdkSupported)
  const effectiveSdkOnly = sdkOnly && hasAnySdkModel

  // Filter to Claude models, pick one per family (highest total_turns wins, already sorted)
  const seen = new Set<string>()
  const options: ModelOption[] = []

  for (const m of data) {
    if (!m.id.startsWith('claude-')) continue
    if (effectiveSdkOnly && !m.sdkSupported) continue
    const family = m.family ?? 'unknown'
    if (seen.has(family)) continue
    seen.add(family)
    options.push({
      id: m.id,
      label: m.displayName ?? formatModelName(m.id),
      description: m.description ?? undefined,
      contextWindow: formatContextWindow(m.maxInputTokens),
    })
  }

  return { options, isLoading }
}

/**
 * Resolve a session's primaryModel to an SDK-supported model ID.
 * Returns the model as-is if it's in the supported list,
 * otherwise returns null (caller falls back to user default).
 */
export function resolveSessionModel(
  primaryModel: string | null | undefined,
  options: ModelOption[],
): string | null {
  if (!primaryModel || options.length === 0) return null
  // Exact match — session model is still SDK-supported
  if (options.some((o) => o.id === primaryModel)) return primaryModel
  return null
}
