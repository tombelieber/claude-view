import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { type ReactNode, createElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { type TrendsParams, useTrendsData } from './use-trends-data'

const mockFetch = vi.fn()

const mockTrendsResponse = {
  metric: 'sessions',
  dataPoints: [],
  average: 1,
  trend: 0,
  trendDirection: 'stable',
  insight: 'ok',
  categoryEvolution: null,
  categoryInsight: null,
  classificationRequired: false,
  activityHeatmap: [],
  heatmapInsight: 'ok',
  periodStart: '2026-01-01',
  periodEnd: '2026-01-31',
  totalSessions: 1,
  meta: {
    effectiveRange: { from: 1735689600, to: 1738368000, source: 'explicit_bounds' },
    dataScope: {
      sessions: 'primary_sessions_only',
      workload: 'primary_plus_subagent_work',
    },
    sessionBreakdown: {
      primarySessions: 1,
      sidechainSessions: 4,
      otherSessions: 2,
      totalObservedSessions: 7,
    },
  },
}

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  })

  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children)
}

function getRequestedUrl() {
  const request = mockFetch.mock.calls[0]?.[0]
  expect(typeof request).toBe('string')
  return new URL(request as string, 'http://localhost')
}

describe('useTrendsData', () => {
  beforeEach(() => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: async () => mockTrendsResponse,
      text: async () => '',
    })
    vi.stubGlobal('fetch', mockFetch)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.clearAllMocks()
  })

  it('serializes from=0 using nullish checks', async () => {
    renderHook(
      () =>
        useTrendsData({
          metric: 'sessions',
          granularity: 'day',
          from: 0,
          to: 123,
        }),
      { wrapper: createWrapper() },
    )

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledTimes(1)
    })

    const url = getRequestedUrl()
    expect(url.pathname).toBe('/api/insights/trends')
    expect(url.searchParams.get('from')).toBe('0')
    expect(url.searchParams.get('to')).toBe('123')
    expect(url.searchParams.has('range')).toBe(false)
  })

  it('preserves scope metadata from the API response', async () => {
    const { result } = renderHook(
      () =>
        useTrendsData({
          metric: 'sessions',
          granularity: 'day',
          from: 10,
          to: 20,
        }),
      { wrapper: createWrapper() },
    )

    await waitFor(() => {
      expect(result.current.data?.meta.dataScope?.sessions).toBe('primary_sessions_only')
    })

    expect(result.current.data?.meta.dataScope?.workload).toBe('primary_plus_subagent_work')
    expect(result.current.data?.meta.sessionBreakdown?.primarySessions).toBe(1)
    expect(result.current.data?.meta.sessionBreakdown?.sidechainSessions).toBe(4)
    expect(result.current.data?.meta.sessionBreakdown?.otherSessions).toBe(2)
    expect(result.current.data?.meta.sessionBreakdown?.totalObservedSessions).toBe(7)
  })

  it('sends range-only payload without from/to', async () => {
    renderHook(
      () =>
        useTrendsData({
          metric: 'sessions',
          granularity: 'week',
          range: '3mo',
        }),
      { wrapper: createWrapper() },
    )

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledTimes(1)
    })

    const url = getRequestedUrl()
    expect(url.searchParams.get('range')).toBe('3mo')
    expect(url.searchParams.has('from')).toBe(false)
    expect(url.searchParams.has('to')).toBe(false)
  })

  it('rejects mixed range and from/to params before making a request', async () => {
    const invalidParams = {
      metric: 'sessions',
      granularity: 'day',
      range: '3mo',
      from: 1,
      to: 2,
    } as unknown as TrendsParams

    const { result } = renderHook(() => useTrendsData(invalidParams), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current.isError).toBe(true)
    })

    expect(mockFetch).not.toHaveBeenCalled()
    expect(result.current.error).toBeInstanceOf(Error)
    expect((result.current.error as Error).message).toContain('either `range` or `from`/`to`')
  })
})
