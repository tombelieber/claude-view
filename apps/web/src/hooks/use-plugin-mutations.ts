import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'
import type { PluginActionRequest, PluginActionResponse } from '../types/generated'

async function runPluginAction(req: PluginActionRequest): Promise<PluginActionResponse> {
  const res = await fetch('/api/plugins/action', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  })

  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }

  const data: PluginActionResponse = await res.json()
  if (!data.success) {
    throw new Error(data.message ?? 'Plugin action failed')
  }
  return data
}

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

export function usePluginMutations() {
  const queryClient = useQueryClient()

  const mutation = useMutation({
    mutationFn: runPluginAction,
    onSuccess: (_data, req) => {
      const verb = req.action === 'disable' ? 'Disabled' : `${capitalize(req.action)}ed`
      toast.success(`${verb} ${req.name}`, { duration: TOAST_DURATION.micro })
      queryClient.invalidateQueries({ queryKey: ['plugins'] })
    },
    onError: (err, req) => {
      toast.error(`Failed to ${req.action} ${req.name}: ${err.message}`, {
        duration: TOAST_DURATION.extended,
      })
    },
  })

  return {
    execute: mutation.mutateAsync,
    isPending: mutation.isPending,
    pendingName: mutation.isPending ? (mutation.variables?.name ?? null) : null,
  }
}
