import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { StatsDashboard } from './StatsDashboard'

// Mock hooks
const mockUseDashboardStats = vi.fn()
const mockUseTimeRange = vi.fn()
const mockRefetch = vi.fn()

vi.mock('../hooks/use-dashboard', () => ({
  useDashboardStats: (...args: unknown[]) => mockUseDashboardStats(...args),
}))

vi.mock('../hooks/use-time-range', () => ({
  useTimeRange: () => mockUseTimeRange(),
}))

vi.mock('../hooks/use-media-query', () => ({
  useIsMobile: () => false,
}))

// Mock child components that fetch their own data
vi.mock('./AIGenerationStats', () => ({
  AIGenerationStats: () => <div data-testid="ai-generation-stats">AI Generation Stats</div>,
}))

vi.mock('./RecentCommits', () => ({
  RecentCommits: () => <div data-testid="recent-commits">Recent Commits</div>,
}))

vi.mock('./ContributionSummaryCard', () => ({
  ContributionSummaryCard: () => <div data-testid="contribution-summary">Contribution Summary</div>,
}))

vi.mock('./CoachCard', () => ({
  CoachCard: () => <div data-testid="coach-card">Coach Card</div>,
}))

function makeStats(overrides = {}) {
  return {
    totalSessions: 42,
    totalProjects: 5,
    periodStart: 1706745600, // Feb 1 2024
    periodEnd: 1707350400,   // Feb 8 2024
    comparisonPeriodStart: null,
    comparisonPeriodEnd: null,
    dataStartDate: 1704067200, // Jan 1 2024
    currentWeek: {
      sessionCount: BigInt(10),
      totalTokens: BigInt(5000),
      totalFilesEdited: BigInt(3),
      commitCount: BigInt(1),
    },
    trends: null,
    heatmap: [
      { date: '2024-02-01', count: 5 },
      { date: '2024-02-02', count: 3 },
    ],
    topSkills: [{ name: '/commit', count: 15 }],
    topCommands: [],
    topMcpTools: [],
    topAgents: [],
    topProjects: [
      { name: 'my-app', displayName: 'My App', sessionCount: 20 },
      { name: 'my-lib', displayName: 'My Lib', sessionCount: 10 },
    ],
    toolTotals: { edit: 100, read: 200, bash: 50, write: 30 },
    longestSessions: [
      { id: 'sess-1', preview: 'Long session', projectDisplayName: 'My App', durationSeconds: 3600 },
    ],
    ...overrides,
  }
}

function defaultTimeRange() {
  return {
    state: {
      preset: '30d' as const,
      customRange: null,
      fromTimestamp: 1706745600,
      toTimestamp: 1707350400,
    },
    setPreset: vi.fn(),
    setCustomRange: vi.fn(),
    label: 'Last 30 days',
    comparisonLabel: 'vs prev 30d',
  }
}

function renderDashboard() {
  return render(
    <MemoryRouter>
      <StatsDashboard />
    </MemoryRouter>,
  )
}

describe('StatsDashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseTimeRange.mockReturnValue(defaultTimeRange())
    mockUseDashboardStats.mockReturnValue({
      data: makeStats(),
      isLoading: false,
      error: null,
      refetch: mockRefetch,
    })
  })

  describe('loading state', () => {
    it('should render DashboardSkeleton when loading', () => {
      mockUseDashboardStats.mockReturnValue({
        data: null,
        isLoading: true,
        error: null,
        refetch: mockRefetch,
      })
      const { container } = renderDashboard()
      // DashboardSkeleton uses role="status" aria-busy="true"
      expect(container.querySelector('[role="status"][aria-busy="true"]')).toBeInTheDocument()
    })
  })

  describe('error state', () => {
    it('should render ErrorState with message', () => {
      mockUseDashboardStats.mockReturnValue({
        data: null,
        isLoading: false,
        error: new Error('Network error'),
        refetch: mockRefetch,
      })
      renderDashboard()
      // ErrorState uses role="alert"
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByText('Network error')).toBeInTheDocument()
    })

    it('should have retry button', () => {
      mockUseDashboardStats.mockReturnValue({
        data: null,
        isLoading: false,
        error: new Error('Network error'),
        refetch: mockRefetch,
      })
      renderDashboard()
      expect(screen.getByText('Try again')).toBeInTheDocument()
    })
  })

  describe('empty state', () => {
    it('should render EmptyState when stats is null', () => {
      mockUseDashboardStats.mockReturnValue({
        data: null,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      renderDashboard()
      expect(screen.getByText('No statistics available')).toBeInTheDocument()
    })
  })

  describe('success state', () => {
    it('should render header with session and project counts', () => {
      renderDashboard()
      expect(screen.getByText('42')).toBeInTheDocument()
      expect(screen.getByText('sessions')).toBeInTheDocument()
      expect(screen.getByText('5')).toBeInTheDocument()
      expect(screen.getByText('projects')).toBeInTheDocument()
    })

    it('should render the title', () => {
      renderDashboard()
      expect(screen.getByText('Your Claude Code Usage')).toBeInTheDocument()
    })

    it('should render invocable categories with data', () => {
      renderDashboard()
      expect(screen.getByText('Top Skills')).toBeInTheDocument()
      expect(screen.getByText('/commit')).toBeInTheDocument()
      expect(screen.getByText('15')).toBeInTheDocument()
    })

    it('should not render empty invocable categories', () => {
      renderDashboard()
      // topCommands, topMcpTools, topAgents are empty arrays
      expect(screen.queryByText('Top Commands')).not.toBeInTheDocument()
      expect(screen.queryByText('Top MCP Tools')).not.toBeInTheDocument()
      expect(screen.queryByText('Top Agents')).not.toBeInTheDocument()
    })

    it('should render Most Active Projects', () => {
      renderDashboard()
      expect(screen.getByText('Most Active Projects')).toBeInTheDocument()
      expect(screen.getByText('My App')).toBeInTheDocument()
      expect(screen.getByText('My Lib')).toBeInTheDocument()
    })

    it('should render tool usage grid', () => {
      renderDashboard()
      expect(screen.getByText('Tool Usage')).toBeInTheDocument()
      expect(screen.getByText('Edits')).toBeInTheDocument()
      expect(screen.getByText('Reads')).toBeInTheDocument()
      expect(screen.getByText('Bash')).toBeInTheDocument()
      // Edits = edit + write = 100 + 30 = 130
      expect(screen.getByText('130')).toBeInTheDocument()
      expect(screen.getByText('200')).toBeInTheDocument()
      expect(screen.getByText('50')).toBeInTheDocument()
    })

    it('should render Longest Sessions', () => {
      renderDashboard()
      expect(screen.getByText('Longest Sessions')).toBeInTheDocument()
      expect(screen.getByText('Long session')).toBeInTheDocument()
      expect(screen.getByText('1.0h')).toBeInTheDocument()
    })

    it('should render AIGenerationStats', () => {
      renderDashboard()
      expect(screen.getByTestId('ai-generation-stats')).toBeInTheDocument()
    })
  })

  describe('time range integration', () => {
    it('should show TimeRangeSelector', () => {
      renderDashboard()
      // TimeRangeSelector renders buttons with preset labels
      expect(screen.getByText('30d')).toBeInTheDocument()
      expect(screen.getByText('7d')).toBeInTheDocument()
      expect(screen.getByText('All')).toBeInTheDocument()
    })

    it('should pass time range to useDashboardStats', () => {
      renderDashboard()
      expect(mockUseDashboardStats).toHaveBeenCalledWith(
        undefined,
        undefined,
        { from: 1706745600, to: 1707350400 },
      )
    })
  })

  describe('date caption', () => {
    it('should show date range caption when period bounds are set', () => {
      renderDashboard()
      // "Showing stats from Feb 1, 2024 - Feb 8, 2024"
      expect(screen.getByText(/Showing stats from/)).toBeInTheDocument()
    })

    it('should show all-time caption when periodStart/End are null', () => {
      mockUseDashboardStats.mockReturnValue({
        data: makeStats({ periodStart: null, periodEnd: null }),
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      renderDashboard()
      expect(screen.getByText('Showing all-time stats')).toBeInTheDocument()
    })

  })
})
