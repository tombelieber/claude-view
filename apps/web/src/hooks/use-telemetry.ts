import { useQueryClient } from '@tanstack/react-query'
import posthog from 'posthog-js'
import { useCallback, useState } from 'react'
import { toast } from 'sonner'

export function useTelemetry() {
  const queryClient = useQueryClient()
  const [isPending, setIsPending] = useState(false)

  const enableTelemetry = useCallback(async () => {
    setIsPending(true)
    try {
      const res = await fetch('/api/telemetry/consent', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: true }),
      })
      if (!res.ok) throw new Error(`Server responded ${res.status}`)
      posthog.opt_in_capturing()
      queryClient.invalidateQueries({ queryKey: ['config'] })
    } catch {
      toast.error('Failed to save preference — please try again')
    } finally {
      setIsPending(false)
    }
  }, [queryClient])

  const disableTelemetry = useCallback(async () => {
    setIsPending(true)
    try {
      const res = await fetch('/api/telemetry/consent', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: false }),
      })
      if (!res.ok) throw new Error(`Server responded ${res.status}`)
      posthog.opt_out_capturing()
      queryClient.invalidateQueries({ queryKey: ['config'] })
    } catch {
      toast.error('Failed to save preference — please try again')
    } finally {
      setIsPending(false)
    }
  }, [queryClient])

  return { enableTelemetry, disableTelemetry, isPending }
}
