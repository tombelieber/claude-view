import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TurnDurationCard } from './TurnDurationCard'

describe('TurnDurationCard', () => {
  describe('Happy path', () => {
    it('should render duration in milliseconds', () => {
      render(<TurnDurationCard durationMs={245} />)
      expect(screen.getByText(/245ms/)).toBeInTheDocument()
    })

    it('should display "Turn completed in" message', () => {
      render(<TurnDurationCard durationMs={500} />)
      expect(screen.getByText(/Turn completed in 500ms/)).toBeInTheDocument()
    })

    it('should render start and end times when provided', () => {
      render(
        <TurnDurationCard
          durationMs={1200}
          startTime="10:30:00"
          endTime="10:30:01"
        />
      )
      expect(screen.getByText(/10:30:00/)).toBeInTheDocument()
      expect(screen.getByText(/10:30:01/)).toBeInTheDocument()
    })

    it('should render the clock icon', () => {
      const { container } = render(<TurnDurationCard durationMs={100} />)
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should have amber styling', () => {
      const { container } = render(<TurnDurationCard durationMs={100} />)
      const wrapper = container.firstElementChild as HTMLElement
      expect(wrapper.className).toMatch(/amber/)
    })
  })

  describe('Edge cases', () => {
    it('should show "0ms" when durationMs is 0', () => {
      render(<TurnDurationCard durationMs={0} />)
      expect(screen.getByText(/0ms/)).toBeInTheDocument()
    })

    it('should render without start/end times', () => {
      const { container } = render(<TurnDurationCard durationMs={300} />)
      expect(container).toBeInTheDocument()
      expect(screen.getByText(/300ms/)).toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(<TurnDurationCard durationMs={100} />)
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })

    it('should have a role for the timing badge', () => {
      render(<TurnDurationCard durationMs={245} />)
      expect(screen.getByRole('status')).toBeInTheDocument()
    })
  })
})
