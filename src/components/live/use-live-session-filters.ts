import { useMemo, useCallback } from 'react'
import type {
  LiveSessionFilters,
  LiveSortField,
  LiveSortDirection,
} from './live-filter'
import { DEFAULT_LIVE_FILTERS } from './live-filter'

const FILTER_KEYS = ['status', 'project', 'branch', 'q', 'sort', 'dir'] as const

export function useLiveSessionFilters(
  searchParams: URLSearchParams,
  setSearchParams: (
    params: URLSearchParams,
    opts?: { replace?: boolean }
  ) => void
): [
  LiveSessionFilters,
  {
    setStatus: (statuses: string[]) => void
    setProjects: (projects: string[]) => void
    setBranches: (branches: string[]) => void
    setSearch: (query: string) => void
    setSort: (field: LiveSortField, dir?: LiveSortDirection) => void
    clearAll: () => void
    activeCount: number
  },
] {
  // Parse filters from URL params with stable memoization keyed on string
  // eslint-disable-next-line react-hooks/exhaustive-deps -- stable string key per CLAUDE.md
  const filters = useMemo((): LiveSessionFilters => {
    const statuses =
      searchParams
        .get('status')
        ?.split(',')
        .filter(Boolean) ?? []
    const projects =
      searchParams
        .get('project')
        ?.split(',')
        .filter(Boolean) ?? []
    const branches =
      searchParams
        .get('branch')
        ?.split(',')
        .filter(Boolean) ?? []
    const search = searchParams.get('q') ?? ''
    const sort =
      (searchParams.get('sort') as LiveSortField) ?? DEFAULT_LIVE_FILTERS.sort
    const sortDir =
      (searchParams.get('dir') as LiveSortDirection) ??
      DEFAULT_LIVE_FILTERS.sortDir
    return { statuses, projects, branches, search, sort, sortDir }
  }, [searchParams.toString()])

  const setStatus = useCallback(
    (statuses: string[]) => {
      const params = new URLSearchParams(searchParams)
      if (statuses.length > 0) {
        params.set('status', statuses.join(','))
      } else {
        params.delete('status')
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams]
  )

  const setProjects = useCallback(
    (projects: string[]) => {
      const params = new URLSearchParams(searchParams)
      if (projects.length > 0) {
        params.set('project', projects.join(','))
      } else {
        params.delete('project')
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams]
  )

  const setBranches = useCallback(
    (branches: string[]) => {
      const params = new URLSearchParams(searchParams)
      if (branches.length > 0) {
        params.set('branch', branches.join(','))
      } else {
        params.delete('branch')
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams]
  )

  const setSearch = useCallback(
    (query: string) => {
      const params = new URLSearchParams(searchParams)
      if (query) {
        params.set('q', query)
      } else {
        params.delete('q')
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams]
  )

  const setSort = useCallback(
    (field: LiveSortField, dir?: LiveSortDirection) => {
      const params = new URLSearchParams(searchParams)
      params.set('sort', field)
      if (dir) {
        params.set('dir', dir)
      } else {
        // Toggle direction if same field, otherwise default to asc
        const currentSort = searchParams.get('sort')
        const currentDir = searchParams.get('dir') ?? 'asc'
        if (currentSort === field) {
          params.set('dir', currentDir === 'asc' ? 'desc' : 'asc')
        } else {
          params.set('dir', 'asc')
        }
      }
      setSearchParams(params, { replace: true })
    },
    [searchParams, setSearchParams]
  )

  const clearAll = useCallback(() => {
    const params = new URLSearchParams(searchParams)
    for (const key of FILTER_KEYS) {
      params.delete(key)
    }
    setSearchParams(params, { replace: true })
  }, [searchParams, setSearchParams])

  // Count active filters (exclude sort/sortDir — those aren't "filters")
  const activeCount = useMemo(() => {
    let count = 0
    if (filters.statuses.length > 0) count++
    if (filters.projects.length > 0) count++
    if (filters.branches.length > 0) count++
    if (filters.search.trim()) count++
    return count
  }, [filters])

  return [
    filters,
    { setStatus, setProjects, setBranches, setSearch, setSort, clearAll, activeCount },
  ]
}
