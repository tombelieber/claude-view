import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatCard } from './StatCard'

describe('StatCard', () => {
  describe('rendering', () => {
    it('should render label and value', () => {
      render(<StatCard label="Sessions" value="6,742" />)

      expect(screen.getByText('Sessions')).toBeInTheDocument()
      expect(screen.getByText('6,742')).toBeInTheDocument()
    })

    it('should render date values', () => {
      render(<StatCard label="Oldest Session" value="Oct 14, 2024" />)

      expect(screen.getByText('Oldest Session')).toBeInTheDocument()
      expect(screen.getByText('Oct 14, 2024')).toBeInTheDocument()
    })

    it('should render relative time values', () => {
      render(<StatCard label="Last Git Sync" value="3m ago" />)

      expect(screen.getByText('Last Git Sync')).toBeInTheDocument()
      expect(screen.getByText('3m ago')).toBeInTheDocument()
    })
  })

  describe('accessibility', () => {
    it('should have role="group"', () => {
      render(<StatCard label="Sessions" value="6,742" />)

      const group = screen.getByRole('group')
      expect(group).toBeInTheDocument()
    })

    it('should have aria-label combining label and value', () => {
      render(<StatCard label="Sessions" value="6,742" />)

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute('aria-label', 'Sessions: 6,742')
    })

    it('should have aria-hidden on visual elements', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const ariaHiddenElements = container.querySelectorAll('[aria-hidden="true"]')
      expect(ariaHiddenElements.length).toBe(2) // label and value
    })
  })

  describe('styling', () => {
    it('should apply custom className', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" className="custom-class" />
      )

      expect(container.firstChild).toHaveClass('custom-class')
    })

    it('should have centered text', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const card = container.firstChild as HTMLElement
      expect(card.className).toMatch(/text-center/)
    })

    it('should have rounded corners', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const card = container.firstChild as HTMLElement
      expect(card.className).toMatch(/rounded-lg/)
    })

    it('should have gray background', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const card = container.firstChild as HTMLElement
      expect(card.className).toMatch(/bg-gray-50/)
    })

    it('should have tabular-nums on value', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const valueElement = container.querySelector('.tabular-nums')
      expect(valueElement).toBeInTheDocument()
    })

    it('should have uppercase tracking-wider on label', () => {
      const { container } = render(
        <StatCard label="Sessions" value="6,742" />
      )

      const labelElement = container.querySelector('.uppercase')
      expect(labelElement).toBeInTheDocument()
      expect(labelElement?.className).toMatch(/tracking-wider/)
    })
  })

  describe('edge cases', () => {
    it('should handle empty label', () => {
      render(<StatCard label="" value="100" />)

      const group = screen.getByRole('group')
      expect(group).toHaveAttribute('aria-label', ': 100')
    })

    it('should handle empty value', () => {
      render(<StatCard label="Empty" value="" />)

      expect(screen.getByText('Empty')).toBeInTheDocument()
      const group = screen.getByRole('group')
      expect(group).toHaveAttribute('aria-label', 'Empty: ')
    })

    it('should handle very long labels', () => {
      render(
        <StatCard
          label="Very Long Label That Might Need Wrapping"
          value="123"
        />
      )

      expect(screen.getByText('Very Long Label That Might Need Wrapping')).toBeInTheDocument()
    })

    it('should handle special characters', () => {
      render(<StatCard label="Rate %" value="< 1%" />)

      expect(screen.getByText('Rate %')).toBeInTheDocument()
      expect(screen.getByText('< 1%')).toBeInTheDocument()
    })

    it('should handle numeric-only values', () => {
      render(<StatCard label="Count" value="0" />)

      expect(screen.getByText('0')).toBeInTheDocument()
    })
  })

  describe('use cases from design spec', () => {
    it('should display session count', () => {
      render(<StatCard label="Sessions" value="6,742" />)
      expect(screen.getByText('6,742')).toBeInTheDocument()
    })

    it('should display project count', () => {
      render(<StatCard label="Projects" value="47" />)
      expect(screen.getByText('47')).toBeInTheDocument()
    })

    it('should display commit count', () => {
      render(<StatCard label="Commits" value="1,245" />)
      expect(screen.getByText('1,245')).toBeInTheDocument()
    })

    it('should display oldest session date', () => {
      render(<StatCard label="Oldest Session" value="Oct 14, 2024" />)
      expect(screen.getByText('Oct 14, 2024')).toBeInTheDocument()
    })

    it('should display index built time', () => {
      render(<StatCard label="Index Built" value="2s ago" />)
      expect(screen.getByText('2s ago')).toBeInTheDocument()
    })

    it('should display last git sync time', () => {
      render(<StatCard label="Last Git Sync" value="3m ago" />)
      expect(screen.getByText('3m ago')).toBeInTheDocument()
    })
  })
})
