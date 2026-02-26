import { useQuery, useQueryClient } from '@tanstack/react-query'
import type { ReportRow } from '../types/generated/ReportRow'

async function fetchReports(): Promise<ReportRow[]> {
  const res = await fetch('/api/reports')
  if (!res.ok) throw new Error(`Failed to fetch reports: ${await res.text()}`)
  return res.json()
}

export function useReports() {
  return useQuery({
    queryKey: ['reports'],
    queryFn: fetchReports,
    staleTime: 10_000,
  })
}

/** Hook to get mutate function for invalidating reports cache. */
export function useReportsMutate() {
  const queryClient = useQueryClient()
  return () => queryClient.invalidateQueries({ queryKey: ['reports'] })
}
