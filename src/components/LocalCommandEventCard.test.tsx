import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { LocalCommandEventCard } from './LocalCommandEventCard'

describe('LocalCommandEventCard', () => {
  describe('Happy path', () => {
    it('should render command content', () => {
      render(<LocalCommandEventCard content="git status" />)
      expect(screen.getByText('git status')).toBeInTheDocument()
    })

    it('should render terminal icon', () => {
      const { container } = render(<LocalCommandEventCard content="ls -la" />)
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })

    it('should have gray text styling', () => {
      const { container } = render(<LocalCommandEventCard content="pwd" />)
      const wrapper = container.firstElementChild as HTMLElement
      expect(wrapper.className).toMatch(/gray/)
    })

    it('should render as single-line event', () => {
      render(<LocalCommandEventCard content="npm install" />)
      // Should not have a button (not collapsible)
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('should render nothing for empty content', () => {
      const { container } = render(<LocalCommandEventCard content="" />)
      expect(container.firstElementChild).toBeNull()
    })

    it('should render nothing for whitespace-only content', () => {
      const { container } = render(<LocalCommandEventCard content="   " />)
      expect(container.firstElementChild).toBeNull()
    })
  })

  describe('Accessibility', () => {
    it('should have aria-hidden on decorative icon', () => {
      const { container } = render(<LocalCommandEventCard content="echo hello" />)
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })

    it('should not be collapsible (no button)', () => {
      render(<LocalCommandEventCard content="echo hello" />)
      expect(screen.queryByRole('button')).not.toBeInTheDocument()
    })
  })
})
