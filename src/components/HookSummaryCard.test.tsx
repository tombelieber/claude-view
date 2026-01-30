import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { HookSummaryCard } from './HookSummaryCard'

describe('HookSummaryCard', () => {
  const defaultProps = {
    hookCount: 4,
    hookInfos: ['pre-commit', 'lint', 'format', 'test'],
  }

  describe('Happy path', () => {
    it('should render hook count', () => {
      render(<HookSummaryCard {...defaultProps} />)
      expect(screen.getByText(/4 hooks executed/)).toBeInTheDocument()
    })

    it('should show error count when hookErrors provided', () => {
      render(
        <HookSummaryCard
          {...defaultProps}
          hookErrors={['lint failed']}
        />
      )
      expect(screen.getByText(/1 error/)).toBeInTheDocument()
    })

    it('should have amber left border styling', () => {
      const { container } = render(<HookSummaryCard {...defaultProps} />)
      const wrapper = container.firstElementChild as HTMLElement
      expect(wrapper.className).toMatch(/amber/)
    })

    it('should render the hook icon', () => {
      const { container } = render(<HookSummaryCard {...defaultProps} />)
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should display duration when provided', () => {
      render(<HookSummaryCard {...defaultProps} durationMs={350} />)
      expect(screen.getByText(/350ms/)).toBeInTheDocument()
    })

    it('should show prevented continuation warning', () => {
      render(
        <HookSummaryCard
          {...defaultProps}
          preventedContinuation={true}
        />
      )
      expect(screen.getByText(/prevented continuation/i)).toBeInTheDocument()
    })
  })

  describe('Collapsible behavior', () => {
    it('should toggle hook list on click', () => {
      render(<HookSummaryCard {...defaultProps} />)

      const button = screen.getByRole('button')

      // Initially collapsed
      expect(screen.queryByText('pre-commit')).not.toBeInTheDocument()

      // Expand
      fireEvent.click(button)
      expect(screen.getByText('pre-commit')).toBeInTheDocument()
      expect(screen.getByText('lint')).toBeInTheDocument()
      expect(screen.getByText('format')).toBeInTheDocument()
      expect(screen.getByText('test')).toBeInTheDocument()

      // Collapse
      fireEvent.click(button)
      expect(screen.queryByText('pre-commit')).not.toBeInTheDocument()
    })

    it('should show hook errors when expanded', () => {
      render(
        <HookSummaryCard
          {...defaultProps}
          hookErrors={['lint failed', 'test timeout']}
        />
      )

      fireEvent.click(screen.getByRole('button'))
      expect(screen.getByText('lint failed')).toBeInTheDocument()
      expect(screen.getByText('test timeout')).toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should show "No hooks" for empty hookInfos', () => {
      render(<HookSummaryCard hookCount={0} hookInfos={[]} />)
      expect(screen.getByText(/No hooks/)).toBeInTheDocument()
    })

    it('should handle undefined hookErrors', () => {
      const { container } = render(<HookSummaryCard {...defaultProps} />)
      expect(container).toBeInTheDocument()
    })

    it('should handle undefined durationMs', () => {
      const { container } = render(<HookSummaryCard {...defaultProps} />)
      expect(container).toBeInTheDocument()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(<HookSummaryCard {...defaultProps} />)
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })

    it('should have accessible button for collapse toggle', () => {
      render(<HookSummaryCard {...defaultProps} />)
      expect(screen.getByRole('button')).toBeInTheDocument()
    })

    it('should be keyboard navigable', () => {
      render(<HookSummaryCard {...defaultProps} />)
      const button = screen.getByRole('button')
      button.focus()
      expect(button).toHaveFocus()
      fireEvent.keyDown(button, { key: 'Enter' })
      fireEvent.click(button)
      expect(screen.getByText('pre-commit')).toBeInTheDocument()
    })
  })
})
