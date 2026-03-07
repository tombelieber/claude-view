import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'

async function archiveSession(id: string): Promise<void> {
  const res = await fetch(`/api/sessions/${id}/archive`, { method: 'POST' })
  if (!res.ok) throw new Error(`Archive failed: ${res.status}`)
}

async function unarchiveSession(id: string): Promise<void> {
  const res = await fetch(`/api/sessions/${id}/unarchive`, { method: 'POST' })
  if (!res.ok) throw new Error(`Unarchive failed: ${res.status}`)
}

async function archiveSessionsBulk(ids: string[]): Promise<void> {
  const res = await fetch('/api/sessions/archive', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ids }),
  })
  if (!res.ok) throw new Error(`Bulk archive failed: ${res.status}`)
}

function invalidateSessionCaches(qc: ReturnType<typeof useQueryClient>) {
  qc.invalidateQueries({ queryKey: ['sessions'] })
  qc.invalidateQueries({ queryKey: ['sessions-infinite'] })
  qc.invalidateQueries({ queryKey: ['recent-sessions'] })
}

export function useArchiveSession() {
  const qc = useQueryClient()

  const archive = useMutation({
    mutationFn: archiveSession,
    onSuccess: (_data, sessionId) => {
      invalidateSessionCaches(qc)
      toast('Session archived', {
        action: {
          label: 'Undo',
          onClick: () => unarchiveMutation.mutate(sessionId),
        },
        duration: TOAST_DURATION.standard,
      })
    },
    onError: () => toast.error('Failed to archive session', { duration: TOAST_DURATION.extended }),
  })

  const unarchiveMutation = useMutation({
    mutationFn: unarchiveSession,
    onSuccess: () => {
      invalidateSessionCaches(qc)
      toast.success('Session restored')
    },
    onError: () => toast.error('Failed to restore session', { duration: TOAST_DURATION.extended }),
  })

  const bulkArchive = useMutation({
    mutationFn: archiveSessionsBulk,
    onSuccess: (_data, ids) => {
      invalidateSessionCaches(qc)
      toast(`${ids.length} sessions archived`, {
        action: {
          label: 'Undo',
          onClick: () => {
            Promise.all(ids.map(unarchiveSession))
              .then(() => {
                invalidateSessionCaches(qc)
                toast.success(`${ids.length} sessions restored`)
              })
              .catch((error: unknown) => {
                console.error('Failed to archive session:', error)
                invalidateSessionCaches(qc)
                toast.error('Failed to restore some sessions', {
                  duration: TOAST_DURATION.extended,
                })
              })
          },
        },
        duration: TOAST_DURATION.standard,
      })
    },
    onError: () => toast.error('Failed to archive sessions', { duration: TOAST_DURATION.extended }),
  })

  return { archive, unarchive: unarchiveMutation, bulkArchive }
}
