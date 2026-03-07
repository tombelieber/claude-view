import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { getAccessToken } from '../lib/supabase'

interface ShareResponse {
  token: string
  url: string
}

interface ShareListItem {
  token: string
  session_id: string
  title: string | null
  size_bytes: number
  created_at: number
  view_count: number
  url: string | null
}

async function authHeaders(): Promise<Record<string, string>> {
  const token = await getAccessToken()
  return token ? { Authorization: `Bearer ${token}` } : {}
}

async function createShare(sessionId: string): Promise<ShareResponse> {
  const headers = await authHeaders()
  const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/share`, {
    method: 'POST',
    headers,
  })
  if (res.status === 401) throw new Error('AUTH_REQUIRED')
  if (!res.ok) throw new Error(`Share failed: ${res.status}`)
  return res.json()
}

async function revokeShare(sessionId: string): Promise<void> {
  const headers = await authHeaders()
  const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/share`, {
    method: 'DELETE',
    headers,
  })
  if (!res.ok) throw new Error('Revoke failed')
}

async function fetchShares(): Promise<ShareListItem[]> {
  const headers = await authHeaders()
  const res = await fetch('/api/shares', { headers })
  if (res.status === 401) return [] // not signed in — no shares to show
  if (!res.ok) throw new Error(`Failed to load shares: ${res.status}`)
  const data = await res.json()
  return (data.shares ?? []).map((s: Omit<ShareListItem, 'url'>) => ({
    ...s,
    url: localStorage.getItem(`share_url:${s.token}`),
  }))
}

export function useCreateShare() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: createShare,
    onSuccess: (data) => {
      if (data.url) localStorage.setItem(`share_url:${data.token}`, data.url)
      queryClient.invalidateQueries({ queryKey: ['shares'] })
    },
  })
}

export function useRevokeShare() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: revokeShare,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['shares'] }),
  })
}

export function useShares() {
  return useQuery({ queryKey: ['shares'], queryFn: fetchShares })
}
