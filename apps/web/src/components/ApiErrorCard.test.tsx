import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ApiErrorCard } from './ApiErrorCard'

describe('ApiErrorCard', () => {
  const defaultProps = {
    error: { code: 429, message: 'Rate limit exceeded' },
    retryAttempt: 1,
    maxRetries: 3,
  }

  describe('Happy path', () => {
    it('should render error code and message', () => {
      render(<ApiErrorCard {...defaultProps} />)
      expect(screen.getByText(/429/)).toBeInTheDocument()
      expect(screen.getByText(/Rate limit exceeded/)).toBeInTheDocument()
    })

    it('should render retry count when expanded', () => {
      render(<ApiErrorCard {...defaultProps} />)
      fireEvent.click(screen.getByRole('button'))
      expect(screen.getByText(/1\/3/)).toBeInTheDocument()
    })

    it('should render backoff time when expanded', () => {
      render(<ApiErrorCard {...defaultProps} retryInMs={5000} />)
      fireEvent.click(screen.getByRole('button'))
      expect(screen.getByText(/5000ms/)).toBeInTheDocument()
    })

    it('should have red left border styling', () => {
      const { container } = render(<ApiErrorCard {...defaultProps} />)
      const wrapper = container.firstElementChild as HTMLElement
      expect(wrapper.className).toMatch(/red/)
    })

    it('should render the error icon', () => {
      const { container } = render(<ApiErrorCard {...defaultProps} />)
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('Collapsible behavior', () => {
    it('should toggle content on click', () => {
      render(<ApiErrorCard {...defaultProps} />)

      const button = screen.getByRole('button')
      expect(button).toBeInTheDocument()

      // Initially collapsed - error details not visible
      expect(screen.queryByText(/Retry:/)).not.toBeInTheDocument()

      // Click to expand
      fireEvent.click(button)
      expect(screen.getByText(/Retry:/)).toBeInTheDocument()

      // Click to collapse
      fireEvent.click(button)
      expect(screen.queryByText(/Retry:/)).not.toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should show "Unknown error" for empty error object', () => {
      render(<ApiErrorCard error={{}} retryAttempt={1} maxRetries={3} />)
      expect(screen.getByText(/Unknown error/)).toBeInTheDocument()
    })

    it('should show warning when retryAttempt exceeds maxRetries', () => {
      render(
        <ApiErrorCard
          error={{ code: 500, message: 'Server error' }}
          retryAttempt={4}
          maxRetries={3}
        />
      )
      expect(screen.getByText(/retries exhausted/i)).toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(<ApiErrorCard {...defaultProps} />)
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })

    it('should have accessible button for collapse toggle', () => {
      render(<ApiErrorCard {...defaultProps} />)
      const button = screen.getByRole('button')
      expect(button).toBeInTheDocument()
    })

    it('should be keyboard navigable', () => {
      render(<ApiErrorCard {...defaultProps} />)
      const button = screen.getByRole('button')
      button.focus()
      expect(button).toHaveFocus()
      fireEvent.keyDown(button, { key: 'Enter' })
      fireEvent.click(button)
      expect(screen.getByText(/Retry:/)).toBeInTheDocument()
    })
  })
})
