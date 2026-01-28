import { useQuery } from '@tanstack/react-query'
import type { DashboardStats } from '../types/generated'

async function fetchDashboardStats(): Promise<DashboardStats> {
  const response = await fetch('/api/stats/dashboard')
  if (!response.ok) throw new Error('Failed to fetch dashboard stats')
  return response.json()
}

export function useDashboardStats() {
  return useQuery({
    queryKey: ['dashboard-stats'],
    queryFn: fetchDashboardStats,
  })
}
