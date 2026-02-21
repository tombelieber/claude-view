import { useQuery, useQueryClient } from '@tanstack/react-query'
import type { AppSettings } from '../types/generated/AppSettings'

async function fetchSettings(): Promise<AppSettings> {
  const res = await fetch('/api/settings')
  if (!res.ok) throw new Error(`Failed to fetch settings: ${await res.text()}`)
  return res.json()
}

export function useAppSettings() {
  const queryClient = useQueryClient()

  const { data, error, isLoading } = useQuery({
    queryKey: ['app-settings'],
    queryFn: fetchSettings,
    staleTime: 30_000,
  })

  const updateSettings = async (updates: Partial<AppSettings>) => {
    const res = await fetch('/api/settings', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updates),
    })
    if (!res.ok) throw new Error(await res.text())
    const updated: AppSettings = await res.json()
    queryClient.setQueryData(['app-settings'], updated)
    return updated
  }

  return { settings: data, error, isLoading, updateSettings }
}
