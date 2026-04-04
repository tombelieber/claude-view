/**
 * Centralized session mutation hook — replaces 11+ scattered fetch() calls
 * to /api/sidecar/sessions/* across 5+ files.
 *
 * Each mutation invalidates relevant query caches on success and shows
 * consistent toast notifications.
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

const TOAST_DURATION = { micro: 2000, extended: 5000 }

/** Invalidate all session list caches after a mutation. */
function invalidateSessionCaches(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ['chat-sidebar-sessions'] })
  qc.invalidateQueries({ queryKey: ['sessions-infinite'] })
  qc.invalidateQueries({ queryKey: ['recent-sessions'] })
  qc.invalidateQueries({ queryKey: ['sessions'] })
  qc.invalidateQueries({ queryKey: ['server-activity'] })
  qc.invalidateQueries({ queryKey: ['session-activity'] })
  qc.invalidateQueries({ queryKey: ['activity-sessions-light'] })
}

// ============================================================================
// Create
// ============================================================================

interface CreateSessionInput {
  initialMessage: string
}

interface CreateSessionResult {
  sessionId: string
}

// ============================================================================
// Resume
// ============================================================================

interface ResumeSessionResult {
  controlId?: string
  error?: string
}

// ============================================================================
// Fork
// ============================================================================

interface ForkSessionInput {
  sessionId: string
  projectPath?: string
}

interface ForkSessionResult {
  sessionId?: string
  error?: string
}

// ============================================================================
// Hook
// ============================================================================

export function useSessionMutations() {
  const qc = useQueryClient()

  const createSession = useMutation({
    mutationFn: async (input: CreateSessionInput): Promise<CreateSessionResult> => {
      const res = await fetch('/api/sidecar/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initialMessage: input.initialMessage }),
      })
      if (!res.ok) {
        const body = await res.json().catch(() => ({}))
        throw new Error(body.error ?? `Failed to create session: ${res.status}`)
      }
      const data = await res.json()
      if (!data.sessionId) throw new Error('No sessionId returned from server')
      return { sessionId: data.sessionId }
    },
    onSuccess: () => invalidateSessionCaches(qc),
    onError: (err: Error) => {
      toast.error('Failed to start session', {
        description: err.message,
        duration: TOAST_DURATION.extended,
      })
    },
  })

  const resumeSession = useMutation({
    mutationFn: async (sessionId: string): Promise<ResumeSessionResult> => {
      const res = await fetch(`/api/sidecar/sessions/${sessionId}/resume`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })
      return res.json()
    },
    onSuccess: (data, _sessionId) => {
      if (data.controlId) {
        toast.success('Session resumed', { duration: TOAST_DURATION.micro })
        invalidateSessionCaches(qc)
      } else {
        toast.error('Resume failed', {
          description: data.error ?? 'Unknown error',
          duration: TOAST_DURATION.extended,
        })
      }
    },
    onError: (err: Error) => {
      toast.error('Failed to resume session', {
        description: err.message,
        duration: TOAST_DURATION.extended,
      })
    },
  })

  const deleteSession = useMutation({
    mutationFn: async (sessionId: string): Promise<void> => {
      const res = await fetch(`/api/sidecar/sessions/${sessionId}`, {
        method: 'DELETE',
      })
      if (!res.ok) {
        const body = await res.json().catch(() => ({}))
        throw new Error(body.error ?? `Failed to delete session: ${res.status}`)
      }
    },
    onSuccess: () => {
      toast.success('Session shut down', { duration: TOAST_DURATION.micro })
      invalidateSessionCaches(qc)
    },
    onError: (err: Error) => {
      toast.error('Failed to shut down session', {
        description: err.message,
        duration: TOAST_DURATION.extended,
      })
    },
  })

  const forkSession = useMutation({
    mutationFn: async (input: ForkSessionInput): Promise<ForkSessionResult> => {
      const res = await fetch(`/api/sidecar/sessions/${input.sessionId}/fork`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ projectPath: input.projectPath }),
      })
      return res.json()
    },
    onSuccess: (data) => {
      if (data.sessionId) {
        toast.success('Session forked', { duration: TOAST_DURATION.micro })
        invalidateSessionCaches(qc)
      } else {
        toast.error('Fork failed', {
          description: data.error,
          duration: TOAST_DURATION.extended,
        })
      }
    },
    onError: (err: Error) => {
      toast.error('Failed to fork session', {
        description: err.message,
        duration: TOAST_DURATION.extended,
      })
    },
  })

  return {
    createSession,
    resumeSession,
    deleteSession,
    forkSession,
  }
}
