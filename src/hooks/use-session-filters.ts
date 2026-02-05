// src/hooks/use-session-filters.ts
/**
 * Extended session filter state management with URL persistence.
 *
 * IMPORTANT: The returned `filters` object is memoized via useMemo keyed on
 * the URL string — it keeps a stable reference across re-renders when the URL
 * hasn't changed. This is critical: any child component that puts `filters`
 * in a useEffect dependency array would otherwise reset on every parent render.
 *
 * Manages all session filter parameters including:
 * - Sort order (recent/tokens/prompts/files_edited/duration)
 * - Group by (none/branch/project/model/day/week/month)
 * - Branch filter (multi-select)
 * - Model filter (multi-select)
 * - Has commits filter (any/yes/no)
 * - Has skills filter (any/yes/no)
 * - Duration minimum (null/1800/3600/7200 seconds)
 * - Files edited minimum (null/5/10/20)
 * - Token usage minimum (null/10000/50000/100000)
 * - High re-edit rate filter (null/true for >20%)
 */

import { useMemo, useCallback } from 'react';

export type SessionSort = 'recent' | 'tokens' | 'prompts' | 'files_edited' | 'duration';
export type GroupBy = 'none' | 'branch' | 'project' | 'model' | 'day' | 'week' | 'month';
export type ViewMode = 'timeline' | 'table';

export interface SessionFilters {
  // Sort and grouping
  sort: SessionSort;
  groupBy: GroupBy;
  viewMode: ViewMode;

  // Multi-select filters
  branches: string[];
  models: string[];

  // Boolean filters
  hasCommits: 'any' | 'yes' | 'no';
  hasSkills: 'any' | 'yes' | 'no';

  // Numeric filters
  minDuration: number | null; // seconds
  minFiles: number | null;
  minTokens: number | null;
  highReedit: boolean | null; // High re-edit rate (>20%)
}

export const DEFAULT_FILTERS: SessionFilters = {
  sort: 'recent',
  groupBy: 'none',
  viewMode: 'timeline',
  branches: [],
  models: [],
  hasCommits: 'any',
  hasSkills: 'any',
  minDuration: null,
  minFiles: null,
  minTokens: null,
  highReedit: null,
};

/**
 * Parse filters from URL search params.
 */
function parseFilters(searchParams: URLSearchParams): SessionFilters {
  return {
    sort: (searchParams.get('sort') || 'recent') as SessionSort,
    groupBy: (searchParams.get('groupBy') || 'none') as GroupBy,
    viewMode: (searchParams.get('viewMode') || 'timeline') as ViewMode,

    // Parse comma-separated lists
    branches: searchParams.get('branches')?.split(',').filter(Boolean) || [],
    models: searchParams.get('models')?.split(',').filter(Boolean) || [],

    // Parse boolean filters
    hasCommits: (searchParams.get('hasCommits') || 'any') as 'any' | 'yes' | 'no',
    hasSkills: (searchParams.get('hasSkills') || 'any') as 'any' | 'yes' | 'no',

    // Parse numeric filters
    minDuration: searchParams.has('minDuration') ? parseInt(searchParams.get('minDuration')!) : null,
    minFiles: searchParams.has('minFiles') ? parseInt(searchParams.get('minFiles')!) : null,
    minTokens: searchParams.has('minTokens') ? parseInt(searchParams.get('minTokens')!) : null,

    // Parse boolean for high re-edit rate
    highReedit: searchParams.has('highReedit') ? searchParams.get('highReedit') === 'true' : null,
  };
}

/** Keys managed by this module — used to clean stale params before merging. */
const FILTER_KEYS = [
  'sort', 'groupBy', 'viewMode',
  'branches', 'models',
  'hasCommits', 'hasSkills',
  'minDuration', 'minFiles', 'minTokens',
  'highReedit',
] as const;

/**
 * Serialize filters into an existing URLSearchParams, preserving any
 * params that belong to other systems (e.g. the legacy "filter" param).
 */
function serializeFilters(filters: SessionFilters, existing: URLSearchParams): URLSearchParams {
  const params = new URLSearchParams(existing);

  // Clear all keys we own so stale values don't linger
  for (const key of FILTER_KEYS) {
    params.delete(key);
  }

  // Only set non-default values
  if (filters.sort !== 'recent') {
    params.set('sort', filters.sort);
  }

  if (filters.groupBy !== 'none') {
    params.set('groupBy', filters.groupBy);
  }

  if (filters.viewMode !== 'timeline') {
    params.set('viewMode', filters.viewMode);
  }

  if (filters.branches.length > 0) {
    params.set('branches', filters.branches.join(','));
  }

  if (filters.models.length > 0) {
    params.set('models', filters.models.join(','));
  }

  if (filters.hasCommits !== 'any') {
    params.set('hasCommits', filters.hasCommits);
  }

  if (filters.hasSkills !== 'any') {
    params.set('hasSkills', filters.hasSkills);
  }

  if (filters.minDuration !== null) {
    params.set('minDuration', String(filters.minDuration));
  }

  if (filters.minFiles !== null) {
    params.set('minFiles', String(filters.minFiles));
  }

  if (filters.minTokens !== null) {
    params.set('minTokens', String(filters.minTokens));
  }

  if (filters.highReedit !== null) {
    params.set('highReedit', String(filters.highReedit));
  }

  return params;
}

/**
 * Count how many active filters are set (excluding sort and groupBy).
 */
export function countActiveFilters(filters: SessionFilters): number {
  let count = 0;

  if (filters.branches.length > 0) count++;
  if (filters.models.length > 0) count++;
  if (filters.hasCommits !== 'any') count++;
  if (filters.hasSkills !== 'any') count++;
  if (filters.minDuration !== null) count++;
  if (filters.minFiles !== null) count++;
  if (filters.minTokens !== null) count++;
  if (filters.highReedit !== null) count++;

  return count;
}

/**
 * Hook to manage session filters with URL persistence.
 *
 * @example
 * ```tsx
 * const [filters, setFilters] = useSessionFilters(searchParams, setSearchParams);
 *
 * // Set individual filter
 * setFilters({ ...filters, branches: ['main', 'dev'] });
 *
 * // Clear all filters
 * setFilters(DEFAULT_FILTERS);
 * ```
 */
export function useSessionFilters(
  searchParams: URLSearchParams,
  setSearchParams: (params: URLSearchParams, opts?: { replace?: boolean }) => void
): [SessionFilters, (filters: SessionFilters) => void] {
  // Memoize on the URL string so `filters` keeps a stable reference across
  // re-renders that don't change the URL. Without this, every parent render
  // would produce a new object, breaking any useEffect([..., filters]) in
  // child components (e.g. FilterPopover's draft-reset effect).
  const urlKey = searchParams.toString();
  const filters = useMemo(() => parseFilters(searchParams), [urlKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const setFilters = useCallback(
    (newFilters: SessionFilters) => {
      const params = serializeFilters(newFilters, searchParams);
      setSearchParams(params, { replace: true });
    },
    [urlKey, setSearchParams], // eslint-disable-line react-hooks/exhaustive-deps
  );

  return [filters, setFilters];
}
