import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MessageQueueEventCard } from './MessageQueueEventCard'

describe('MessageQueueEventCard', () => {
  describe('rendering', () => {
    it('should render enqueue operation with timestamp', () => {
      render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      expect(screen.getByText(/Message enqueued at 14:32:15/)).toBeInTheDocument()
    })

    it('should render dequeue operation as "Message processed"', () => {
      render(
        <MessageQueueEventCard operation="dequeue" timestamp="14:32:15" />
      )
      expect(screen.getByText(/Message processed/)).toBeInTheDocument()
    })

    it('should show content preview when provided', () => {
      render(
        <MessageQueueEventCard
          operation="enqueue"
          timestamp="14:32:15"
          content="Hello world"
        />
      )
      expect(screen.getByText(/Hello world/)).toBeInTheDocument()
    })

    it('should show queueId when provided', () => {
      render(
        <MessageQueueEventCard
          operation="enqueue"
          timestamp="14:32:15"
          queueId="queue-abc-123"
        />
      )
      expect(screen.getByText(/queue-abc-123/)).toBeInTheDocument()
    })
  })

  describe('visual styling', () => {
    it('should have a gray left border', () => {
      const { container } = render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      const card = container.firstElementChild as HTMLElement
      expect(card.className).toMatch(/border-l/)
      expect(card.className).toMatch(/border-l-gray-400/)
    })

    it('should render the ListOrdered icon', () => {
      const { container } = render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('no collapse behavior', () => {
    it('should not have a collapse button', () => {
      render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })

  describe('edge cases', () => {
    it('should render without content (no preview shown)', () => {
      const { container } = render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      expect(container.firstElementChild).toBeInTheDocument()
    })

    it('should render with empty content string', () => {
      const { container } = render(
        <MessageQueueEventCard
          operation="enqueue"
          timestamp="14:32:15"
          content=""
        />
      )
      expect(container.firstElementChild).toBeInTheDocument()
    })

    it('should render without timestamp (no time shown)', () => {
      render(
        <MessageQueueEventCard operation="enqueue" timestamp="" />
      )
      // Should show operation text but not "at" with timestamp
      expect(screen.getByText(/Message enqueued/)).toBeInTheDocument()
      expect(screen.queryByText(/at\s*$/)).not.toBeInTheDocument()
    })
  })

  describe('accessibility', () => {
    it('should have an aria-label on the card', () => {
      render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      expect(screen.getByLabelText(/message queue event/i)).toBeInTheDocument()
    })

    it('should have aria-hidden on the icon', () => {
      const { container } = render(
        <MessageQueueEventCard operation="enqueue" timestamp="14:32:15" />
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
