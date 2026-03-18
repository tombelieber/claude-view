import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

// Mock useConfig to return undecided telemetry with a key
vi.mock('./use-config', () => ({
  useConfig: vi.fn(() => ({
    telemetry: 'undecided',
    posthogKey: 'phc_test',
    anonymousId: 'test-id',
    auth: false,
    sharing: false,
    version: '1.0.0',
  })),
}))

describe('useTelemetryPrompt', () => {
  let queryClient: QueryClient

  function createWrapper() {
    queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    return ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }

  beforeEach(() => {
    localStorage.clear()
    vi.clearAllMocks()
  })

  it('does not prompt before threshold (3 views)', async () => {
    const { useTelemetryPrompt } = await import('./use-telemetry-prompt')
    const { result } = renderHook(() => useTelemetryPrompt(), { wrapper: createWrapper() })
    expect(result.current.shouldPrompt).toBe(false)
  })

  it('prompts after 3 session views', async () => {
    const { useTelemetryPrompt } = await import('./use-telemetry-prompt')
    const { result } = renderHook(() => useTelemetryPrompt(), { wrapper: createWrapper() })

    act(() => {
      result.current.recordSessionView()
    })
    act(() => {
      result.current.recordSessionView()
    })
    expect(result.current.shouldPrompt).toBe(false)

    act(() => {
      result.current.recordSessionView()
    })
    expect(result.current.shouldPrompt).toBe(true)
  })

  it('persists count across hook remounts', async () => {
    localStorage.setItem('cv_session_views', '3')
    const { useTelemetryPrompt } = await import('./use-telemetry-prompt')
    const { result } = renderHook(() => useTelemetryPrompt(), { wrapper: createWrapper() })
    expect(result.current.shouldPrompt).toBe(true)
  })

  it('does not prompt when telemetry already decided', async () => {
    localStorage.setItem('cv_session_views', '10')
    const { useConfig } = await import('./use-config')
    vi.mocked(useConfig).mockReturnValue({
      telemetry: 'enabled',
      posthogKey: 'phc_test',
      anonymousId: 'test-id',
      auth: false,
      sharing: false,
      version: '1.0.0',
    })
    const { useTelemetryPrompt } = await import('./use-telemetry-prompt')
    const { result } = renderHook(() => useTelemetryPrompt(), { wrapper: createWrapper() })
    expect(result.current.shouldPrompt).toBe(false)
  })

  it('does not prompt when no posthog key (self-hosted)', async () => {
    localStorage.setItem('cv_session_views', '10')
    const { useConfig } = await import('./use-config')
    vi.mocked(useConfig).mockReturnValue({
      telemetry: 'undecided',
      posthogKey: null,
      anonymousId: null,
      auth: false,
      sharing: false,
      version: '1.0.0',
    })
    const { useTelemetryPrompt } = await import('./use-telemetry-prompt')
    const { result } = renderHook(() => useTelemetryPrompt(), { wrapper: createWrapper() })
    expect(result.current.shouldPrompt).toBe(false)
  })
})
