import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { TaskQueueCard } from './TaskQueueCard'

describe('TaskQueueCard', () => {
  describe('Schema-faithful rendering', () => {
    it('should display task description and type badge', () => {
      render(
        <TaskQueueCard
          taskDescription="Waiting for file lock on package cache"
          taskType="local_bash"
        />,
      )

      expect(screen.getByText('Waiting for file lock on package cache')).toBeInTheDocument()
      expect(screen.getByText('local_bash')).toBeInTheDocument()
    })

    it('should show fallback when taskDescription is empty', () => {
      render(<TaskQueueCard taskDescription="" taskType="agent" />)

      expect(screen.getByText('Waiting for task')).toBeInTheDocument()
    })
  })

  describe('Visual styling', () => {
    it('should have orange left border', () => {
      const { container } = render(<TaskQueueCard taskDescription="test" taskType="local_bash" />)

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('flex')
    })
  })

  describe('No collapse', () => {
    it('should not have a button (not collapsible)', () => {
      render(<TaskQueueCard taskDescription="test" taskType="agent" />)

      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })

  describe('ARIA', () => {
    it('should have ARIA label on the card', () => {
      render(<TaskQueueCard taskDescription="test" taskType="agent" />)

      expect(screen.getByLabelText(/task queue/i)).toBeInTheDocument()
    })
  })
})
