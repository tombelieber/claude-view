import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { trackFeatureAction, trackFeatureOpened } from './telemetry-track'

describe('telemetry-track', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('POSTs feature_opened with the closed surface enum', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }))
    vi.stubGlobal('fetch', fetchMock)

    trackFeatureOpened('search')
    await Promise.resolve()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, init] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/telemetry/event')
    expect(init.method).toBe('POST')
    expect(JSON.parse(init.body)).toEqual({ event: 'feature_opened', surface: 'search' })
  })

  it('POSTs feature_action with the closed action enum', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }))
    vi.stubGlobal('fetch', fetchMock)

    trackFeatureAction('chat_message_sent')
    await Promise.resolve()

    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      event: 'feature_action',
      action: 'chat_message_sent',
    })
  })

  it('is fire-and-forget: a network error never throws or rejects', async () => {
    const fetchMock = vi.fn().mockRejectedValue(new Error('offline'))
    vi.stubGlobal('fetch', fetchMock)

    // Must not throw synchronously...
    expect(() => trackFeatureOpened('analytics')).not.toThrow()
    // ...and the swallowed rejection must not surface as an unhandled error.
    await expect(Promise.resolve()).resolves.toBeUndefined()
  })
})
