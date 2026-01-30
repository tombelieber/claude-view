import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TaskQueueCard } from './TaskQueueCard'

describe('TaskQueueCard', () => {
  describe('Title and status rendering', () => {
    it('should display waiting info with position and duration', () => {
      render(
        <TaskQueueCard waitDuration={1.2} position={3} queueLength={8} />
      )

      expect(screen.getByText(/Waiting for task/)).toBeInTheDocument()
      expect(screen.getByText(/position 3\/8/)).toBeInTheDocument()
      expect(screen.getByText(/1\.2s/)).toBeInTheDocument()
    })

    it('should show just "Waiting for task..." when all props undefined', () => {
      render(<TaskQueueCard />)

      expect(screen.getByText(/Waiting for task\.\.\./)).toBeInTheDocument()
    })

    it('should have gray left border', () => {
      const { container } = render(<TaskQueueCard />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-gray')
    })
  })

  describe('No collapse', () => {
    it('should not have a button (not collapsible)', () => {
      render(<TaskQueueCard position={1} queueLength={5} />)

      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })

  describe('ARIA', () => {
    it('should have ARIA label on the card', () => {
      render(<TaskQueueCard />)

      expect(screen.getByLabelText(/task queue/i)).toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should show position without queue length', () => {
      render(<TaskQueueCard position={2} />)

      expect(screen.getByText(/position 2/)).toBeInTheDocument()
    })

    it('should show duration without position', () => {
      render(<TaskQueueCard waitDuration={3.5} />)

      expect(screen.getByText(/3\.5s/)).toBeInTheDocument()
    })
  })
})
