// apps/web/src/hooks/use-supported-models.ts
// Fetches the canonical model list from the SDK (via sidecar cache).
// Refreshed automatically whenever a new session is created/resumed.

import { useQuery } from '@tanstack/react-query'
import type { ModelOption } from './use-models'

// Mirrors @anthropic-ai/claude-agent-sdk ModelInfo shape.
// Can't import directly — SDK is a sidecar-only dependency.
// Only the fields we consume are declared.
interface SdkModelInfo {
  value: string
  displayName: string
  description: string
}

interface SupportedModelsResponse {
  models: SdkModelInfo[]
  updatedAt: number | null
}

async function fetchSupportedModels(): Promise<SupportedModelsResponse> {
  const res = await fetch('/api/control/supported-models')
  if (!res.ok) throw new Error(`GET /api/control/supported-models failed: ${res.status}`)
  return res.json()
}

/**
 * Canonical model list from the Agent SDK.
 *
 * Returns ModelOption[] suitable for ModelSelector dropdown.
 * Falls back to empty array before any session has been created
 * (caller should use FALLBACK_MODELS in that case).
 */
export function useSupportedModels(): { options: ModelOption[]; isLoading: boolean } {
  const { data, isLoading } = useQuery({
    queryKey: ['supported-models'],
    queryFn: fetchSupportedModels,
    // Cache refreshes hourly on the sidecar — no need for frequent polling.
    // React Query re-fetches on window focus by default.
    staleTime: 10 * 60 * 1000,
    // Don't retry on failure — sidecar may not be running (local-only feature).
    // Fallback chain in ModelSelector handles this gracefully.
    retry: false,
  })

  if (!data || data.models.length === 0) {
    return { options: [], isLoading }
  }

  const options: ModelOption[] = data.models.map((m) => ({
    id: m.value,
    label: m.displayName,
    description: m.description,
  }))

  return { options, isLoading }
}
