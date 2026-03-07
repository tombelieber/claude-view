import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import type { GrepResponse } from '../types/generated'

interface UseGrepOptions {
  caseSensitive?: boolean
  wholeWord?: boolean
  project?: string
  limit?: number
  enabled?: boolean
}

export function useGrep(pattern: string, options: UseGrepOptions = {}) {
  const { caseSensitive = false, wholeWord = false, project, limit = 200, enabled = true } = options

  // 300ms debounce — slightly longer than useSearch (200ms) since grep
  // scans raw JSONL files and benefits from fewer in-flight requests.
  const [debouncedPattern, setDebouncedPattern] = useState(pattern)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedPattern(pattern), 300)
    return () => clearTimeout(timer)
  }, [pattern])

  const queryResult = useQuery<GrepResponse>({
    queryKey: ['grep', debouncedPattern, caseSensitive, wholeWord, project, limit],
    queryFn: async () => {
      const params = new URLSearchParams()
      params.set('pattern', debouncedPattern)
      params.set('limit', String(limit))
      if (caseSensitive) params.set('caseSensitive', 'true')
      if (wholeWord) params.set('wholeWord', 'true')
      if (project) params.set('project', project)
      const res = await fetch(`/api/grep?${params}`)
      if (!res.ok) throw new Error(await res.text())
      return res.json()
    },
    enabled: enabled && debouncedPattern.trim().length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  })

  return {
    ...queryResult,
    debouncedPattern,
    isDebouncing: pattern !== debouncedPattern,
  }
}
