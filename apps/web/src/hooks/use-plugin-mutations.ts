import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../lib/notify'
import type { PluginActionRequest, PluginOp } from '../types/generated'

async function enqueueOp(req: PluginActionRequest): Promise<PluginOp> {
  const res = await fetch('/api/plugins/ops', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  })

  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }

  return res.json()
}

async function fetchOps(): Promise<PluginOp[]> {
  const res = await fetch('/api/plugins/ops')
  if (!res.ok) return []
  return res.json()
}

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

export function usePluginMutations() {
  const queryClient = useQueryClient()
  // Track which op IDs we've already toasted so we don't repeat
  const toastedRef = useRef<Set<string>>(new Set())

  // Poll ops every 1s while there are active (queued/running) ops
  const { data: ops = [] } = useQuery({
    queryKey: ['plugin-ops'],
    queryFn: fetchOps,
    refetchInterval: (query) => {
      const data = query.state.data ?? []
      const hasActive = data.some((op) => op.status === 'queued' || op.status === 'running')
      return hasActive ? 1000 : false
    },
  })

  // Toast on completion/failure and invalidate plugin list
  useEffect(() => {
    for (const op of ops) {
      if (toastedRef.current.has(op.id)) continue

      if (op.status === 'completed') {
        toastedRef.current.add(op.id)
        const verb = op.action === 'disable' ? 'Disabled' : `${capitalize(op.action)}ed`
        toast.success(`${verb} ${op.name}`, { duration: TOAST_DURATION.micro })
        queryClient.invalidateQueries({ queryKey: ['plugins'] })
      } else if (op.status === 'failed') {
        toastedRef.current.add(op.id)
        toast.error(`Failed to ${op.action} ${op.name}: ${op.error ?? 'Unknown error'}`, {
          duration: TOAST_DURATION.extended,
        })
      }
    }
  }, [ops, queryClient])

  const execute = useCallback(
    async (req: PluginActionRequest) => {
      try {
        await enqueueOp(req)
        // Immediately refetch ops to start polling
        queryClient.invalidateQueries({ queryKey: ['plugin-ops'] })
      } catch (err) {
        toast.error(
          `Failed to ${req.action} ${req.name}: ${err instanceof Error ? err.message : 'Unknown error'}`,
          { duration: TOAST_DURATION.extended },
        )
      }
    },
    [queryClient],
  )

  // Any active op makes the overall state "pending"
  const hasActive = ops.some((op) => op.status === 'queued' || op.status === 'running')

  // Build set of pending plugin names for per-card status
  const pendingNames = new Set<string>()
  for (const op of ops) {
    if (op.status === 'queued' || op.status === 'running') {
      pendingNames.add(op.name)
    }
  }

  return {
    execute,
    isPending: hasActive,
    // Backwards-compat: return the first pending name (multiple can exist now)
    pendingName: hasActive
      ? (ops.find((op) => op.status === 'queued' || op.status === 'running')?.name ?? null)
      : null,
    // New: per-card check
    isPluginPending: (name: string) => pendingNames.has(name),
    ops,
  }
}
