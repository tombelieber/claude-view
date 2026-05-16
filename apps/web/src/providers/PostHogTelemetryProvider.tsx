import { useConfig } from '@/hooks/use-config'
import { PostHogProvider } from '@posthog/react'
import posthog from 'posthog-js'
import { type ReactNode, useEffect, useRef } from 'react'

function PostHogInitializer({ children }: { children: ReactNode }) {
  const config = useConfig()
  const initialized = useRef(false)

  useEffect(() => {
    if (initialized.current || !config.posthogKey) return
    initialized.current = true

    posthog.init(config.posthogKey, {
      api_host: 'https://us.i.posthog.com',
      person_profiles: 'identified_only',
      persistence: 'localStorage',
      // Privacy hardening — the public promise is "no code, prompts, file
      // paths or session content, ever". autocapture, session recording,
      // and pageview/pageleave all carry DOM text or URLs (which embed
      // session ids and project paths), so they are hard-off. Only
      // explicit, closed-enum events are ever sent.
      capture_pageview: false,
      capture_pageleave: false,
      autocapture: false,
      disable_session_recording: true,
      disable_surveys: true,
      opt_out_capturing_by_default: true,
      bootstrap: {
        distinctID: config.anonymousId ?? undefined,
        isIdentifiedID: false,
      },
    })
    posthog.register({ app_version: config.version, source: 'web' })

    if (config.telemetry === 'enabled') {
      posthog.opt_in_capturing()
    }
  }, [config.posthogKey, config.anonymousId, config.telemetry, config.version])

  return <>{children}</>
}

export function PostHogTelemetryProvider({ children }: { children: ReactNode }) {
  return (
    <PostHogProvider client={posthog}>
      <PostHogInitializer>{children}</PostHogInitializer>
    </PostHogProvider>
  )
}
