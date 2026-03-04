import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { RouterProvider, createMemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { InsightsPage } from './InsightsPage'

const FIXED_NOW_MS = Date.parse('2026-03-05T00:00:00Z')

const mockUseInsights = vi.fn()
const mockUseTrendsData = vi.fn()

vi.mock('../hooks/use-insights', () => ({
  useInsights: (...args: unknown[]) => mockUseInsights(...args),
}))

vi.mock('../hooks/use-trends-data', async () => {
  const actual = await vi.importActual('../hooks/use-trends-data')
  return {
    ...actual,
    useTrendsData: (...args: unknown[]) => mockUseTrendsData(...args),
  }
})

vi.mock('./ExperimentalBadge', () => ({
  ExperimentalBadge: () => <span data-testid="experimental-badge">Experimental</span>,
}))

vi.mock('./ExperimentalBanner', () => ({
  ExperimentalBanner: () => <div data-testid="experimental-banner" />,
}))

vi.mock('./insights/HeroInsight', () => ({
  HeroInsight: () => <div data-testid="hero-insight" />,
}))

vi.mock('./insights/QuickStatsRow', () => ({
  QuickStatsRow: () => <div data-testid="quick-stats" />,
}))

vi.mock('./insights/PatternsTab', () => ({
  PatternsTab: () => <div data-testid="patterns-tab" />,
}))

vi.mock('./insights/CategoriesTab', () => ({
  CategoriesTab: ({ timeRange }: { timeRange: string }) => (
    <div data-testid="categories-tab">categories-{timeRange}</div>
  ),
}))

vi.mock('./insights/BenchmarksTab', () => ({
  BenchmarksTab: ({ timeRange }: { timeRange: string }) => (
    <div data-testid="benchmarks-tab">benchmarks-{timeRange}</div>
  ),
}))

vi.mock('./insights/QualityTab', () => ({
  QualityTab: () => <div data-testid="quality-tab" />,
}))

vi.mock('./insights/InsightsSkeleton', () => ({
  InsightsSkeleton: () => <div data-testid="insights-skeleton" />,
}))

vi.mock('./insights/TimeRangeFilter', () => ({
  TimeRangeFilter: ({ value, onChange }: { value: string; onChange: (range: string) => void }) => (
    <div>
      <span data-testid="time-range-value">{value}</span>
      <button onClick={() => onChange('7d')}>Set 7d</button>
      <button onClick={() => onChange('all')}>Set all</button>
      <button onClick={() => onChange('30d')}>Set 30d</button>
    </div>
  ),
}))

vi.mock('./insights/PatternsTabs', () => ({
  PatternsTabs: ({ activeTab, onTabChange }: { activeTab: string; onTabChange: (tab: string) => void }) => (
    <div>
      <span data-testid="active-tab-value">{activeTab}</span>
      <button onClick={() => onTabChange('patterns')}>Tab patterns</button>
      <button onClick={() => onTabChange('trends')}>Tab trends</button>
      <button onClick={() => onTabChange('categories')}>Tab categories</button>
    </div>
  ),
}))

vi.mock('./insights/TrendsChart', () => ({
  TrendsChart: ({ onGranularityChange }: { onGranularityChange: (value: 'day' | 'week' | 'month') => void }) => (
    <button onClick={() => onGranularityChange('month')}>Set month granularity</button>
  ),
}))

vi.mock('./insights/CategoryEvolutionChart', () => ({
  CategoryEvolutionChart: () => <div data-testid="category-evolution-chart" />,
}))

vi.mock('./insights/ActivityHeatmapGrid', () => ({
  ActivityHeatmapGrid: () => <div data-testid="activity-heatmap-grid" />,
}))

const mockInsightsResponse = {
  heroInsight: null,
  quickStats: {
    workBreakdown: null,
    efficiency: null,
    patterns: null,
  },
  patternGroups: {
    high: [],
    medium: [],
    observations: [],
  },
  meta: {
    totalSessions: 25,
    patternsReturned: 0,
    minSessionsRequired: 20,
    hasEnoughData: true,
  },
}

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
  periodEnd: '2026-03-05',
  totalSessions: 5,
}

function renderInsightsPage(initialEntry = '/insights') {
  const router = createMemoryRouter(
    [
      {
        path: '/insights',
        element: <InsightsPage />,
      },
    ],
    {
      initialEntries: [initialEntry],
    },
  )

  render(<RouterProvider router={router} />)
  return router
}

describe('InsightsPage', () => {
  beforeEach(() => {
    vi.spyOn(Date, 'now').mockReturnValue(FIXED_NOW_MS)

    mockUseInsights.mockReturnValue({
      data: mockInsightsResponse,
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    })

    mockUseTrendsData.mockReturnValue({
      data: mockTrendsResponse,
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    })
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.clearAllMocks()
  })

  it('syncs timeRange and activeTab from URL on manual edits and back/forward', async () => {
    const router = renderInsightsPage('/insights?range=30d&tab=patterns')

    expect(screen.getByTestId('time-range-value')).toHaveTextContent('30d')
    expect(screen.getByTestId('active-tab-value')).toHaveTextContent('patterns')
    expect(mockUseInsights).toHaveBeenLastCalledWith({ timeRange: '30d' })

    await act(async () => {
      await router.navigate('/insights?range=all&tab=trends')
    })

    await waitFor(() => {
      expect(screen.getByTestId('time-range-value')).toHaveTextContent('all')
      expect(screen.getByTestId('active-tab-value')).toHaveTextContent('trends')
      expect(mockUseInsights).toHaveBeenLastCalledWith({ timeRange: 'all' })
    })

    await act(async () => {
      await router.navigate(-1)
    })

    await waitFor(() => {
      expect(screen.getByTestId('time-range-value')).toHaveTextContent('30d')
      expect(screen.getByTestId('active-tab-value')).toHaveTextContent('patterns')
      expect(mockUseInsights).toHaveBeenLastCalledWith({ timeRange: '30d' })
    })
  })

  it('updates URL params when controls change', async () => {
    const user = userEvent.setup()
    const router = renderInsightsPage('/insights')

    await user.click(screen.getByRole('button', { name: 'Set all' }))
    await user.click(screen.getByRole('button', { name: 'Tab trends' }))

    await waitFor(() => {
      expect(router.state.location.search).toContain('range=all')
      expect(router.state.location.search).toContain('tab=trends')
    })

    await user.click(screen.getByRole('button', { name: 'Set 30d' }))
    await user.click(screen.getByRole('button', { name: 'Tab patterns' }))

    await waitFor(() => {
      expect(router.state.location.search).toBe('')
    })
  })

  it('passes explicit 7d bounds to trends without hidden widening and resets granularity on range change', async () => {
    const user = userEvent.setup()
    const router = renderInsightsPage('/insights?range=7d&tab=trends')

    const expectedNow = Math.floor(FIXED_NOW_MS / 1000)

    await waitFor(() => {
      expect(mockUseTrendsData).toHaveBeenCalled()
    })

    const firstCall = mockUseTrendsData.mock.calls.at(-1)?.[0]
    expect(firstCall).toMatchObject({
      metric: 'reedit_rate',
      granularity: 'day',
      from: expectedNow - 7 * 86400,
      to: expectedNow,
    })
    expect(firstCall).not.toHaveProperty('range')

    await user.click(screen.getByRole('button', { name: 'Set month granularity' }))

    await waitFor(() => {
      const call = mockUseTrendsData.mock.calls.at(-1)?.[0]
      expect(call).toMatchObject({ granularity: 'month' })
    })

    await act(async () => {
      await router.navigate('/insights?range=90d&tab=trends')
    })

    await waitFor(() => {
      const call = mockUseTrendsData.mock.calls.at(-1)?.[0]
      expect(call).toMatchObject({
        granularity: 'week',
        from: expectedNow - 90 * 86400,
        to: expectedNow,
      })
      expect(call).not.toHaveProperty('range')
    })
  })
})
