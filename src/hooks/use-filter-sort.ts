import type { SessionFilter, SessionSort } from '../components/FilterSortBar'

/**
 * Hook to manage filter/sort state with URL persistence.
 */
export function useFilterSort(
  searchParams: URLSearchParams,
  setSearchParams: (params: URLSearchParams, opts?: { replace?: boolean }) => void
) {
  const filter = (searchParams.get('filter') || 'all') as SessionFilter
  const sort = (searchParams.get('sort') || 'recent') as SessionSort

  const setFilter = (newFilter: SessionFilter) => {
    const params = new URLSearchParams(searchParams)
    if (newFilter === 'all') {
      params.delete('filter')
    } else {
      params.set('filter', newFilter)
    }
    setSearchParams(params, { replace: true })
  }

  const setSort = (newSort: SessionSort) => {
    const params = new URLSearchParams(searchParams)
    if (newSort === 'recent') {
      params.delete('sort')
    } else {
      params.set('sort', newSort)
    }
    setSearchParams(params, { replace: true })
  }

  return { filter, sort, setFilter, setSort }
}
