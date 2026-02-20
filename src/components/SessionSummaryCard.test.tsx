import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { SessionSummaryCard } from './SessionSummaryCard'

describe('SessionSummaryCard', () => {
  describe('rendering', () => {
    it('should show truncated summary (first 150 chars) when collapsed', () => {
      const summary = 'A'.repeat(200)
      render(
        <SessionSummaryCard
          summary={summary}
          leafUuid="uuid-123"
          wordCount={50}
        />
      )
      // Should show truncated text with ellipsis
      expect(screen.getByText(/Session summary:/)).toBeInTheDocument()
      // The displayed text should end with "..." and be shorter than full summary
      const displayed = summary.slice(0, 150) + '...'
      expect(screen.getByText(displayed)).toBeInTheDocument()
    })

    it('should show short summary without truncation', () => {
      render(
        <SessionSummaryCard
          summary="Short summary text"
          leafUuid="uuid-123"
          wordCount={3}
        />
      )
      expect(screen.getByText(/Short summary text/)).toBeInTheDocument()
    })

    it('should show word count', () => {
      render(
        <SessionSummaryCard
          summary="Some summary"
          leafUuid="uuid-123"
          wordCount={42}
        />
      )
      expect(screen.getByText(/42 words/)).toBeInTheDocument()
    })
  })

  describe('visual styling', () => {
    it('should have a gray left border', () => {
      const { container } = render(
        <SessionSummaryCard
          summary="Test summary"
          leafUuid="uuid-123"
          wordCount={10}
        />
      )
      const card = container.firstElementChild as HTMLElement
      expect(card.className).toMatch(/border-l/)
      expect(card.className).toMatch(/border-l-gray-300/)
    })

    it('should render the BookOpen icon', () => {
      const { container } = render(
        <SessionSummaryCard
          summary="Test summary"
          leafUuid="uuid-123"
          wordCount={10}
        />
      )
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('collapsible behavior', () => {
    it('should expand to show full summary on click', () => {
      const summary = 'Word '.repeat(100).trim()
      render(
        <SessionSummaryCard
          summary={summary}
          leafUuid="uuid-123"
          wordCount={100}
        />
      )
      // Click to expand
      fireEvent.click(screen.getByRole('button'))
      // Full summary should now be visible
      expect(screen.getByText(summary)).toBeInTheDocument()
    })

    it('should toggle between collapsed and expanded', () => {
      const summary = 'X'.repeat(200)
      render(
        <SessionSummaryCard
          summary={summary}
          leafUuid="uuid-123"
          wordCount={50}
        />
      )
      const button = screen.getByRole('button')

      // Expand
      fireEvent.click(button)
      expect(screen.getByText(summary)).toBeInTheDocument()

      // Collapse
      fireEvent.click(button)
      // Full text should no longer be displayed
      expect(screen.queryByText(summary)).not.toBeInTheDocument()
    })
  })

  describe('edge cases', () => {
    it('should show "No summary available" when summary is empty', () => {
      render(
        <SessionSummaryCard
          summary=""
          leafUuid="uuid-123"
          wordCount={0}
        />
      )
      expect(screen.getByText(/No summary available/i)).toBeInTheDocument()
    })

    it('should handle very long summary in collapsed state', () => {
      const summary = 'LongWord '.repeat(500).trim()
      const { container } = render(
        <SessionSummaryCard
          summary={summary}
          leafUuid="uuid-123"
          wordCount={500}
        />
      )
      // Should render without crashing
      expect(container.firstElementChild).toBeInTheDocument()
    })
  })

  describe('accessibility', () => {
    it('should have an aria-label on the card', () => {
      render(
        <SessionSummaryCard
          summary="Test summary"
          leafUuid="uuid-123"
          wordCount={10}
        />
      )
      expect(screen.getByLabelText(/session summary/i)).toBeInTheDocument()
    })

    it('should have aria-expanded on the collapse button', () => {
      render(
        <SessionSummaryCard
          summary="Test summary"
          leafUuid="uuid-123"
          wordCount={10}
        />
      )
      const button = screen.getByRole('button')
      expect(button).toHaveAttribute('aria-expanded')
    })

    it('should have aria-hidden on the icon', () => {
      const { container } = render(
        <SessionSummaryCard
          summary="Test summary"
          leafUuid="uuid-123"
          wordCount={10}
        />
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
