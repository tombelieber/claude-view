import { useQuery } from '@tanstack/react-query'
import { useState, useEffect } from 'react'
import type { SearchResponse } from '../types/generated'

interface UseSearchOptions {
  scope?: string
  limit?: number
  enabled?: boolean
}

export function useSearch(query: string, options: UseSearchOptions = {}) {
  const { scope, limit = 20, enabled = true } = options

  // Debounce query by 200ms
  const [debouncedQuery, setDebouncedQuery] = useState(query)

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), 200)
    return () => clearTimeout(timer)
  }, [query])

  const queryResult = useQuery<SearchResponse>({
    queryKey: ['search', debouncedQuery, scope, limit],
    queryFn: async () => {
      const params = new URLSearchParams()
      params.set('q', debouncedQuery)
      if (scope) params.set('scope', scope)
      params.set('limit', String(limit))

      const res = await fetch(`/api/search?${params}`)
      if (!res.ok) {
        throw new Error(`Search failed: ${res.statusText}`)
      }
      return res.json()
    },
    enabled: enabled && debouncedQuery.trim().length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  })

  return {
    ...queryResult,
    debouncedQuery,
    isDebouncing: query !== debouncedQuery,
  }
}
