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
      capture_pageview: false,
      capture_pageleave: true,
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
