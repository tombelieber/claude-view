import { useQuery } from '@tanstack/react-query'
import { supabase } from '../lib/supabase'
import type { ConfigResponse } from '../types/generated/ConfigResponse'

// Synchronous best-guess from client env vars — prevents flash-of-missing-content.
// placeholderData is shown immediately on first render while /api/config loads.
// Unlike initialData, placeholderData does NOT bypass the fetch — the server-
// authoritative value will replace this once the response arrives.
// sharing defaults to false (safe side) — disabled→enabled flicker is less jarring
// than enabled→disabled if auth is configured but sharing isn't.
const placeholder: ConfigResponse = {
  auth: supabase !== null,
  sharing: false,
  version: '',
}

const fallback: ConfigResponse = { auth: false, sharing: false, version: '' }

export function useConfig(): ConfigResponse {
  const { data } = useQuery<ConfigResponse>({
    queryKey: ['config'],
    queryFn: async () => {
      const res = await fetch('/api/config')
      if (!res.ok) return fallback
      try {
        return (await res.json()) as ConfigResponse
      } catch {
        return fallback
      }
    },
    placeholderData: placeholder, // Shown until server responds, then replaced
    staleTime: Number.POSITIVE_INFINITY, // Once fetched from server, never re-fetch
    refetchOnWindowFocus: false, // Config is static for the lifetime of the server
    retry: 2,
  })
  return data ?? fallback
}
