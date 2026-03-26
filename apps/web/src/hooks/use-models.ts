import { useQuery } from '@tanstack/react-query'
import { formatModelName } from '../lib/format-model'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** SDK ModelInfo shape returned by sidecar /sessions/models endpoint. */
interface SidecarModelInfo {
  value: string
  displayName?: string
  description?: string
}

interface SidecarModelsResponse {
  models: SidecarModelInfo[]
  updatedAt: number | null
}

export interface ModelOption {
  id: string
  label: string
  description?: string
  contextWindow?: string
}

// ---------------------------------------------------------------------------
// Sidecar fetch
// ---------------------------------------------------------------------------

async function fetchSidecarModels(): Promise<SidecarModelsResponse> {
  const res = await fetch('/api/sidecar/sessions/models')
  if (!res.ok) throw new Error(`GET /api/sidecar/sessions/models failed: ${res.status}`)
  return res.json()
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/** Extract context window from SDK description, e.g. "with 1M context" → "1M". */
function parseContextWindow(description: string | undefined): string | undefined {
  if (!description) return undefined
  const match = description.match(/(\d+[KMkm])\s*context/i)
  return match ? match[1].toUpperCase() : undefined
}

/**
 * Extract capability description after the "·" separator.
 * SDK format: "Opus 4.6 with 1M context [NEW] · Most capable for complex work"
 * Returns: "Most capable for complex work"
 */
function parseCapabilityDescription(description: string | undefined): string | undefined {
  if (!description) return undefined
  const parts = description.split('·')
  if (parts.length < 2) return undefined
  const cap = parts[parts.length - 1].trim()
  return cap || undefined
}

/**
 * Build a display label from SDK model info.
 *
 * SDK descriptions follow the pattern: "{Family} {version} [extras] · {capability}"
 * e.g. "Opus 4.6 with 1M context · Most capable for complex work"
 *      "Sonnet 4.6 · Best for everyday tasks"
 *      "Haiku 4.5 · Fastest for quick answers"
 *
 * We extract "Claude {Family} {version}" from the description for consistency.
 * Falls back to formatModelName(value) if description doesn't match.
 */
export function buildLabel(model: SidecarModelInfo): string {
  if (model.description) {
    const desc = model.description.split('·')[0].trim()
    const nameMatch = desc.match(/^(opus|sonnet|haiku)\s+(\d+(?:\.\d+)?)/i)
    if (nameMatch) {
      const family = nameMatch[1].charAt(0).toUpperCase() + nameMatch[1].slice(1).toLowerCase()
      return `Claude ${family} ${nameMatch[2]}`
    }
  }
  return formatModelName(model.value)
}

// ---------------------------------------------------------------------------
// Hook: useAvailableModels
// ---------------------------------------------------------------------------

/**
 * Fetch available models directly from sidecar's SDK model cache.
 *
 * One hop: Frontend → Sidecar → SDK. No DB, no alias resolution, no pricing table.
 * The SDK is the source of truth for what models the user can select.
 *
 * Filters out "default" when another model with the same family is present
 * (it's a duplicate — "default" = whatever the current default model is).
 */
export function useAvailableModels(): { options: ModelOption[]; isLoading: boolean } {
  const { data, isLoading } = useQuery({
    queryKey: ['sidecar-models'],
    queryFn: fetchSidecarModels,
    staleTime: 5 * 60 * 1000,
    // Cold start: sidecar cache may be empty on first fetch (SDK CLI
    // subprocess takes a few seconds). Retry every 3s until populated.
    refetchInterval: (query) => {
      const models = query.state.data?.models
      return !models || models.length === 0 ? 3_000 : false
    },
  })

  if (!data || data.models.length === 0) {
    return { options: [], isLoading }
  }

  const options: ModelOption[] = []
  let defaultOption: ModelOption | null = null
  const families = new Set<string>()

  for (const m of data.models) {
    const option: ModelOption = {
      id: m.value,
      label: buildLabel(m),
      description: parseCapabilityDescription(m.description),
      contextWindow: parseContextWindow(m.description),
    }

    if (m.value === 'default') {
      defaultOption = option
      continue
    }

    families.add(m.value)
    options.push(option)
  }

  // Only include "default" if it represents a model not already listed.
  // e.g. if SDK returns default + sonnet + haiku + opus, and default maps to sonnet,
  // skip it. But if default maps to something novel, include it.
  if (defaultOption) {
    // Check if default's label matches an existing option's label
    const isDuplicate = options.some((o) => o.label === defaultOption!.label)
    if (!isDuplicate) {
      defaultOption.label = `${defaultOption.label} (Default)`
      options.unshift(defaultOption)
    }
  }

  return { options, isLoading }
}

// Keep the old name as alias for backwards compatibility during migration
export const useModelOptions = useAvailableModels

// ---------------------------------------------------------------------------
// Resolve session model
// ---------------------------------------------------------------------------

/**
 * Resolve a session's primaryModel to an available model option.
 *
 * Sessions store real model IDs (e.g. "claude-sonnet-4-6") but the selector
 * now uses SDK aliases (e.g. "sonnet"). Try exact match first, then family match.
 * Returns the option id if found, null otherwise (caller falls back to user default).
 */
export function resolveSessionModel(
  primaryModel: string | null | undefined,
  options: ModelOption[],
): string | null {
  if (!primaryModel || options.length === 0) return null

  // Exact match — model stored as alias (new format)
  if (options.some((o) => o.id === primaryModel)) return primaryModel

  // Family match — model stored as real ID (legacy format)
  // Extract family from "claude-{family}-..." or "claude-{major}-{minor}-{family}-..."
  const family = extractFamily(primaryModel)
  if (family) {
    const match = options.find((o) => o.id === family)
    if (match) return match.id
  }

  return null
}

/** Extract family alias from a real model ID. */
function extractFamily(modelId: string): string | null {
  // Modern: claude-opus-4-6 → opus, claude-sonnet-4-5-20250929 → sonnet
  const modern = modelId.match(/^claude-([a-z]+)-\d/)
  if (modern) return modern[1]
  // Legacy: claude-3-5-sonnet-20241022 → sonnet, claude-3-opus-20240229 → opus
  const legacy = modelId.match(/^claude-\d+-(?:\d+-)?([a-z]+)/)
  if (legacy) return legacy[1]
  return null
}
