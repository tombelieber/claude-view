import { useQuery } from '@tanstack/react-query'
import type { ReportPreview } from '../types/generated/ReportPreview'

async function fetchPreview(startTs: number, endTs: number): Promise<ReportPreview> {
  const res = await fetch(`/api/reports/preview?startTs=${startTs}&endTs=${endTs}`)
  if (!res.ok) throw new Error(`Failed to fetch report preview: ${await res.text()}`)
  return res.json()
}

export function useReportPreview(startTs: number, endTs: number) {
  return useQuery({
    queryKey: ['report-preview', startTs, endTs],
    queryFn: () => fetchPreview(startTs, endTs),
    staleTime: 30_000,
    enabled: startTs > 0 && endTs > 0,
  })
}
