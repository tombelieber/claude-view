import { renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mockCapture = vi.fn()
vi.mock('@posthog/react', () => ({
  usePostHog: () => ({ capture: mockCapture }),
}))

describe('useTrackEvent', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('calls posthog.capture with event name and source:web', async () => {
    const { useTrackEvent } = await import('./use-track-event')
    const { result } = renderHook(() => useTrackEvent())
    result.current('session_opened', { some_prop: 'value' })
    expect(mockCapture).toHaveBeenCalledWith('session_opened', {
      source: 'web',
      some_prop: 'value',
    })
  })

  it('works with no extra properties', async () => {
    const { useTrackEvent } = await import('./use-track-event')
    const { result } = renderHook(() => useTrackEvent())
    result.current('live_monitor_viewed')
    expect(mockCapture).toHaveBeenCalledWith('live_monitor_viewed', { source: 'web' })
  })

  it('does not throw when posthog is null (self-hosted)', async () => {
    vi.doMock('@posthog/react', () => ({ usePostHog: () => null }))
    const { useTrackEvent } = await import('./use-track-event')
    const { result } = renderHook(() => useTrackEvent())
    expect(() => result.current('test_event')).not.toThrow()
  })
})
