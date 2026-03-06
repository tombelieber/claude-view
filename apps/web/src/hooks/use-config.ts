import { useQuery } from '@tanstack/react-query'
import { supabase } from '../lib/supabase'

export interface AppConfig {
  auth: boolean
  sharing: boolean
  version: string
}

// Synchronous best-guess from client env vars — prevents flash-of-missing-content.
// placeholderData is shown immediately on first render while /api/config loads.
// Unlike initialData, placeholderData does NOT bypass the fetch — the server-
// authoritative value will replace this once the response arrives.
const placeholder: AppConfig = {
  auth: supabase !== null,
  sharing: supabase !== null, // best guess: if auth is configured, sharing likely is too
  version: '',
}

const fallback: AppConfig = { auth: false, sharing: false, version: '' }

export function useConfig(): AppConfig {
  const { data } = useQuery<AppConfig>({
    queryKey: ['config'],
    queryFn: async () => {
      const res = await fetch('/api/config')
      if (!res.ok) return fallback
      return res.json()
    },
    placeholderData: placeholder, // Shown until server responds, then replaced
    staleTime: Number.POSITIVE_INFINITY, // Once fetched from server, never re-fetch
    retry: 2,
  })
  return data ?? fallback
}
