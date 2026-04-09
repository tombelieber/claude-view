import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

export interface CliSession {
  id: string
  createdAt: number
  status: 'starting' | 'running' | 'detached' | 'exited'
  projectDir: string | null
  args: string[]
}

export function useCliSessions() {
  return useQuery({
    queryKey: ['cli-sessions'],
    queryFn: async (): Promise<CliSession[]> => {
      const resp = await fetch('/api/cli-sessions')
      if (!resp.ok) throw new Error('Failed to fetch CLI sessions')
      const data = await resp.json()
      return data.sessions
    },
    refetchInterval: 10_000,
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
