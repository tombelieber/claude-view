/**
 * Prompt filter state types and utilities.
 *
 * Defines filter, sort, and group-by options for the Prompt History page.
 * URL persistence follows the same pattern as use-session-filters.ts.
 */

import { useCallback, useMemo } from 'react'

export type PromptSort = 'recent' | 'oldest' | 'most_repeated' | 'longest'
export type PromptGroupBy = 'none' | 'day' | 'week' | 'month' | 'project' | 'intent' | 'model'

export interface PromptFilters {
  // Sort and grouping
  sort: PromptSort
  groupBy: PromptGroupBy

  // Multi-select filters
  intents: string[]
  models: string[]
  branches: string[]

  // Toggle filters
  hasPaste: 'any' | 'yes' | 'no'

  // Single-select filters
  complexity: string | null // micro | short | medium | detailed | long
  templateMatch: string | null // template | unique

  // Data-driven options passed through for the filter popover
  availableBranches?: string[]
  availableModels?: string[]
}

export const defaultPromptFilters: PromptFilters = {
  sort: 'recent',
  groupBy: 'none',
  intents: [],
  models: [],
  branches: [],
  hasPaste: 'any',
  complexity: null,
  templateMatch: null,
}

/**
 * Count how many active filters are set (excluding sort and groupBy).
 */
export function countActivePromptFilters(filters: PromptFilters): number {
  let count = 0

  if (filters.intents.length > 0) count++
  if (filters.models.length > 0) count++
  if (filters.branches.length > 0) count++
  if (filters.hasPaste !== 'any') count++
  if (filters.complexity !== null) count++
  if (filters.templateMatch !== null) count++

  return count
}

/** Keys managed by this module — used to clean stale params before merging. */
const FILTER_KEYS = [
  'sort',
  'groupBy',
  'intents',
  'models',
  'branches',
  'hasPaste',
  'complexity',
  'templateMatch',
] as const

/**
 * Parse prompt filters from URL search params.
 */
function parseFilters(searchParams: URLSearchParams): PromptFilters {
  return {
    sort: (searchParams.get('sort') || 'recent') as PromptSort,
    groupBy: (searchParams.get('groupBy') || 'none') as PromptGroupBy,

    intents: searchParams.get('intents')?.split(',').filter(Boolean) || [],
    models: searchParams.get('models')?.split(',').filter(Boolean) || [],
    branches: searchParams.get('branches')?.split(',').filter(Boolean) || [],

    hasPaste: (searchParams.get('hasPaste') || 'any') as 'any' | 'yes' | 'no',
    complexity: searchParams.get('complexity') || null,
    templateMatch: searchParams.get('templateMatch') || null,
  }
}

/**
 * Serialize prompt filters into an existing URLSearchParams, preserving
 * params that belong to other systems.
 */
function serializeFilters(filters: PromptFilters, existing: URLSearchParams): URLSearchParams {
  const params = new URLSearchParams(existing)

  // Clear all keys we own
  for (const key of FILTER_KEYS) {
    params.delete(key)
  }

  // Only set non-default values
  if (filters.sort !== 'recent') {
    params.set('sort', filters.sort)
  }

  if (filters.groupBy !== 'none') {
    params.set('groupBy', filters.groupBy)
  }

  if (filters.intents.length > 0) {
    params.set('intents', filters.intents.join(','))
  }

  if (filters.models.length > 0) {
    params.set('models', filters.models.join(','))
  }

  if (filters.branches.length > 0) {
    params.set('branches', filters.branches.join(','))
  }

  if (filters.hasPaste !== 'any') {
    params.set('hasPaste', filters.hasPaste)
  }

  if (filters.complexity !== null) {
    params.set('complexity', filters.complexity)
  }

  if (filters.templateMatch !== null) {
    params.set('templateMatch', filters.templateMatch)
  }

  return params
}

/**
 * Hook to manage prompt filters with URL persistence.
 *
 * @example
 * ```tsx
 * const [filters, setFilters] = usePromptFilters(searchParams, setSearchParams);
 * setFilters({ ...filters, intents: ['fix', 'create'] });
 * ```
 */
export function usePromptFilters(
  searchParams: URLSearchParams,
  setSearchParams: (params: URLSearchParams, opts?: { replace?: boolean }) => void,
): [PromptFilters, (filters: PromptFilters) => void] {
  const urlKey = searchParams.toString()
  const filters = useMemo(() => parseFilters(searchParams), [urlKey]) // eslint-disable-line react-hooks/exhaustive-deps

  const setFilters = useCallback(
    (newFilters: PromptFilters) => {
      const params = serializeFilters(newFilters, searchParams)
      setSearchParams(params, { replace: true })
    },
    [urlKey, setSearchParams], // eslint-disable-line react-hooks/exhaustive-deps
  )

  return [filters, setFilters]
}
