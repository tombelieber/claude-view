import { useCallback, useEffect, useState } from 'react'
import { useConfig } from './use-config'

const STORAGE_KEY = 'cv_session_views'
const PROMPT_THRESHOLD = 3

/**
 * Tracks session detail views and returns whether the telemetry consent
 * dialog should be shown. Only triggers after the user has opened
 * {@link PROMPT_THRESHOLD} sessions — by then they're invested enough
 * to make an informed choice instead of reflexively dismissing.
 *
 * Returns `false` when telemetry is already decided (enabled or disabled)
 * or when there's no PostHog key (self-hosted).
 */
export function useTelemetryPrompt() {
  const config = useConfig()
  const [shouldPrompt, setShouldPrompt] = useState(false)

  const isUndecided = config.telemetry === 'undecided' && config.posthogKey !== null

  // Increment counter when a session is viewed
  const recordSessionView = useCallback(() => {
    if (!isUndecided) return
    const current = Number(localStorage.getItem(STORAGE_KEY) ?? '0')
    const next = current + 1
    localStorage.setItem(STORAGE_KEY, String(next))
    if (next >= PROMPT_THRESHOLD) {
      setShouldPrompt(true)
    }
  }, [isUndecided])

  // Check on mount if threshold already reached
  useEffect(() => {
    if (!isUndecided) {
      setShouldPrompt(false)
      return
    }
    const current = Number(localStorage.getItem(STORAGE_KEY) ?? '0')
    if (current >= PROMPT_THRESHOLD) {
      setShouldPrompt(true)
    }
  }, [isUndecided])

  return { shouldPrompt, recordSessionView }
}
