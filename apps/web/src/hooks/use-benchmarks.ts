import { useQuery } from '@tanstack/react-query'
import type { BenchmarksResponseWithMeta } from '../types/generated/BenchmarksResponseWithMeta'

// The endpoint returns `BenchmarksResponseWithMeta`, which carries an always-present
// `meta: AnalyticsScopeMeta` (non-Option in the backend) alongside the flattened
// benchmarks fields.
export type BenchmarksResponse = BenchmarksResponseWithMeta

interface UseBenchmarksOptions {
  range?: 'all' | '30d' | '90d' | '1y'
}

async function fetchBenchmarks(range: string): Promise<BenchmarksResponse> {
  const response = await fetch(`/api/insights/benchmarks?range=${range}`)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch benchmarks: ${errorText}`)
  }
  return response.json()
}

export function useBenchmarks({ range = 'all' }: UseBenchmarksOptions = {}) {
  return useQuery({
    queryKey: ['benchmarks', range],
    queryFn: () => fetchBenchmarks(range),
    staleTime: 60_000, // Cache for 1 minute (benchmarks change slowly)
    refetchOnWindowFocus: false,
  })
}
