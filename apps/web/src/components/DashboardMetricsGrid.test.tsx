import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { DashboardMetricsGrid } from './DashboardMetricsGrid'
import type { DashboardTrends } from '../types/generated'

function makeTrends(overrides: Partial<DashboardTrends> = {}): DashboardTrends {
  const metric = (current: number) => ({
    current: BigInt(current),
    previous: BigInt(0),
    delta: BigInt(current),
    deltaPercent: current > 0 ? 100 : null,
  })

  return {
    sessions: metric(42),
    tokens: metric(15000),
    filesEdited: metric(8),
    commits: metric(3),
    avgTokensPerPrompt: metric(750),
    avgReeditRate: metric(12),
    ...overrides,
  }
}

describe('DashboardMetricsGrid', () => {
  describe('rendering with trends data', () => {
    it('should render 6 MetricCards with correct labels', () => {
      render(<DashboardMetricsGrid trends={makeTrends()} />)

      expect(screen.getByText('Sessions')).toBeInTheDocument()
      expect(screen.getByText('Tokens')).toBeInTheDocument()
      expect(screen.getByText('Files Edited')).toBeInTheDocument()
      expect(screen.getByText('Commits Linked')).toBeInTheDocument()
      expect(screen.getByText('Tokens/Prompt')).toBeInTheDocument()
      expect(screen.getByText('Re-edit Rate')).toBeInTheDocument()
    })

    it('should render formatted values', () => {
      render(<DashboardMetricsGrid trends={makeTrends()} />)

      // 42 sessions (below 1000 threshold)
      expect(screen.getByText('42')).toBeInTheDocument()
      // 15000 tokens formatted as 15.0K
      expect(screen.getByText('15.0K')).toBeInTheDocument()
      // 8 files edited
      expect(screen.getByText('8')).toBeInTheDocument()
      // 3 commits
      expect(screen.getByText('3')).toBeInTheDocument()
      // 750 tokens per prompt
      expect(screen.getByText('750')).toBeInTheDocument()
      // 12% re-edit rate
      expect(screen.getByText('12%')).toBeInTheDocument()
    })

    it('should have aria-label="Period metrics" section', () => {
      render(<DashboardMetricsGrid trends={makeTrends()} />)
      expect(screen.getByLabelText('Period metrics')).toBeInTheDocument()
    })
  })

  describe('skeleton state', () => {
    it('should render 6 skeleton placeholders when trends is null', () => {
      const { container } = render(<DashboardMetricsGrid trends={null} />)
      const skeletons = container.querySelectorAll('.animate-pulse')
      expect(skeletons.length).toBe(6)
    })

    it('should render 6 skeleton placeholders when trends is undefined', () => {
      const { container } = render(<DashboardMetricsGrid trends={undefined} />)
      const skeletons = container.querySelectorAll('.animate-pulse')
      expect(skeletons.length).toBe(6)
    })

    it('should have loading aria-label when trends is null', () => {
      render(<DashboardMetricsGrid trends={null} />)
      expect(screen.getByLabelText('Week-over-week metrics (loading)')).toBeInTheDocument()
    })
  })

  describe('comparisonLabel', () => {
    it('should pass comparisonLabel as footer to MetricCards', () => {
      render(<DashboardMetricsGrid trends={makeTrends()} comparisonLabel="vs prev 7d" />)
      const footers = screen.getAllByText('vs prev 7d')
      expect(footers.length).toBe(6)
    })

    it('should not render footer when comparisonLabel is null', () => {
      render(<DashboardMetricsGrid trends={makeTrends()} comparisonLabel={null} />)
      expect(screen.queryByText('vs prev')).not.toBeInTheDocument()
    })
  })

  describe('edge cases', () => {
    it('should handle zero values gracefully', () => {
      const zeroTrends = makeTrends({
        sessions: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
        tokens: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
        filesEdited: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
        commits: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
        avgTokensPerPrompt: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
        avgReeditRate: { current: BigInt(0), previous: BigInt(0), delta: BigInt(0), deltaPercent: null },
      })
      render(<DashboardMetricsGrid trends={zeroTrends} />)

      // Should show "0" for numeric fields and "0%" for rate
      const zeros = screen.getAllByText('0')
      expect(zeros.length).toBeGreaterThanOrEqual(5) // 5 numeric zeros
      expect(screen.getByText('0%')).toBeInTheDocument() // re-edit rate
    })

    it('should handle large values with formatting', () => {
      const largeTrends = makeTrends({
        tokens: { current: BigInt(2500000), previous: BigInt(0), delta: BigInt(2500000), deltaPercent: 100 },
      })
      render(<DashboardMetricsGrid trends={largeTrends} />)
      expect(screen.getByText('2.5M')).toBeInTheDocument()
    })
  })
})
