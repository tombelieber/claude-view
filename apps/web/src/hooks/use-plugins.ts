import { useQuery } from '@tanstack/react-query'
import type { PluginsResponse } from '../types/generated'

// ============================================================================
// Fetch function
// ============================================================================

interface PluginsQueryParams {
  scope?: string
  source?: string
  kind?: string
  search?: string
  sort?: string
}

async function fetchPlugins(params: PluginsQueryParams): Promise<PluginsResponse> {
  const search = new URLSearchParams()
  if (params.scope) search.set('scope', params.scope)
  if (params.source) search.set('source', params.source)
  if (params.kind) search.set('kind', params.kind)
  if (params.search) search.set('search', params.search)
  if (params.sort) search.set('sort', params.sort)
  const qs = search.toString()
  const res = await fetch(`/api/plugins${qs ? `?${qs}` : ''}`)
  if (!res.ok) throw new Error('Failed to fetch plugins')
  return res.json()
}

// ============================================================================
// Hook
// ============================================================================

/** Fetch installed + available plugins with optional filters. */
export function usePlugins(params: PluginsQueryParams = {}) {
  return useQuery({
    queryKey: ['plugins', params],
    queryFn: () => fetchPlugins(params),
    staleTime: 30_000,
  })
}
