import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import { createElement } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useOAuthUsage } from './use-oauth-usage'

function makeWrapper() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return ({ children }: { children: React.ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children)
}

function mockFetchOnce(data: object, maxAgeSecs: number) {
  vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
    new Response(JSON.stringify(data), {
      status: 200,
      headers: { 'Cache-Control': `private, max-age=${maxAgeSecs}` },
    }),
  )
}

const USAGE_DATA = { hasAuth: true, error: null, plan: 'Max', tiers: [] }

describe('useOAuthUsage', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it('exposes OAuthUsage directly (select strips the internal wrapper)', async () => {
    mockFetchOnce(USAGE_DATA, 300)

    const { result } = renderHook(() => useOAuthUsage(), { wrapper: makeWrapper() })

    await waitFor(() => expect(result.current.data).toBeDefined())

    // Consumers get OAuthUsage, not OAuthUsageResult — maxAgeSecs must not leak through
    expect(result.current.data).toEqual(USAGE_DATA)
    expect((result.current.data as unknown as { maxAgeSecs?: number }).maxAgeSecs).toBeUndefined()
  })

  /**
   * Regression test for the module-level `let serverMaxAgeSecs` bug.
   *
   * Bug: interval was captured from a plain JS `let` at first render and never
   * updated — so even if the server returned max-age=60, the hook polled every
   * 300s (the stale initial default). No re-render = no interval update.
   *
   * Fix: store in React state (useState). setIntervalMs in queryFn triggers a
   * re-render, so TanStack Query reads the new refetchInterval on the next cycle.
   *
   * Verified by: server returns max-age=60, we advance 61s — if the bug were
   * present (interval frozen at 300s), no refetch would fire.
   */
  it('adopts server-returned max-age as refetch interval', async () => {
    // First fetch: server says cache for 60s
    mockFetchOnce(USAGE_DATA, 60)

    const { result } = renderHook(() => useOAuthUsage(), { wrapper: makeWrapper() })

    await waitFor(() => expect(result.current.data).toBeDefined())

    // Switch to fake timers AFTER initial fetch settles
    vi.useFakeTimers()

    // Mock the second fetch (triggered by the 60s interval)
    const secondFetch = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify(USAGE_DATA), {
        status: 200,
        headers: { 'Cache-Control': 'private, max-age=60' },
      }),
    )

    // Advance past 60s and drain microtasks — should trigger a refetch.
    // If the bug were present (interval frozen at 300s), no fetch would fire here.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(61_000)
    })

    expect(secondFetch).toHaveBeenCalledTimes(1)
  })

  it('does NOT refetch before server max-age elapses', async () => {
    // Use fake timers from the start so TanStack Query's intervals are fake-controlled
    vi.useFakeTimers()

    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify(USAGE_DATA), {
        status: 200,
        headers: { 'Cache-Control': 'private, max-age=120' },
      }),
    )

    const { result } = renderHook(() => useOAuthUsage(), { wrapper: makeWrapper() })

    // Advance a tiny bit to let the initial fetch fire and settle
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100)
    })
    expect(result.current.data).toEqual(USAGE_DATA)
    expect(fetchMock).toHaveBeenCalledTimes(1)

    // Advance 90s more — still within 120s TTL, no refetch should fire
    await act(async () => {
      await vi.advanceTimersByTimeAsync(90_000)
    })

    expect(fetchMock).toHaveBeenCalledTimes(1) // still only the initial fetch
  })
})
