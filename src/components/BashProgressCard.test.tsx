import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { BashProgressCard } from './BashProgressCard'

describe('BashProgressCard', () => {
  describe('Title and status rendering', () => {
    it('should display command with exit code and duration', () => {
      render(
        <BashProgressCard
          command="npm test"
          output="All tests passed"
          exitCode={0}
          duration={342}
        />
      )

      expect(screen.getByText(/\$ npm test/)).toBeInTheDocument()
      expect(screen.getByText(/exit 0/)).toBeInTheDocument()
      expect(screen.getByText(/342ms/)).toBeInTheDocument()
    })

    it('should show green styling for exit code 0', () => {
      const { container } = render(
        <BashProgressCard command="ls" exitCode={0} />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-green')
    })

    it('should show red styling for non-zero exit code', () => {
      const { container } = render(
        <BashProgressCard command="bad-cmd" exitCode={1} />
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-red')
    })
  })

  describe('Collapsible behavior', () => {
    it('should expand to show output on click', () => {
      render(
        <BashProgressCard
          command="npm test"
          output="All tests passed\n5 tests, 0 failures"
          exitCode={0}
        />
      )

      expect(screen.queryByText(/All tests passed/)).not.toBeInTheDocument()

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText(/All tests passed/)).toBeInTheDocument()
    })

    it('should show "No output" when output is empty string', () => {
      render(
        <BashProgressCard command="touch file.txt" output="" exitCode={0} />
      )

      fireEvent.click(screen.getByRole('button'))

      expect(screen.getByText('No output')).toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should not show exit status when exitCode is undefined', () => {
      render(<BashProgressCard command="running..." />)

      expect(screen.queryByText(/exit/)).not.toBeInTheDocument()
    })

    it('should not show duration when undefined', () => {
      render(<BashProgressCard command="ls" exitCode={0} />)

      expect(screen.queryByText(/ms/)).not.toBeInTheDocument()
    })

    it('should render with only command prop', () => {
      const { container } = render(<BashProgressCard command="echo hello" />)
      expect(container).toBeInTheDocument()
      expect(screen.getByText(/\$ echo hello/)).toBeInTheDocument()
    })
  })

  describe('ARIA and keyboard', () => {
    it('should have ARIA label', () => {
      render(<BashProgressCard command="npm test" exitCode={0} />)
      expect(screen.getByRole('button', { name: /bash/i })).toBeInTheDocument()
    })
  })
})
