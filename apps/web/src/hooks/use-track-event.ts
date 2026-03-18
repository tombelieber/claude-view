import { usePostHog } from '@posthog/react'
import { useCallback } from 'react'

export function useTrackEvent() {
  const posthog = usePostHog()
  return useCallback(
    (event: string, properties?: Record<string, unknown>) => {
      posthog?.capture(event, { source: 'web', ...properties })
    },
    [posthog],
  )
}
