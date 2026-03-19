import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mockOptIn = vi.fn()
const mockOptOut = vi.fn()
vi.mock('posthog-js', () => ({
  default: {
    opt_in_capturing: mockOptIn,
    opt_out_capturing: mockOptOut,
  },
}))

describe('useTelemetry', () => {
  let fetchSpy: ReturnType<typeof vi.spyOn>
  let queryClient: QueryClient

  function createWrapper() {
    queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    return ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
  }

  beforeEach(() => {
    fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(JSON.stringify({ status: 'enabled' }), { status: 200 }))
    vi.clearAllMocks()
  })

  afterEach(() => {
    fetchSpy.mockRestore()
  })

  it('enableTelemetry calls POST with enabled:true and opts in posthog', async () => {
    const { useTelemetry } = await import('./use-telemetry')
    const { result } = renderHook(() => useTelemetry(), { wrapper: createWrapper() })
    await act(async () => {
      await result.current.enableTelemetry()
    })
    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/telemetry/consent',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ enabled: true }),
      }),
    )
    expect(mockOptIn).toHaveBeenCalledOnce()
  })

  it('disableTelemetry calls POST with enabled:false and opts out posthog', async () => {
    const { useTelemetry } = await import('./use-telemetry')
    const { result } = renderHook(() => useTelemetry(), { wrapper: createWrapper() })
    await act(async () => {
      await result.current.disableTelemetry()
    })
    expect(fetchSpy).toHaveBeenCalledWith(
      '/api/telemetry/consent',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ enabled: false }),
      }),
    )
    expect(mockOptOut).toHaveBeenCalledOnce()
  })

  it('both functions invalidate config query cache', async () => {
    const { useTelemetry } = await import('./use-telemetry')
    const { result } = renderHook(() => useTelemetry(), { wrapper: createWrapper() })
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries')
    await act(async () => {
      await result.current.enableTelemetry()
    })
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ['config'] })
  })
})
