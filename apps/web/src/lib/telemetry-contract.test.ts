import type { CaptureResult } from 'posthog-js'
import { describe, expect, it } from 'vitest'
import { telemetryBeforeSend, telemetryDistinctId } from './telemetry-contract'

const capture = (event: string, properties = {}): CaptureResult => ({
  uuid: 'test-event',
  event,
  properties,
})

describe('shared OSS telemetry boundary', () => {
  it('namespaces old install IDs without double-prefixing or aliasing products', () => {
    expect(telemetryDistinctId('old-uuid')).toBe('claude_view:old-uuid')
    expect(telemetryDistinctId('claude_view:old-uuid')).toBe('claude_view:old-uuid')
    expect(telemetryDistinctId('app_canvas:old-uuid')).toBe('claude_view:app_canvas:old-uuid')
  })

  it('overrides spoofed metadata and strips SDK URLs, email, content and profile writes', () => {
    const event = {
      ...capture('session_opened', {
        token: 'phc_test',
        product: 'app_canvas',
        schema_version: 99,
        distinct_id: 'account-id',
        email: 'test@example.com',
        $current_url: 'http://localhost/private-session',
        plugin_name: 'private-plugin',
      }),
      $set: { email: 'test@example.com' },
    }
    const result = telemetryBeforeSend('1.2.3', 'old-uuid')(event)
    expect(result?.properties).toEqual({
      token: 'phc_test',
      distinct_id: 'claude_view:old-uuid',
      product: 'claude_view',
      schema_version: 1,
      source: 'web',
      deployment: 'local_app',
      environment: 'production',
      app_version: '1.2.3',
      $process_person_profile: false,
      $geoip_disable: true,
      $ip: null,
    })
    expect(result).not.toHaveProperty('$set')
  })

  it('rejects automatic SDK events, identification and unknown events', () => {
    for (const event of ['$identify', '$pageview', '$web_vitals', '$opt_in', 'unknown']) {
      expect(telemetryBeforeSend('1', 'id')(capture(event))).toBeNull()
    }
  })

  it('retains coarse counters, but no free-form search text or invalid counters', () => {
    const props = telemetryBeforeSend(
      '1',
      'id',
    )(
      capture('search_used', {
        query_length: 4,
        result_count: -1,
        query: 'private query',
      }),
    )?.properties
    expect(props?.query_length).toBe(4)
    expect(props).not.toHaveProperty('result_count')
    expect(props).not.toHaveProperty('query')
  })

  it('marks reserved qualification installs for exclusion from product dashboards', () => {
    const result = telemetryBeforeSend(
      '1',
      'qualification:synthetic',
    )(capture('live_monitor_viewed'))
    expect(result?.properties.environment).toBe('qualification')
  })
})
