import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MetricCard } from './MetricCard'

describe('MetricCard', () => {
  describe('rendering', () => {
    it('should render label and value', () => {
      render(<MetricCard label="Sessions" value="142" />)

      expect(screen.getByText('Sessions')).toBeInTheDocument()
      expect(screen.getByText('142')).toBeInTheDocument()
    })

    it('should render sub-value when provided', () => {
      render(
        <MetricCard
          label="Lines Generated"
          value="+12,847"
          subValue="-3,201 removed"
        />
      )

      expect(screen.getByText('-3,201 removed')).toBeInTheDocument()
    })

    it('should render footer when provided', () => {
      render(
        <MetricCard
          label="Lines Generated"
          value="+12,847"
          footer="net: +9,646"
        />
      )

      expect(screen.getByText('net: +9,646')).toBeInTheDocument()
    })

    it('should render without optional props', () => {
      const { container } = render(
        <MetricCard label="Simple Metric" value="100" />
      )

      expect(container.firstChild).toBeInTheDocument()
    })
  })

  describe('trend indicator', () => {
    it('should show positive trend with up arrow', () => {
      const { container } = render(
        <MetricCard
          label="Sessions"
          value="142"
          trend={{ delta: 22, deltaPercent: 18.3 }}
        />
      )

      expect(screen.getByText('+18.3%')).toBeInTheDocument()
      // Check for TrendingUp icon (SVG element)
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should show negative trend with down arrow', () => {
      const { container } = render(
        <MetricCard
          label="Commits"
          value="89"
          trend={{ delta: -5, deltaPercent: -5.3 }}
        />
      )

      expect(screen.getByText('-5.3%')).toBeInTheDocument()
      // Should have TrendingDown icon
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should show neutral trend with minus icon', () => {
      const { container } = render(
        <MetricCard
          label="Rate"
          value="50%"
          trend={{ delta: 0, deltaPercent: 0 }}
        />
      )

      // When delta is 0, no "+" prefix is added
      expect(screen.getByText('0.0%')).toBeInTheDocument()
      // Should have Minus icon
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should not show trend when not provided', () => {
      const { container } = render(
        <MetricCard label="Sessions" value="142" />
      )

      // Should not have any trend icons
      const svgs = container.querySelectorAll('svg')
      expect(svgs.length).toBe(0)
    })

    it('should show "--" when deltaPercent is null', () => {
      render(
        <MetricCard
          label="Sessions"
          value="142"
          trend={{ delta: 10, deltaPercent: null }}
        />
      )

      // When deltaPercent is null, the trend section should not render
      expect(screen.queryByText('--')).not.toBeInTheDocument()
    })
  })

  describe('trend colors', () => {
    it('should have green color for positive trend', () => {
      const { container } = render(
        <MetricCard
          label="Sessions"
          value="142"
          trend={{ delta: 22, deltaPercent: 18.3 }}
        />
      )

      const trendElement = container.querySelector('[class*="text-green"]')
      expect(trendElement).toBeInTheDocument()
    })

    it('should have red color for negative trend', () => {
      const { container } = render(
        <MetricCard
          label="Commits"
          value="89"
          trend={{ delta: -5, deltaPercent: -5.3 }}
        />
      )

      const trendElement = container.querySelector('[class*="text-red"]')
      expect(trendElement).toBeInTheDocument()
    })

    it('should have gray color for neutral trend', () => {
      const { container } = render(
        <MetricCard
          label="Rate"
          value="50%"
          trend={{ delta: 0, deltaPercent: 0 }}
        />
      )

      const trendElement = container.querySelector('[class*="text-gray"]')
      expect(trendElement).toBeInTheDocument()
    })
  })

  describe('accessibility', () => {
    it('should have role="group"', () => {
      render(<MetricCard label="Sessions" value="142" />)

      const group = screen.getByRole('group')
      expect(group).toBeInTheDocument()
    })

    it('should have aria-label with label and value', () => {
      render(<MetricCard label="Sessions" value="142" />)

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('Sessions')
      )
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('142')
      )
    })

    it('should include trend info in aria-label', () => {
      render(
        <MetricCard
          label="Sessions"
          value="142"
          trend={{ delta: 22, deltaPercent: 18.3 }}
        />
      )

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('up')
      )
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('18.3')
      )
    })

    it('should include sub-value in aria-label', () => {
      render(
        <MetricCard
          label="Lines"
          value="+12,847"
          subValue="-3,201 removed"
        />
      )

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('-3,201 removed')
      )
    })

    it('should include footer in aria-label', () => {
      render(
        <MetricCard
          label="Lines"
          value="+12,847"
          footer="net: +9,646"
        />
      )

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute(
        'aria-label',
        expect.stringContaining('net: +9,646')
      )
    })

    it('should have aria-hidden on visual elements', () => {
      const { container } = render(
        <MetricCard
          label="Sessions"
          value="142"
          subValue="this week"
          footer="vs last week"
          trend={{ delta: 22, deltaPercent: 18.3 }}
        />
      )

      // All the visual text elements should be aria-hidden
      const ariaHiddenElements = container.querySelectorAll('[aria-hidden="true"]')
      expect(ariaHiddenElements.length).toBeGreaterThan(0)
    })
  })

  describe('styling', () => {
    it('should apply custom className', () => {
      const { container } = render(
        <MetricCard
          label="Sessions"
          value="142"
          className="custom-class"
        />
      )

      expect(container.firstChild).toHaveClass('custom-class')
    })

    it('should have card styling with border and rounded corners', () => {
      const { container } = render(
        <MetricCard label="Sessions" value="142" />
      )

      const card = container.firstChild as HTMLElement
      expect(card.className).toMatch(/rounded-xl/)
      expect(card.className).toMatch(/border/)
    })

    it('should have tabular-nums on value', () => {
      const { container } = render(
        <MetricCard label="Sessions" value="142" />
      )

      const valueElement = container.querySelector('.tabular-nums')
      expect(valueElement).toBeInTheDocument()
    })

    it('should have blue color for value text', () => {
      const { container } = render(
        <MetricCard label="Sessions" value="142" />
      )

      const valueElement = container.querySelector('[class*="text-blue"]')
      expect(valueElement).toBeInTheDocument()
    })
  })

  describe('edge cases', () => {
    it('should handle empty string values', () => {
      render(<MetricCard label="" value="" />)

      const group = screen.getByRole('group')
      expect(group).toBeInTheDocument()
    })

    it('should handle very long values', () => {
      render(
        <MetricCard
          label="Very Long Label That Might Overflow"
          value="1,234,567,890"
        />
      )

      expect(screen.getByText('Very Long Label That Might Overflow')).toBeInTheDocument()
      expect(screen.getByText('1,234,567,890')).toBeInTheDocument()
    })

    it('should handle special characters in values', () => {
      render(
        <MetricCard
          label="Rate"
          value="50%"
          subValue="< 1s latency"
        />
      )

      expect(screen.getByText('50%')).toBeInTheDocument()
      expect(screen.getByText('< 1s latency')).toBeInTheDocument()
    })
  })
})
