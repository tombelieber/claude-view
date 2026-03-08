import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ActivityCalendar } from './ActivityCalendar'
import { CodeBlock } from './CodeBlock'
import { DashboardMetricsGrid } from './DashboardMetricsGrid'
import { Skeleton } from './LoadingStates'
import { MetricCard } from './MetricCard'
import { SessionCard } from './SessionCard'
import { TierBadge } from './TierBadge'

function createWrapper() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

// Suppress console errors for this test suite
const originalError = console.error
beforeEach(() => {
  console.error = vi.fn()
})

afterEach(() => {
  console.error = originalError
})

describe('Component null/undefined safety', () => {
  // Message component requires ConversationContext, tested in integration tests

  describe('CodeBlock component', () => {
    it('should handle null code safely', () => {
      const { container } = render(<CodeBlock code={null as any} language="javascript" />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined code safely', () => {
      const { container } = render(<CodeBlock code={undefined as any} language="javascript" />)

      expect(container).toBeInTheDocument()
    })

    it('should handle null language safely', () => {
      const { container } = render(<CodeBlock code="const x = 1;" language={null as any} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty code safely', () => {
      const { container } = render(<CodeBlock code="" language="javascript" />)

      expect(container).toBeInTheDocument()
    })
  })

  describe('MetricCard component', () => {
    it('should handle null label safely', () => {
      const { container } = render(<MetricCard {...({ label: null, value: '42' } as any)} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined value safely', () => {
      const { container } = render(<MetricCard {...({ label: 'Test', value: undefined } as any)} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle null change safely', () => {
      const { container } = render(
        <MetricCard {...({ label: 'Test', value: '42', trend: null } as any)} />,
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle all nulls safely', () => {
      const { container } = render(
        <MetricCard {...({ label: null, value: null, change: null } as any)} />,
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('SessionCard component', () => {
    it('should handle null session data safely', () => {
      const { container } = render(
        <SessionCard
          {...({ sessionId: 'test', title: null, created: new Date(), tokens: {} } as any)}
        />,
        { wrapper: createWrapper() },
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined created date safely', () => {
      const { container } = render(
        <SessionCard
          {...({ sessionId: 'test', title: 'Test Session', created: undefined, tokens: {} } as any)}
        />,
        { wrapper: createWrapper() },
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle null tokens safely', () => {
      const { container } = render(
        <SessionCard
          {...({
            sessionId: 'test',
            title: 'Test Session',
            created: new Date(),
            tokens: null,
          } as any)}
        />,
        { wrapper: createWrapper() },
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty tokens safely', () => {
      const { container } = render(
        <SessionCard
          {...({
            sessionId: 'test',
            title: 'Test Session',
            created: new Date(),
            tokens: {},
          } as any)}
        />,
        { wrapper: createWrapper() },
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('DashboardMetricsGrid component', () => {
    it('should handle null metrics safely', () => {
      const { container } = render(<DashboardMetricsGrid trends={null} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined metrics safely', () => {
      const { container } = render(<DashboardMetricsGrid trends={undefined} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty metrics array safely', () => {
      const { container } = render(<DashboardMetricsGrid {...({ trends: [] } as any)} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined array items safely', () => {
      const { container } = render(
        <DashboardMetricsGrid {...({ trends: [undefined, null] } as any)} />,
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('ActivityCalendar component', () => {
    it('should handle null data safely', () => {
      const { container } = render(<ActivityCalendar sessions={null} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined data safely', () => {
      const { container } = render(<ActivityCalendar sessions={undefined} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty data safely', () => {
      const { container } = render(<ActivityCalendar {...({ sessions: {} } as any)} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty data array safely', () => {
      const { container } = render(<ActivityCalendar sessions={[]} />)

      expect(container).toBeInTheDocument()
    })
  })

  // CommandPalette requires Router context, tested in integration tests instead

  describe('Skeleton component', () => {
    it('should handle null label safely', () => {
      const { container } = render(<Skeleton label={null as any} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined rows safely', () => {
      const { container } = render(<Skeleton label="Test" rows={undefined} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle zero rows safely', () => {
      const { container } = render(<Skeleton label="Test" rows={0} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty label safely', () => {
      const { container } = render(<Skeleton label="" rows={3} />)

      expect(container).toBeInTheDocument()
    })
  })

  describe('TierBadge component', () => {
    it('should handle null tier safely', () => {
      const { container } = render(<TierBadge tier={null as any} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined tier safely', () => {
      const { container } = render(<TierBadge tier={undefined as any} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty string tier safely', () => {
      const { container } = render(<TierBadge tier={'' as any} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle invalid tier values safely', () => {
      const { container } = render(<TierBadge tier={'invalid-tier-value' as any} />)

      expect(container).toBeInTheDocument()
    })
  })
})
