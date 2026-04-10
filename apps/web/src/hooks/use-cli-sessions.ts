import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'

export interface CliSession {
  id: string
  createdAt: number
  status: 'running' | 'exited'
  projectDir: string | null
  args: string[]
}

/**
 * Listen for CLI session SSE events on the existing Live Monitor EventSource.
 *
 * The live SSE stream (/api/live/stream) is already open via useLiveSessions().
 * We add listeners to it via a global reference rather than opening a second
 * connection. The events are: cli_session_created, cli_session_updated,
 * cli_session_removed.
 *
 * Implementation: window-level custom events dispatched by the SSE stream,
 * avoiding tight coupling between useLiveSessions and useCliSessions.
 */
function useCliSessionSSE() {
  const queryClient = useQueryClient()

  useEffect(() => {
    const updateCache = (updater: (prev: CliSession[]) => CliSession[]) => {
      queryClient.setQueryData<CliSession[]>(['cli-sessions'], (prev) => updater(prev ?? []))
    }

    const handleCreated = (e: Event) => {
      const { cliSession } = (e as CustomEvent).detail
      const session: CliSession = {
        id: cliSession.id,
        createdAt: cliSession.createdAt,
        status: cliSession.status,
        projectDir: cliSession.projectDir ?? null,
        args: [],
      }
      updateCache((prev) => {
        if (prev.some((s) => s.id === session.id)) return prev
        return [session, ...prev]
      })
    }

    const handleUpdated = (e: Event) => {
      const { cliSession } = (e as CustomEvent).detail
      updateCache((prev) =>
        prev.map((s) => (s.id === cliSession.id ? { ...s, status: cliSession.status } : s)),
      )
    }

    const handleRemoved = (e: Event) => {
      const { cliSessionId } = (e as CustomEvent).detail
      updateCache((prev) => prev.filter((s) => s.id !== cliSessionId))
    }

    window.addEventListener('cv:cli_session_created', handleCreated)
    window.addEventListener('cv:cli_session_updated', handleUpdated)
    window.addEventListener('cv:cli_session_removed', handleRemoved)

    return () => {
      window.removeEventListener('cv:cli_session_created', handleCreated)
      window.removeEventListener('cv:cli_session_updated', handleUpdated)
      window.removeEventListener('cv:cli_session_removed', handleRemoved)
    }
  }, [queryClient])
}

export function useCliSessions() {
  useCliSessionSSE()

  return useQuery({
    queryKey: ['cli-sessions'],
    queryFn: async (): Promise<CliSession[]> => {
      const resp = await fetch('/api/cli-sessions')
      if (!resp.ok) throw new Error('Failed to fetch CLI sessions')
      const data = await resp.json()
      return data.sessions
    },
    // No refetchInterval — SSE events + background health check keep data fresh.
    staleTime: 60_000,
  })
}

export function useCreateCliSession() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (opts: { projectDir?: string; args?: string[] }) => {
      const resp = await fetch('/api/cli-sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(opts),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({}))
        throw new Error(data.details ?? data.error ?? `HTTP ${resp.status}`)
      }
      return resp.json()
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cli-sessions'] }),
  })
}

export function useKillCliSession() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (sessionId: string) => {
      const resp = await fetch(`/api/cli-sessions/${sessionId}`, { method: 'DELETE' })
      if (!resp.ok) throw new Error('Failed to kill session')
      return resp.json()
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cli-sessions'] }),
  })
}
