import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { CompactBoundaryCard } from './CompactBoundaryCard'

describe('CompactBoundaryCard', () => {
  describe('Happy path', () => {
    it('should render pre and post token counts', () => {
      render(
        <CompactBoundaryCard
          trigger="auto-triggered"
          preTokens={8000}
          postTokens={4500}
        />
      )
      expect(screen.getByText(/8,000/)).toBeInTheDocument()
      expect(screen.getByText(/4,500/)).toBeInTheDocument()
    })

    it('should render the trigger description', () => {
      render(
        <CompactBoundaryCard
          trigger="auto-triggered"
          preTokens={8000}
          postTokens={4500}
        />
      )
      expect(screen.getByText(/auto-triggered/)).toBeInTheDocument()
    })

    it('should display "Context compacted" label', () => {
      render(
        <CompactBoundaryCard
          trigger="auto-triggered"
          preTokens={8000}
          postTokens={4500}
        />
      )
      expect(screen.getByText(/Context compacted/)).toBeInTheDocument()
    })

    it('should have indigo styling', () => {
      const { container } = render(
        <CompactBoundaryCard trigger="manual" preTokens={5000} postTokens={2500} />
      )
      const wrapper = container.firstElementChild as HTMLElement
      expect(wrapper.className).toMatch(/indigo/)
    })

    it('should render scissors icon', () => {
      const { container } = render(
        <CompactBoundaryCard trigger="manual" preTokens={5000} />
      )
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should show only preTokens when postTokens is undefined', () => {
      render(<CompactBoundaryCard trigger="auto" preTokens={8000} />)
      expect(screen.getByText(/8,000/)).toBeInTheDocument()
      // Should not show arrow or postTokens
      expect(screen.queryByText(/\u2192/)).not.toBeInTheDocument()
    })

    it('should format large numbers with commas', () => {
      render(
        <CompactBoundaryCard
          trigger="auto"
          preTokens={120000}
          postTokens={60000}
        />
      )
      expect(screen.getByText(/120,000/)).toBeInTheDocument()
      expect(screen.getByText(/60,000/)).toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(
        <CompactBoundaryCard trigger="auto" preTokens={5000} />
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })

    it('should not be collapsible (no button)', () => {
      render(<CompactBoundaryCard trigger="auto" preTokens={5000} />)
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })
})
