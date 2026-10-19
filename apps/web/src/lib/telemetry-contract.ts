import type { CaptureResult } from 'posthog-js'

export function telemetryDistinctId(id: string): string {
  return id.startsWith('claude_view:') ? id : `claude_view:${id}`
}

/** Only explicit product events survive; SDK URLs, identities and DOM never do. */
export function telemetryBeforeSend(version: string, id: string) {
  const distinctId = telemetryDistinctId(id)
  return (event: CaptureResult | null): CaptureResult | null => {
    if (!event) return null
    const props: Record<string, unknown> = {}
    switch (event.event) {
      case 'session_opened':
      case 'live_monitor_viewed':
      case 'plugin_installed':
        break
      case 'search_used':
        for (const key of ['query_length', 'result_count']) {
          const value = event.properties[key]
          if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) {
            props[key] = value
          }
        }
        break
      case 'theme_toggled':
        if (['light', 'dark', 'system'].includes(event.properties.to)) {
          props.to = event.properties.to
        }
        break
      case 'settings_changed':
        props.changed_fields = ['git_sync_interval']
        break
      default:
        return null
    }
    return {
      uuid: event.uuid,
      event: event.event,
      timestamp: event.timestamp,
      properties: {
        ...props,
        token: event.properties.token,
        distinct_id: distinctId,
        product: 'claude_view',
        schema_version: 1,
        source: 'web',
        deployment: 'local_app',
        environment: distinctId.startsWith('claude_view:qualification:')
          ? 'qualification'
          : 'production',
        app_version: version,
        $process_person_profile: false,
        $geoip_disable: true,
        $ip: null,
      },
    }
  }
}
