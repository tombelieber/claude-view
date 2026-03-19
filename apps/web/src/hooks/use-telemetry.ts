import { useQueryClient } from '@tanstack/react-query'
import posthog from 'posthog-js'

export function useTelemetry() {
  const queryClient = useQueryClient()

  const enableTelemetry = async () => {
    await fetch('/api/telemetry/consent', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: true }),
    })
    posthog.opt_in_capturing()
    queryClient.invalidateQueries({ queryKey: ['config'] })
  }

  const disableTelemetry = async () => {
    await fetch('/api/telemetry/consent', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: false }),
    })
    posthog.opt_out_capturing()
    queryClient.invalidateQueries({ queryKey: ['config'] })
  }

  return { enableTelemetry, disableTelemetry }
}
