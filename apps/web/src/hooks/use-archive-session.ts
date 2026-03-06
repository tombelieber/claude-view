import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

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

export function useArchiveSession() {
  const qc = useQueryClient()

  const archive = useMutation({
    mutationFn: archiveSession,
    onSuccess: (_data, sessionId) => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast('Session archived', {
        action: {
          label: 'Undo',
          onClick: () => unarchiveMutation.mutate(sessionId),
        },
        duration: 5000,
      })
    },
    onError: () => toast.error('Failed to archive session'),
  })

  const unarchiveMutation = useMutation({
    mutationFn: unarchiveSession,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast.success('Session restored')
    },
    onError: () => toast.error('Failed to restore session'),
  })

  const bulkArchive = useMutation({
    mutationFn: archiveSessionsBulk,
    onSuccess: (_data, ids) => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
      qc.invalidateQueries({ queryKey: ['recent-sessions'] })
      toast(`${ids.length} sessions archived`, {
        action: {
          label: 'Undo',
          onClick: () => {
            Promise.all(ids.map(unarchiveSession)).then(() => {
              qc.invalidateQueries({ queryKey: ['sessions'] })
              toast.success(`${ids.length} sessions restored`)
            })
          },
        },
        duration: 5000,
      })
    },
    onError: () => toast.error('Failed to archive sessions'),
  })

  return { archive, unarchive: unarchiveMutation, bulkArchive }
}
