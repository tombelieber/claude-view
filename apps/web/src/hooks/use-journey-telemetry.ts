import { useEffect, useRef } from 'react'
import { useLocation } from 'react-router-dom'
import { surfaceForPath } from '@/lib/journey-route'
import { trackFeatureOpened } from '@/lib/telemetry-track'

/**
 * Single, systematic journey-telemetry wiring point. Mounted once in `App`
 * (inside the router), it fires `feature_opened { surface }` the first
 * time each surface is visited in an app session as the user navigates —
 * giving "which features get used" + the PostHog Paths journey with zero
 * per-screen instrumentation.
 *
 * Dedupe is per-session (a ref Set); the server additionally guards the
 * once-ever `first_feature_used` activation event. Unknown routes emit
 * nothing (handled by {@link surfaceForPath}). The server decides, from
 * resolved consent, whether to actually forward — the client never gates.
 */
export function useJourneyTelemetry(): void {
  const { pathname } = useLocation()
  const seen = useRef<Set<string>>(new Set())

  useEffect(() => {
    const surface = surfaceForPath(pathname)
    if (!surface || seen.current.has(surface)) return
    seen.current.add(surface)
    trackFeatureOpened(surface)
  }, [pathname])
}
