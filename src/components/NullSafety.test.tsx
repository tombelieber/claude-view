import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Message } from './Message'
import { CodeBlock } from './CodeBlock'
import { MetricCard } from './MetricCard'
import { SessionCard } from './SessionCard'
import { DashboardMetricsGrid } from './DashboardMetricsGrid'
import { ActivityCalendar } from './ActivityCalendar'
import { CommandPalette } from './CommandPalette'
import { Skeleton } from './LoadingStates'
import { TierBadge } from './TierBadge'

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
      const { container } = render(
        <CodeBlock code={null as any} language="javascript" />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined code safely', () => {
      const { container } = render(
        <CodeBlock code={undefined as any} language="javascript" />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle null language safely', () => {
      const { container } = render(
        <CodeBlock code="const x = 1;" language={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty code safely', () => {
      const { container } = render(
        <CodeBlock code="" language="javascript" />
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('MetricCard component', () => {
    it('should handle null label safely', () => {
      const { container } = render(
        <MetricCard label={null as any} value={42} change={0} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined value safely', () => {
      const { container } = render(
        <MetricCard label="Test" value={undefined as any} change={0} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle null change safely', () => {
      const { container } = render(
        <MetricCard label="Test" value={42} change={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle all nulls safely', () => {
      const { container } = render(
        <MetricCard
          label={null as any}
          value={null as any}
          change={null as any}
        />
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('SessionCard component', () => {
    it('should handle null session data safely', () => {
      const { container } = render(
        <SessionCard
          sessionId="test"
          title={null as any}
          created={new Date()}
          tokens={{} as any}
        />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined created date safely', () => {
      const { container } = render(
        <SessionCard
          sessionId="test"
          title="Test Session"
          created={undefined as any}
          tokens={{} as any}
        />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle null tokens safely', () => {
      const { container } = render(
        <SessionCard
          sessionId="test"
          title="Test Session"
          created={new Date()}
          tokens={null as any}
        />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty tokens safely', () => {
      const { container } = render(
        <SessionCard
          sessionId="test"
          title="Test Session"
          created={new Date()}
          tokens={{}}
        />
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('DashboardMetricsGrid component', () => {
    it('should handle null metrics safely', () => {
      const { container } = render(
        <DashboardMetricsGrid metrics={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined metrics safely', () => {
      const { container } = render(
        <DashboardMetricsGrid metrics={undefined as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty metrics array safely', () => {
      const { container } = render(<DashboardMetricsGrid metrics={[]} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined array items safely', () => {
      const { container } = render(
        <DashboardMetricsGrid metrics={[undefined, null] as any} />
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('ActivityCalendar component', () => {
    it('should handle null data safely', () => {
      const { container } = render(
        <ActivityCalendar data={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined data safely', () => {
      const { container } = render(
        <ActivityCalendar data={undefined as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty data safely', () => {
      const { container } = render(<ActivityCalendar data={{}} />)

      expect(container).toBeInTheDocument()
    })

    it('should handle empty data array safely', () => {
      const { container } = render(<ActivityCalendar data={[]} as any />)

      expect(container).toBeInTheDocument()
    })
  })

  // CommandPalette requires Router context, tested in integration tests instead

  describe('Skeleton component', () => {
    it('should handle null label safely', () => {
      const { container } = render(
        <Skeleton label={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined rows safely', () => {
      const { container } = render(
        <Skeleton label="Test" rows={undefined} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle zero rows safely', () => {
      const { container } = render(
        <Skeleton label="Test" rows={0} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty label safely', () => {
      const { container } = render(
        <Skeleton label="" rows={3} />
      )

      expect(container).toBeInTheDocument()
    })
  })

  describe('TierBadge component', () => {
    it('should handle null tier safely', () => {
      const { container } = render(
        <TierBadge tier={null as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle undefined tier safely', () => {
      const { container } = render(
        <TierBadge tier={undefined as any} />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle empty string tier safely', () => {
      const { container } = render(
        <TierBadge tier="" />
      )

      expect(container).toBeInTheDocument()
    })

    it('should handle invalid tier values safely', () => {
      const { container } = render(
        <TierBadge tier="invalid-tier-value" />
      )

      expect(container).toBeInTheDocument()
    })
  })
})
